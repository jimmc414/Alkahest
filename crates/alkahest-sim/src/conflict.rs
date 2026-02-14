use alkahest_core::direction::GRAVITY_DIRECTIONS;

/// A single sub-pass in the movement dispatch schedule.
///
/// Each sub-pass processes voxels moving in one direction,
/// filtered by checkerboard parity so no two processed voxels
/// target the same destination cell.
#[derive(Debug, Clone, Copy)]
pub struct SubPass {
    /// The movement direction offset as [x, y, z].
    pub direction: [i32; 3],
    /// Checkerboard parity: 0 = even cells (x+z even), 1 = odd cells (x+z odd).
    pub parity: u32,
}

/// Build the complete M2 movement sub-pass schedule (C-SIM-2: fixed order every tick).
///
/// For each gravity direction, dispatch twice: even parity then odd parity.
/// This ensures no two simultaneously-processed voxels can target the same cell.
pub fn build_gravity_schedule() -> Vec<SubPass> {
    let mut schedule = Vec::with_capacity(GRAVITY_DIRECTIONS.len() * 2);
    for dir in GRAVITY_DIRECTIONS {
        let offset = dir.offset();
        let direction = [offset.x, offset.y, offset.z];
        // Even parity first, then odd
        schedule.push(SubPass {
            direction,
            parity: 0,
        });
        schedule.push(SubPass {
            direction,
            parity: 1,
        });
    }
    schedule
}

/// GPU-uploadable uniform for a movement sub-pass.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MovementUniforms {
    /// Direction offset (x, y, z) as i32.
    pub direction: [i32; 3],
    /// Checkerboard parity (0 or 1).
    pub parity: u32,
    /// Current simulation tick number.
    pub tick: u32,
    /// Padding to 32 bytes (aligned for WebGPU uniform requirements).
    pub _pad: [u32; 3],
}

#[cfg(test)]
mod tests {
    use super::*;
    use alkahest_core::direction::Direction;

    #[test]
    fn test_gravity_schedule_length() {
        let schedule = build_gravity_schedule();
        // 5 directions * 2 parities = 10 sub-passes
        assert_eq!(schedule.len(), 10);
    }

    #[test]
    fn test_gravity_schedule_parities_alternate() {
        let schedule = build_gravity_schedule();
        for pair in schedule.chunks(2) {
            assert_eq!(pair[0].parity, 0);
            assert_eq!(pair[1].parity, 1);
        }
    }

    #[test]
    fn test_gravity_schedule_first_is_down() {
        let schedule = build_gravity_schedule();
        assert_eq!(schedule[0].direction, [0, -1, 0]);
        assert_eq!(schedule[1].direction, [0, -1, 0]);
    }

    #[test]
    fn test_all_gravity_directions_go_down() {
        let schedule = build_gravity_schedule();
        for sp in &schedule {
            assert_eq!(
                sp.direction[1], -1,
                "sub-pass direction {:?} does not go down",
                sp.direction
            );
        }
    }

    #[test]
    fn test_movement_uniforms_size() {
        // Must be a multiple of 16 for WebGPU uniform alignment
        assert_eq!(std::mem::size_of::<MovementUniforms>(), 32);
        assert_eq!(std::mem::size_of::<MovementUniforms>() % 16, 0);
    }

    #[test]
    fn test_checkerboard_no_conflict() {
        // For the Down direction (0,-1,0): source (x,y,z) targets (x,y-1,z).
        // With parity=0 filtering x+z even, no two sources share a target.
        let dir = Direction::Down.offset();
        for parity in 0..2u32 {
            let mut targets = std::collections::HashSet::new();
            for x in 0i32..8 {
                for z in 0i32..8 {
                    if ((x + z) as u32 % 2) != parity {
                        continue;
                    }
                    for y in 1i32..8 {
                        let target = (x + dir.x, y + dir.y, z + dir.z);
                        assert!(
                            targets.insert(target),
                            "conflict at target {target:?} in parity {parity}"
                        );
                    }
                }
            }
        }
    }
}
