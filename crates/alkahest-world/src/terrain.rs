use alkahest_core::constants::*;
use alkahest_core::types::ChunkCoord;

// Material IDs matching the loaded material table.
const MAT_AIR: u16 = 0;
const MAT_STONE: u16 = 1;
const MAT_SAND: u16 = 2;
const MAT_WATER: u16 = 3;

/// Sea level in world-space voxel Y coordinate.
const SEA_LEVEL: i32 = 8;

/// Terrain generator using 2D simplex noise for heightmap-based terrain.
pub struct TerrainGenerator {
    /// Permutation table for simplex noise (doubled for wrapping).
    perm: [u8; 512],
}

impl TerrainGenerator {
    pub fn new(seed: u64) -> Self {
        let perm = Self::build_permutation(seed);
        Self { perm }
    }

    /// Generate voxel data for a chunk. Returns VOXELS_PER_CHUNK packed [u32; 2] entries.
    ///
    /// Terrain layers:
    /// - Stone fill from y=0 to height-2
    /// - Sand/dirt layer at height-1 and height
    /// - Water at y=SEA_LEVEL where height < SEA_LEVEL
    /// - Air above terrain and water
    pub fn generate_chunk(&self, coord: ChunkCoord) -> Vec<[u32; 2]> {
        let mut data = vec![[0u32; 2]; VOXELS_PER_CHUNK as usize];
        let cs = CHUNK_SIZE as i32;
        let base_x = coord.x * cs;
        let base_y = coord.y * cs;
        let base_z = coord.z * cs;

        for lz in 0..cs {
            for lx in 0..cs {
                let wx = base_x + lx;
                let wz = base_z + lz;

                // 2D simplex noise heightmap (3 octaves, amplitude ~12 voxels)
                let height = self.terrain_height(wx, wz);

                for ly in 0..cs {
                    let wy = base_y + ly;
                    let idx = (lx + ly * cs + lz * cs * cs) as usize;

                    let material_id = if wy < height - 1 {
                        MAT_STONE
                    } else if wy >= height - 1 && wy <= height {
                        MAT_SAND
                    } else if wy <= SEA_LEVEL && height <= SEA_LEVEL {
                        MAT_WATER
                    } else {
                        MAT_AIR
                    };

                    if material_id != MAT_AIR {
                        let temp = AMBIENT_TEMP_QUANTIZED as u32;
                        let low = (material_id as u32) | (temp << 16);
                        data[idx] = [low, 0];
                    }
                    // Air voxels are already [0, 0] from vec initialization
                }
            }
        }

        data
    }

    /// Check if a chunk will contain any non-air voxels.
    pub fn chunk_has_content(&self, coord: ChunkCoord) -> bool {
        let cs = CHUNK_SIZE as i32;
        let base_y = coord.y * cs;
        let top_y = base_y + cs - 1;

        // If the entire chunk is above the maximum possible terrain height and above sea level,
        // it's all air. Max terrain height is ~SEA_LEVEL + 12 (noise amplitude).
        if base_y > SEA_LEVEL + 16 {
            return false;
        }

        // If chunk top is below 0, it's always solid (but that shouldn't happen with y>=0)
        if top_y < 0 {
            return true;
        }

        // Quick check: sample a few columns
        let base_x = coord.x * cs;
        let base_z = coord.z * cs;
        for sx in [0, cs / 2, cs - 1] {
            for sz in [0, cs / 2, cs - 1] {
                let height = self.terrain_height(base_x + sx, base_z + sz);
                // If terrain or water intersects this chunk's Y range
                let effective_top = height.max(SEA_LEVEL);
                if base_y <= effective_top {
                    return true;
                }
            }
        }

        false
    }

    /// Compute terrain height at a world-space (x, z) position using 3-octave simplex noise.
    fn terrain_height(&self, wx: i32, wz: i32) -> i32 {
        let x = wx as f64;
        let z = wz as f64;

        // 3 octaves of 2D simplex noise
        let scale = 0.02;
        let mut h = 0.0f64;
        h += self.simplex2d(x * scale, z * scale) * 8.0;
        h += self.simplex2d(x * scale * 2.0 + 100.0, z * scale * 2.0 + 100.0) * 4.0;
        h += self.simplex2d(x * scale * 4.0 + 200.0, z * scale * 4.0 + 200.0) * 2.0;

        // Center around sea level with slight elevation
        let base_height = SEA_LEVEL as f64 + 4.0 + h;
        base_height.round() as i32
    }

    /// 2D simplex noise. Returns value in [-1, 1].
    fn simplex2d(&self, x: f64, z: f64) -> f64 {
        const F2: f64 = 0.5 * (1.7320508075688772 - 1.0); // (sqrt(3)-1)/2
        const G2: f64 = (3.0 - 1.7320508075688772) / 6.0; // (3-sqrt(3))/6

        let s = (x + z) * F2;
        let i = (x + s).floor();
        let j = (z + s).floor();

        let t = (i + j) * G2;
        let x0 = x - (i - t);
        let y0 = z - (j - t);

        let (i1, j1) = if x0 > y0 { (1.0, 0.0) } else { (0.0, 1.0) };

        let x1 = x0 - i1 + G2;
        let y1 = y0 - j1 + G2;
        let x2 = x0 - 1.0 + 2.0 * G2;
        let y2 = y0 - 1.0 + 2.0 * G2;

        let ii = (i as i32 & 255) as usize;
        let jj = (j as i32 & 255) as usize;

        let gi0 = self.perm[ii + self.perm[jj] as usize] as usize % 12;
        let gi1 = self.perm[ii + i1 as usize + self.perm[jj + j1 as usize] as usize] as usize % 12;
        let gi2 = self.perm[ii + 1 + self.perm[jj + 1] as usize] as usize % 12;

        let n0 = Self::corner_contribution(gi0, x0, y0);
        let n1 = Self::corner_contribution(gi1, x1, y1);
        let n2 = Self::corner_contribution(gi2, x2, y2);

        // Scale to [-1, 1]
        70.0 * (n0 + n1 + n2)
    }

    fn corner_contribution(gi: usize, x: f64, y: f64) -> f64 {
        let t = 0.5 - x * x - y * y;
        if t < 0.0 {
            0.0
        } else {
            let t = t * t;
            t * t * Self::grad2d(gi, x, y)
        }
    }

    fn grad2d(hash: usize, x: f64, y: f64) -> f64 {
        // 12 gradient directions for 2D simplex
        const GRAD: [[f64; 2]; 12] = [
            [1.0, 1.0],
            [-1.0, 1.0],
            [1.0, -1.0],
            [-1.0, -1.0],
            [1.0, 0.0],
            [-1.0, 0.0],
            [0.0, 1.0],
            [0.0, -1.0],
            [1.0, 1.0],
            [-1.0, 1.0],
            [1.0, -1.0],
            [-1.0, -1.0],
        ];
        let g = &GRAD[hash % 12];
        g[0] * x + g[1] * y
    }

    fn build_permutation(seed: u64) -> [u8; 512] {
        let mut p: [u8; 256] = [0; 256];
        for (i, val) in p.iter_mut().enumerate() {
            *val = i as u8;
        }

        // Fisher-Yates shuffle with seed
        let mut rng = seed;
        for i in (1..256).rev() {
            rng = rng
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let j = (rng >> 33) as usize % (i + 1);
            p.swap(i, j);
        }

        let mut perm = [0u8; 512];
        for (i, val) in perm.iter_mut().enumerate() {
            *val = p[i & 255];
        }
        perm
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::IVec3;

    #[test]
    fn test_terrain_deterministic() {
        let gen = TerrainGenerator::new(42);
        let coord = IVec3::new(0, 0, 0);
        let data1 = gen.generate_chunk(coord);
        let data2 = gen.generate_chunk(coord);
        assert_eq!(data1, data2, "terrain generation must be deterministic");
    }

    #[test]
    fn test_terrain_chunk_size() {
        let gen = TerrainGenerator::new(42);
        let data = gen.generate_chunk(IVec3::new(0, 0, 0));
        assert_eq!(data.len(), VOXELS_PER_CHUNK as usize);
    }

    #[test]
    fn test_terrain_generates_stone_sand_water() {
        let gen = TerrainGenerator::new(42);

        // Bottom chunk (y=0) should have stone
        let bottom = gen.generate_chunk(IVec3::new(4, 0, 4));
        let mut has_stone = false;
        let mut has_sand = false;
        for voxel in &bottom {
            let mat = voxel[0] & 0xFFFF;
            if mat == MAT_STONE as u32 {
                has_stone = true;
            }
            if mat == MAT_SAND as u32 {
                has_sand = true;
            }
        }
        assert!(has_stone, "bottom chunk should contain stone");
        assert!(has_sand, "bottom chunk should contain sand surface");

        // Check for water somewhere in the world
        let mut has_water = false;
        'outer: for cx in 0..WORLD_CHUNKS_X as i32 {
            for cz in 0..WORLD_CHUNKS_Z as i32 {
                let data = gen.generate_chunk(IVec3::new(cx, 0, cz));
                for voxel in &data {
                    if voxel[0] & 0xFFFF == MAT_WATER as u32 {
                        has_water = true;
                        break 'outer;
                    }
                }
            }
        }
        assert!(has_water, "world should contain water at low elevations");
    }

    #[test]
    fn test_air_chunk_above_terrain() {
        let gen = TerrainGenerator::new(42);
        let coord = IVec3::new(0, (WORLD_CHUNKS_Y - 1) as i32, 0);
        let data = gen.generate_chunk(coord);
        for voxel in &data {
            let mat = voxel[0] & 0xFFFF;
            assert_eq!(mat, 0, "top chunk should be all air");
        }
    }

    #[test]
    fn test_chunk_has_content() {
        let gen = TerrainGenerator::new(42);
        // Bottom chunk definitely has content
        assert!(gen.chunk_has_content(IVec3::new(0, 0, 0)));
        // Top chunk should be empty
        assert!(!gen.chunk_has_content(IVec3::new(0, (WORLD_CHUNKS_Y - 1) as i32, 0)));
    }
}
