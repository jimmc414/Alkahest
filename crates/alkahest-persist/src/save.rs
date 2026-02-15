use alkahest_core::types::ChunkCoord;

use crate::compress;
use crate::format::*;

/// A snapshot of one chunk's voxel data for serialization.
pub struct ChunkSnapshot {
    pub coord: ChunkCoord,
    pub voxel_data: Vec<u8>,
}

/// Serialize chunks into the Alkahest save binary format.
///
/// Layout: header (64B) + chunk table (18B × N) + compressed data blocks.
pub fn save(
    chunks: &[ChunkSnapshot],
    rule_hash: u64,
    tick_count: u64,
    world_seed: u32,
    camera: CameraState,
) -> Vec<u8> {
    let chunk_count = chunks.len() as u32;

    // Compress each chunk (detect fill first, then LZ4)
    let mut compressed_blocks: Vec<Vec<u8>> = Vec::with_capacity(chunks.len());
    for chunk in chunks {
        if let Some(material_id) = compress::detect_fill(&chunk.voxel_data) {
            compressed_blocks.push(compress::encode_fill(material_id).to_vec());
        } else {
            compressed_blocks.push(compress::compress_chunk(&chunk.voxel_data));
        }
    }

    // Compute chunk table size and data offsets
    let table_size = chunks.len() * CHUNK_TABLE_ENTRY_SIZE;
    let data_start = HEADER_SIZE + table_size;

    // Build header
    let header = SaveHeader {
        magic: MAGIC,
        version: FORMAT_VERSION,
        _pad0: 0,
        rule_hash,
        tick_count,
        chunk_count,
        world_seed,
        camera,
        _pad1: 0,
    };

    // Calculate total file size
    let total_data_size: usize = compressed_blocks.iter().map(|b| b.len()).sum();
    let total_size = data_start + total_data_size;
    let mut output = Vec::with_capacity(total_size);

    // Write header
    output.extend_from_slice(bytemuck::bytes_of(&header));

    // Write chunk table entries
    let mut current_offset = data_start as u64;
    for (i, chunk) in chunks.iter().enumerate() {
        let block_size = compressed_blocks[i].len() as u32;
        // ChunkTableEntry: cx:i16, cy:i16, cz:i16, offset:u64, size:u32 = 18 bytes
        output.extend_from_slice(&(chunk.coord.x as i16).to_le_bytes());
        output.extend_from_slice(&(chunk.coord.y as i16).to_le_bytes());
        output.extend_from_slice(&(chunk.coord.z as i16).to_le_bytes());
        output.extend_from_slice(&current_offset.to_le_bytes());
        output.extend_from_slice(&block_size.to_le_bytes());
        current_offset += block_size as u64;
    }

    // Write compressed data blocks
    for block in &compressed_blocks {
        output.extend_from_slice(block);
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::IVec3;

    #[test]
    fn test_save_produces_valid_binary() {
        let chunks = vec![ChunkSnapshot {
            coord: IVec3::new(1, 2, 3),
            voxel_data: vec![0u8; CHUNK_DATA_SIZE],
        }];

        let camera = CameraState {
            mode: 0,
            yaw: 0.5,
            pitch: -0.3,
            target: [64.0, 32.0, 64.0],
            distance: 60.0,
        };

        let data = save(&chunks, 0x1234, 100, 42, camera);

        // Check header
        assert_eq!(&data[0..4], b"ALKA");
        let version = u16::from_le_bytes([data[4], data[5]]);
        assert_eq!(version, FORMAT_VERSION);

        let rule_hash = u64::from_le_bytes(data[8..16].try_into().expect("slice"));
        assert_eq!(rule_hash, 0x1234);

        let tick = u64::from_le_bytes(data[16..24].try_into().expect("slice"));
        assert_eq!(tick, 100);
    }

    #[test]
    fn test_save_header_fields_correct() {
        let camera = CameraState {
            mode: 1,
            yaw: 1.0,
            pitch: -0.5,
            target: [10.0, 20.0, 30.0],
            distance: 50.0,
        };

        let data = save(&[], 999, 500, 7, camera);

        // Parse header back
        let header: &SaveHeader = bytemuck::from_bytes(&data[..HEADER_SIZE]);
        assert_eq!(header.magic, MAGIC);
        assert_eq!(header.version, FORMAT_VERSION);
        assert_eq!(header.rule_hash, 999);
        assert_eq!(header.tick_count, 500);
        assert_eq!(header.chunk_count, 0);
        assert_eq!(header.world_seed, 7);
        assert_eq!(header.camera.mode, 1);
    }
}
