use alkahest_core::direction::{GRAVITY_DIRECTIONS, MOVEMENT_DIRECTIONS};

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
/// Retained for backward compatibility with M2 tests; M3+ uses build_movement_schedule.
#[allow(dead_code)]
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

/// Build the M3 movement sub-pass schedule: 14 directions x 2 parities = 28 sub-passes.
///
/// Covers gravity (powder+liquid), lateral flow (liquid), and gas rise.
/// Each direction dispatched with even parity then odd parity for conflict resolution.
pub fn build_movement_schedule() -> Vec<SubPass> {
    let mut schedule = Vec::with_capacity(MOVEMENT_DIRECTIONS.len() * 2);
    for dir in MOVEMENT_DIRECTIONS {
        let offset = dir.offset();
        let direction = [offset.x, offset.y, offset.z];
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
    fn test_movement_schedule_length() {
        let schedule = build_movement_schedule();
        // 14 directions * 2 parities = 28 sub-passes
        assert_eq!(schedule.len(), 28);
    }

    #[test]
    fn test_movement_schedule_parities() {
        let schedule = build_movement_schedule();
        for pair in schedule.chunks(2) {
            assert_eq!(pair[0].parity, 0);
            assert_eq!(pair[1].parity, 1);
        }
    }

    #[test]
    fn test_movement_schedule_has_lateral() {
        let schedule = build_movement_schedule();
        let has_north = schedule.iter().any(|sp| sp.direction == [0, 0, -1]);
        let has_south = schedule.iter().any(|sp| sp.direction == [0, 0, 1]);
        let has_east = schedule.iter().any(|sp| sp.direction == [1, 0, 0]);
        let has_west = schedule.iter().any(|sp| sp.direction == [-1, 0, 0]);
        assert!(has_north, "missing North");
        assert!(has_south, "missing South");
        assert!(has_east, "missing East");
        assert!(has_west, "missing West");
    }

    #[test]
    fn test_movement_schedule_has_rise() {
        let schedule = build_movement_schedule();
        let has_up = schedule.iter().any(|sp| sp.direction == [0, 1, 0]);
        let has_up_nw = schedule.iter().any(|sp| sp.direction == [-1, 1, -1]);
        let has_up_ne = schedule.iter().any(|sp| sp.direction == [1, 1, -1]);
        let has_up_sw = schedule.iter().any(|sp| sp.direction == [-1, 1, 1]);
        let has_up_se = schedule.iter().any(|sp| sp.direction == [1, 1, 1]);
        assert!(has_up, "missing Up");
        assert!(has_up_nw, "missing UpNorthWest");
        assert!(has_up_ne, "missing UpNorthEast");
        assert!(has_up_sw, "missing UpSouthWest");
        assert!(has_up_se, "missing UpSouthEast");
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
