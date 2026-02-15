use alkahest_core::constants::BYTES_PER_CHUNK;

/// Magic bytes identifying an Alkahest save file.
pub const MAGIC: [u8; 4] = *b"ALKA";

/// Current save format version.
pub const FORMAT_VERSION: u16 = 1;

/// Size of the file header in bytes.
pub const HEADER_SIZE: usize = 64;

/// Size of each chunk table entry in bytes.
pub const CHUNK_TABLE_ENTRY_SIZE: usize = 18;

/// Marker flag indicating a single-material fill chunk (stored in 4 bytes).
pub const FILL_FLAG: u16 = 0xFFFF;

/// Expected decompressed chunk size in bytes.
pub const CHUNK_DATA_SIZE: usize = BYTES_PER_CHUNK as usize;

/// Camera state stored in the save file header.
///
/// 28 bytes, repr(C) for deterministic layout.
/// mode: 0 = Orbit, 1 = FirstPerson
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraState {
    pub mode: u32,
    pub yaw: f32,
    pub pitch: f32,
    pub target: [f32; 3],
    pub distance: f32,
}

/// Save file header. Fixed 64 bytes, repr(C) for byte-level serialization.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SaveHeader {
    pub magic: [u8; 4],
    pub version: u16,
    pub _pad0: u16,
    pub rule_hash: u64,
    pub tick_count: u64,
    pub chunk_count: u32,
    pub world_seed: u32,
    pub camera: CameraState,
    pub _pad1: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header_size() {
        assert_eq!(std::mem::size_of::<SaveHeader>(), 64);
    }

    #[test]
    fn test_camera_state_size() {
        assert_eq!(std::mem::size_of::<CameraState>(), 28);
    }
}
