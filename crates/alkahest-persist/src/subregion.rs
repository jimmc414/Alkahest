use alkahest_core::types::ChunkCoord;

use crate::format::CameraState;
use crate::save::{self, ChunkSnapshot};

/// Export a subregion of the world defined by a bounding box of chunk coordinates.
///
/// Filters chunks to only those within [bbox_min, bbox_max] inclusive, then delegates to save.
pub fn export_subregion(
    all_chunks: &[ChunkSnapshot],
    bbox_min: ChunkCoord,
    bbox_max: ChunkCoord,
    rule_hash: u64,
    tick_count: u64,
    world_seed: u32,
    camera: CameraState,
) -> Vec<u8> {
    let filtered: Vec<ChunkSnapshot> = all_chunks
        .iter()
        .filter(|c| {
            c.coord.x >= bbox_min.x
                && c.coord.x <= bbox_max.x
                && c.coord.y >= bbox_min.y
                && c.coord.y <= bbox_max.y
                && c.coord.z >= bbox_min.z
                && c.coord.z <= bbox_max.z
        })
        .map(|c| ChunkSnapshot {
            coord: c.coord,
            voxel_data: c.voxel_data.clone(),
        })
        .collect();

    save::save(&filtered, rule_hash, tick_count, world_seed, camera)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::CHUNK_DATA_SIZE;
    use crate::load;
    use glam::IVec3;

    fn default_camera() -> CameraState {
        CameraState {
            mode: 0,
            yaw: 0.0,
            pitch: 0.0,
            target: [0.0; 3],
            distance: 50.0,
        }
    }

    #[test]
    fn test_subregion_filters_correctly() {
        let chunks = vec![
            ChunkSnapshot {
                coord: IVec3::new(0, 0, 0),
                voxel_data: vec![0u8; CHUNK_DATA_SIZE],
            },
            ChunkSnapshot {
                coord: IVec3::new(1, 0, 0),
                voxel_data: vec![0u8; CHUNK_DATA_SIZE],
            },
            ChunkSnapshot {
                coord: IVec3::new(5, 5, 5),
                voxel_data: vec![0u8; CHUNK_DATA_SIZE],
            },
        ];

        let saved = export_subregion(
            &chunks,
            IVec3::new(0, 0, 0),
            IVec3::new(2, 2, 2),
            0,
            0,
            0,
            default_camera(),
        );

        let loaded = load::load(&saved, 0).expect("should load");
        // Only chunks (0,0,0) and (1,0,0) are in the bbox; (5,5,5) is excluded
        assert_eq!(loaded.chunks.len(), 2);

        let coords: Vec<_> = loaded.chunks.iter().map(|(c, _)| *c).collect();
        assert!(coords.contains(&IVec3::new(0, 0, 0)));
        assert!(coords.contains(&IVec3::new(1, 0, 0)));
        assert!(!coords.contains(&IVec3::new(5, 5, 5)));
    }

    #[test]
    fn test_subregion_output_is_loadable() {
        let chunks = vec![ChunkSnapshot {
            coord: IVec3::new(2, 1, 3),
            voxel_data: vec![0u8; CHUNK_DATA_SIZE],
        }];

        let saved = export_subregion(
            &chunks,
            IVec3::new(0, 0, 0),
            IVec3::new(10, 10, 10),
            42,
            100,
            7,
            default_camera(),
        );

        let loaded = load::load(&saved, 42).expect("should load");
        assert_eq!(loaded.chunks.len(), 1);
        assert_eq!(loaded.header.rule_hash, 42);
    }
}
