use alkahest_core::types::ChunkCoord;
use glam::IVec3;

use crate::compat;
use crate::compress;
use crate::error::PersistError;
use crate::format::*;

/// Parsed save file data ready for world reconstruction.
pub struct SaveData {
    pub header: SaveHeader,
    pub camera: CameraState,
    /// Chunks as (coordinate, decompressed 256KB voxel data).
    pub chunks: Vec<(ChunkCoord, Vec<u8>)>,
    /// Compatibility warnings (e.g., rule hash mismatch).
    pub warnings: Vec<String>,
}

/// Load and parse a save file from raw bytes.
pub fn load(bytes: &[u8], current_rule_hash: u64) -> Result<SaveData, PersistError> {
    // Check minimum size (header only)
    if bytes.len() < HEADER_SIZE {
        return Err(PersistError::FileTooSmall(bytes.len(), HEADER_SIZE));
    }

    // Parse header
    let header: &SaveHeader = bytemuck::from_bytes(&bytes[..HEADER_SIZE]);
    let warnings = compat::validate_header(header, current_rule_hash)?;

    let chunk_count = header.chunk_count as usize;
    let camera = header.camera;

    // Validate file has enough room for chunk table
    let table_end = HEADER_SIZE + chunk_count * CHUNK_TABLE_ENTRY_SIZE;
    if bytes.len() < table_end {
        return Err(PersistError::TruncatedFile {
            expected: table_end,
            actual: bytes.len(),
        });
    }

    // Parse chunk table and decompress each chunk
    let mut chunks = Vec::with_capacity(chunk_count);
    for i in 0..chunk_count {
        let entry_start = HEADER_SIZE + i * CHUNK_TABLE_ENTRY_SIZE;
        let entry = &bytes[entry_start..entry_start + CHUNK_TABLE_ENTRY_SIZE];

        let cx = i16::from_le_bytes([entry[0], entry[1]]);
        let cy = i16::from_le_bytes([entry[2], entry[3]]);
        let cz = i16::from_le_bytes([entry[4], entry[5]]);
        let offset = u64::from_le_bytes(entry[6..14].try_into().expect("8-byte slice")) as usize;
        let size = u32::from_le_bytes(entry[14..18].try_into().expect("4-byte slice")) as usize;

        let coord = IVec3::new(cx as i32, cy as i32, cz as i32);

        // Validate data range
        if offset + size > bytes.len() {
            return Err(PersistError::TruncatedFile {
                expected: offset + size,
                actual: bytes.len(),
            });
        }

        let block = &bytes[offset..offset + size];

        // Decompress or expand fill
        let voxel_data = if compress::is_fill(block) {
            compress::expand_fill(block)?
        } else {
            compress::decompress_chunk(block)?
        };

        chunks.push((coord, voxel_data));
    }

    Ok(SaveData {
        header: *header,
        camera,
        chunks,
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::save::{self, ChunkSnapshot};

    fn default_camera() -> CameraState {
        CameraState {
            mode: 0,
            yaw: 0.5,
            pitch: -0.3,
            target: [64.0, 32.0, 64.0],
            distance: 60.0,
        }
    }

    #[test]
    fn test_save_load_roundtrip() {
        let mut voxel_data = vec![0u8; CHUNK_DATA_SIZE];
        // Write some non-trivial pattern
        for i in 0..voxel_data.len() {
            voxel_data[i] = ((i * 7 + 13) % 256) as u8;
        }

        let chunks = vec![
            ChunkSnapshot {
                coord: IVec3::new(0, 0, 0),
                voxel_data: voxel_data.clone(),
            },
            ChunkSnapshot {
                coord: IVec3::new(1, 2, 3),
                voxel_data: vec![0u8; CHUNK_DATA_SIZE], // all air
            },
        ];

        let camera = default_camera();
        let saved = save::save(&chunks, 0xABCD, 42, 7, camera);
        let loaded = load(&saved, 0xABCD).expect("load should succeed");

        assert_eq!(loaded.chunks.len(), 2);
        assert_eq!(loaded.header.tick_count, 42);
        assert_eq!(loaded.header.world_seed, 7);
        assert!(loaded.warnings.is_empty());

        // First chunk: verify data matches
        assert_eq!(loaded.chunks[0].0, IVec3::new(0, 0, 0));
        assert_eq!(loaded.chunks[0].1, voxel_data);

        // Second chunk: was fill-optimized, should still decompress correctly
        assert_eq!(loaded.chunks[1].0, IVec3::new(1, 2, 3));
        assert_eq!(loaded.chunks[1].1, vec![0u8; CHUNK_DATA_SIZE]);
    }

    #[test]
    fn test_save_load_empty_world() {
        let camera = default_camera();
        let saved = save::save(&[], 0, 0, 0, camera);
        let loaded = load(&saved, 0).expect("load should succeed");
        assert!(loaded.chunks.is_empty());
    }

    #[test]
    fn test_save_load_fill_optimization() {
        let chunks = vec![ChunkSnapshot {
            coord: IVec3::new(0, 0, 0),
            voxel_data: vec![0u8; CHUNK_DATA_SIZE],
        }];

        let camera = default_camera();
        let saved = save::save(&chunks, 0, 0, 0, camera);

        // Fill-optimized: header(64) + table(18) + fill(4) = 86 bytes
        assert_eq!(
            saved.len(),
            HEADER_SIZE + CHUNK_TABLE_ENTRY_SIZE + 4,
            "fill-optimized single air chunk should be very small"
        );

        let loaded = load(&saved, 0).expect("load should succeed");
        assert_eq!(loaded.chunks.len(), 1);
        assert_eq!(loaded.chunks[0].1, vec![0u8; CHUNK_DATA_SIZE]);
    }

    #[test]
    fn test_invalid_magic_rejected() {
        let mut data = vec![0u8; 64];
        data[0..4].copy_from_slice(b"NOPE");
        let result = load(&data, 0);
        assert!(matches!(result, Err(PersistError::InvalidMagic)));
    }

    #[test]
    fn test_truncated_file_rejected() {
        // Valid header but claims 1 chunk with no table data
        let camera = default_camera();
        let mut saved = save::save(&[], 0, 0, 0, camera);
        // Manually set chunk_count to 1 in header
        saved[24..28].copy_from_slice(&1u32.to_le_bytes());
        let result = load(&saved, 0);
        assert!(matches!(result, Err(PersistError::TruncatedFile { .. })));
    }

    #[test]
    fn test_rule_hash_mismatch_warns() {
        let camera = default_camera();
        let saved = save::save(&[], 0xAAAA, 0, 0, camera);
        let loaded = load(&saved, 0xBBBB).expect("should load with warning");
        assert_eq!(loaded.warnings.len(), 1);
        assert!(loaded.warnings[0].contains("Rule set has changed"));
    }

    #[test]
    fn test_file_too_small_rejected() {
        let result = load(&[0u8; 10], 0);
        assert!(matches!(result, Err(PersistError::FileTooSmall(10, 64))));
    }
}
