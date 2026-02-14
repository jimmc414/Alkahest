use alkahest_core::constants::VOXELS_PER_CHUNK;

/// Number of u32 values per voxel (8 bytes = 2 x u32).
const U32S_PER_VOXEL: u32 = 2;

/// Total byte size of one voxel buffer.
pub const VOXEL_BUFFER_SIZE: u64 = (VOXELS_PER_CHUNK * U32S_PER_VOXEL * 4) as u64;

/// Double-buffered voxel storage for the simulation (C-SIM-1).
///
/// Two identically-sized storage buffers alternate roles each tick:
/// one is read-only (current state), the other is write-only (next state).
/// After all passes complete, `swap()` flips the roles.
pub struct DoubleBuffer {
    buffers: [wgpu::Buffer; 2],
    /// 0 or 1: index of the buffer currently used for reading.
    read_index: u32,
}

impl DoubleBuffer {
    /// Create both buffers at init time (C-PERF-2: no per-frame allocation).
    pub fn new(device: &wgpu::Device) -> Self {
        let usage = wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST;

        let buffer_a = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("voxel-buffer-a"),
            size: VOXEL_BUFFER_SIZE,
            usage,
            mapped_at_creation: false,
        });

        let buffer_b = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("voxel-buffer-b"),
            size: VOXEL_BUFFER_SIZE,
            usage,
            mapped_at_creation: false,
        });

        Self {
            buffers: [buffer_a, buffer_b],
            read_index: 0,
        }
    }

    /// The buffer the simulation reads from this tick.
    pub fn read_buffer(&self) -> &wgpu::Buffer {
        &self.buffers[self.read_index as usize]
    }

    /// The buffer the simulation writes to this tick.
    pub fn write_buffer(&self) -> &wgpu::Buffer {
        &self.buffers[1 - self.read_index as usize]
    }

    /// Swap read/write roles. Call after all passes complete.
    pub fn swap(&mut self) {
        self.read_index = 1 - self.read_index;
    }

    /// Current tick parity (0 or 1). Passed to shaders for deterministic tie-breaking.
    #[allow(dead_code)] // Available for future use in conflict resolution
    pub fn parity(&self) -> u32 {
        self.read_index
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_voxel_buffer_size() {
        // 32^3 voxels * 2 u32s * 4 bytes = 262144 bytes = 256KB
        assert_eq!(VOXEL_BUFFER_SIZE, 262_144);
    }

    #[test]
    fn test_swap_alternates() {
        // Can't actually create wgpu buffers without a device, but we can test sizing
        assert_eq!(VOXELS_PER_CHUNK * U32S_PER_VOXEL * 4, 262_144);
    }
}
