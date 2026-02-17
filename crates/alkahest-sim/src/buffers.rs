use alkahest_core::constants::{BYTES_PER_CHUNK, MAX_CHUNK_SLOTS, VOXELS_PER_CHUNK};

/// Total byte size of one chunk's voxel data (32^3 * 8 bytes = 256 KB).
pub const CHUNK_BUFFER_SIZE: u64 = BYTES_PER_CHUNK as u64;

/// Total byte size of one chunk's charge data (32^3 * 4 bytes = 128 KB).
/// Each voxel stores 1 u32 for charge (only low 8 bits used, but u32 avoids
/// WGSL alignment issues and cross-thread write conflicts in packed bytes).
pub const CHARGE_SLOT_SIZE: u64 = VOXELS_PER_CHUNK as u64 * 4;

/// Double-buffered chunk pool for multi-chunk simulation (C-SIM-1).
///
/// Two large pool buffers alternate roles each tick: one is read-only (current state),
/// the other is read-write (next state). Each pool is divided into fixed-size 256 KB slots,
/// one per loaded chunk. After all passes complete, `swap()` flips the pool roles.
///
/// Additionally, two charge pool buffers store per-voxel electrical charge (M15).
/// Charge pools follow the same double-buffering and slot layout.
///
/// Pool capacity is determined by `min(device.maxBufferSize / CHUNK_BUFFER_SIZE, MAX_CHUNK_SLOTS)`.
pub struct ChunkPool {
    pools: [wgpu::Buffer; 2],
    charge_pools: [wgpu::Buffer; 2],
    /// 0 or 1: index of the pool currently used for reading.
    read_index: u32,
    /// Number of slots available in each pool.
    slot_count: u32,
    /// Total byte size of each pool buffer.
    pool_byte_size: u64,
    /// Total byte size of each charge pool buffer.
    charge_pool_byte_size: u64,
}

impl ChunkPool {
    /// Create both pool buffers at init time (C-PERF-2: no per-frame allocation).
    /// Pool capacity is capped by the device's maxBufferSize limit (C-GPU-2).
    pub fn new(device: &wgpu::Device) -> Self {
        let max_buffer_size = device.limits().max_buffer_size;
        let slot_count = (max_buffer_size / CHUNK_BUFFER_SIZE).min(MAX_CHUNK_SLOTS as u64) as u32;

        if slot_count < 64 {
            log::warn!(
                "ChunkPool: only {} slots available (maxBufferSize={}). Minimum 64 recommended.",
                slot_count,
                max_buffer_size
            );
        }

        let pool_byte_size = slot_count as u64 * CHUNK_BUFFER_SIZE;
        let charge_pool_byte_size = slot_count as u64 * CHARGE_SLOT_SIZE;
        log::info!(
            "ChunkPool: {} slots, {} MB per pool, {} MB per charge pool",
            slot_count,
            pool_byte_size / (1024 * 1024),
            charge_pool_byte_size / (1024 * 1024),
        );

        let usage = wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST;

        let pool_a = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("chunk-pool-a"),
            size: pool_byte_size,
            usage,
            mapped_at_creation: false,
        });

        let pool_b = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("chunk-pool-b"),
            size: pool_byte_size,
            usage,
            mapped_at_creation: false,
        });

        let charge_a = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("charge-pool-a"),
            size: charge_pool_byte_size,
            usage,
            mapped_at_creation: false,
        });

        let charge_b = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("charge-pool-b"),
            size: charge_pool_byte_size,
            usage,
            mapped_at_creation: false,
        });

        Self {
            pools: [pool_a, pool_b],
            charge_pools: [charge_a, charge_b],
            read_index: 0,
            slot_count,
            pool_byte_size,
            charge_pool_byte_size,
        }
    }

    /// The pool buffer used for reading this tick.
    pub fn read_pool(&self) -> &wgpu::Buffer {
        &self.pools[self.read_index as usize]
    }

    /// The pool buffer used for writing this tick.
    pub fn write_pool(&self) -> &wgpu::Buffer {
        &self.pools[1 - self.read_index as usize]
    }

    /// The charge pool buffer used for reading this tick.
    pub fn charge_read_pool(&self) -> &wgpu::Buffer {
        &self.charge_pools[self.read_index as usize]
    }

    /// The charge pool buffer used for writing this tick.
    pub fn charge_write_pool(&self) -> &wgpu::Buffer {
        &self.charge_pools[1 - self.read_index as usize]
    }

    /// Swap read/write pool roles. Call after all passes complete.
    pub fn swap(&mut self) {
        self.read_index = 1 - self.read_index;
    }

    /// Current tick parity (0 or 1). Passed to shaders for deterministic tie-breaking.
    pub fn parity(&self) -> u32 {
        self.read_index
    }

    /// Number of pool slots available.
    pub fn slot_count(&self) -> u32 {
        self.slot_count
    }

    /// Total byte size of each pool buffer.
    pub fn pool_byte_size(&self) -> u64 {
        self.pool_byte_size
    }

    /// Total byte size of each charge pool buffer.
    pub fn charge_pool_byte_size(&self) -> u64 {
        self.charge_pool_byte_size
    }

    /// Byte offset of a given slot within a pool buffer.
    pub fn slot_byte_offset(slot: u32) -> u64 {
        slot as u64 * CHUNK_BUFFER_SIZE
    }

    /// Byte offset of a given slot within a charge pool buffer.
    pub fn charge_slot_byte_offset(slot: u32) -> u64 {
        slot as u64 * CHARGE_SLOT_SIZE
    }

    /// Upload voxel data for one chunk slot to the write pool.
    pub fn upload_chunk_data(&self, queue: &wgpu::Queue, slot: u32, data: &[[u32; 2]]) {
        let byte_offset = Self::slot_byte_offset(slot);
        let byte_data: &[u8] = bytemuck::cast_slice(data);
        queue.write_buffer(self.write_pool(), byte_offset, byte_data);
    }

    /// Upload voxel data to both pools (for initial terrain loading).
    pub fn upload_chunk_data_both(&self, queue: &wgpu::Queue, slot: u32, data: &[[u32; 2]]) {
        let byte_offset = Self::slot_byte_offset(slot);
        let byte_data: &[u8] = bytemuck::cast_slice(data);
        queue.write_buffer(&self.pools[0], byte_offset, byte_data);
        queue.write_buffer(&self.pools[1], byte_offset, byte_data);
    }

    /// Copy one slot from read pool to write pool (pre-pass copy for simulation).
    pub fn copy_slot_read_to_write(&self, encoder: &mut wgpu::CommandEncoder, slot: u32) {
        let byte_offset = Self::slot_byte_offset(slot);
        encoder.copy_buffer_to_buffer(
            self.read_pool(),
            byte_offset,
            self.write_pool(),
            byte_offset,
            CHUNK_BUFFER_SIZE,
        );
    }

    /// Copy one charge slot from read to write (pre-pass copy for electrical simulation).
    pub fn copy_charge_slot_read_to_write(&self, encoder: &mut wgpu::CommandEncoder, slot: u32) {
        let byte_offset = Self::charge_slot_byte_offset(slot);
        encoder.copy_buffer_to_buffer(
            self.charge_read_pool(),
            byte_offset,
            self.charge_write_pool(),
            byte_offset,
            CHARGE_SLOT_SIZE,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_buffer_size() {
        // 32^3 voxels * 2 u32s * 4 bytes = 262144 bytes = 256 KB
        assert_eq!(CHUNK_BUFFER_SIZE, 262_144);
    }

    #[test]
    fn test_charge_slot_size() {
        // 32^3 voxels * 1 u32 * 4 bytes = 131072 bytes = 128 KB
        assert_eq!(CHARGE_SLOT_SIZE, 131_072);
    }

    #[test]
    fn test_slot_byte_offset() {
        assert_eq!(ChunkPool::slot_byte_offset(0), 0);
        assert_eq!(ChunkPool::slot_byte_offset(1), 262_144);
        assert_eq!(ChunkPool::slot_byte_offset(2), 524_288);
    }

    #[test]
    fn test_charge_slot_byte_offset() {
        assert_eq!(ChunkPool::charge_slot_byte_offset(0), 0);
        assert_eq!(ChunkPool::charge_slot_byte_offset(1), 131_072);
        assert_eq!(ChunkPool::charge_slot_byte_offset(2), 262_144);
    }
}
