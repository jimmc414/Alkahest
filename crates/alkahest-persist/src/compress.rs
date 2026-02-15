use crate::error::PersistError;
use crate::format::{CHUNK_DATA_SIZE, FILL_FLAG};

/// Compress a 256KB chunk using LZ4.
pub fn compress_chunk(data: &[u8]) -> Vec<u8> {
    lz4_flex::compress_prepend_size(data)
}

/// Decompress an LZ4-compressed chunk, validating the output size.
pub fn decompress_chunk(compressed: &[u8]) -> Result<Vec<u8>, PersistError> {
    let decompressed = lz4_flex::decompress_size_prepended(compressed)
        .map_err(|e| PersistError::DecompressError(e.to_string()))?;

    if decompressed.len() != CHUNK_DATA_SIZE {
        return Err(PersistError::InvalidChunkSize {
            expected: CHUNK_DATA_SIZE,
            actual: decompressed.len(),
        });
    }

    Ok(decompressed)
}

/// Check if all voxels in a chunk are identical (single-material fill).
/// Returns the material_id (low u16 of the first voxel) if all voxels match.
pub fn detect_fill(data: &[u8]) -> Option<u16> {
    if data.len() != CHUNK_DATA_SIZE {
        return None;
    }

    // Each voxel is 8 bytes (two u32). Check if all 8-byte blocks are identical.
    let first_voxel = &data[0..8];
    for i in (8..data.len()).step_by(8) {
        if data[i..i + 8] != *first_voxel {
            return None;
        }
    }

    // Extract material_id from bits [0:15] of the low u32
    let low = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let material_id = (low & 0xFFFF) as u16;
    Some(material_id)
}

/// Encode a fill marker: 4 bytes = (material_id: u16, FILL_FLAG: u16).
pub fn encode_fill(material_id: u16) -> [u8; 4] {
    let mut buf = [0u8; 4];
    buf[0..2].copy_from_slice(&material_id.to_le_bytes());
    buf[2..4].copy_from_slice(&FILL_FLAG.to_le_bytes());
    buf
}

/// Check if a compressed data block is a fill marker (exactly 4 bytes with FILL_FLAG).
pub fn is_fill(data: &[u8]) -> bool {
    if data.len() != 4 {
        return false;
    }
    let flag = u16::from_le_bytes([data[2], data[3]]);
    flag == FILL_FLAG
}

/// Expand a 4-byte fill marker back to a full 256KB chunk.
pub fn expand_fill(data: &[u8]) -> Result<Vec<u8>, PersistError> {
    if data.len() != 4 {
        return Err(PersistError::InvalidFillChunk);
    }

    let material_id = u16::from_le_bytes([data[0], data[1]]);

    // Build a single voxel: material_id in low u16, rest zero (air-like defaults)
    let low = material_id as u32;
    let high = 0u32;
    let voxel_bytes_low = low.to_le_bytes();
    let voxel_bytes_high = high.to_le_bytes();

    let mut chunk = vec![0u8; CHUNK_DATA_SIZE];
    for i in (0..CHUNK_DATA_SIZE).step_by(8) {
        chunk[i..i + 4].copy_from_slice(&voxel_bytes_low);
        chunk[i + 4..i + 8].copy_from_slice(&voxel_bytes_high);
    }

    Ok(chunk)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compress_decompress_roundtrip() {
        // Create 256KB of semi-random data
        let mut data = vec![0u8; CHUNK_DATA_SIZE];
        for (i, byte) in data.iter_mut().enumerate() {
            *byte = (i % 251) as u8;
        }

        let compressed = compress_chunk(&data);
        let decompressed = decompress_chunk(&compressed).expect("decompress should succeed");
        assert_eq!(data, decompressed);
    }

    #[test]
    fn test_fill_detection_all_air() {
        // All zeros = air (material_id 0)
        let data = vec![0u8; CHUNK_DATA_SIZE];
        assert_eq!(detect_fill(&data), Some(0));
    }

    #[test]
    fn test_fill_detection_mixed() {
        let mut data = vec![0u8; CHUNK_DATA_SIZE];
        // Set second voxel to a different value
        data[8] = 1;
        assert_eq!(detect_fill(&data), None);
    }

    #[test]
    fn test_fill_encode_decode() {
        let material_id = 42u16;
        let encoded = encode_fill(material_id);
        assert_eq!(encoded.len(), 4);
        assert!(is_fill(&encoded));

        let expanded = expand_fill(&encoded).expect("expand should succeed");
        assert_eq!(expanded.len(), CHUNK_DATA_SIZE);

        // Check that all voxels have material_id 42
        for i in (0..CHUNK_DATA_SIZE).step_by(8) {
            let low = u32::from_le_bytes([
                expanded[i],
                expanded[i + 1],
                expanded[i + 2],
                expanded[i + 3],
            ]);
            assert_eq!((low & 0xFFFF) as u16, material_id);
        }
    }

    #[test]
    fn test_is_fill_rejects_non_fill() {
        assert!(!is_fill(&[0, 0, 0, 0])); // flag != FILL_FLAG
        assert!(!is_fill(&[0, 0, 0xFF, 0xFF, 0])); // wrong length
        assert!(!is_fill(&[0, 0])); // too short
    }

    #[test]
    fn test_compressed_size_sanity() {
        // All-zero data should compress very well
        let data = vec![0u8; CHUNK_DATA_SIZE];
        let compressed = compress_chunk(&data);
        assert!(
            compressed.len() < CHUNK_DATA_SIZE / 10,
            "all-zero data should compress to <10% of original (got {} bytes)",
            compressed.len()
        );
    }
}
