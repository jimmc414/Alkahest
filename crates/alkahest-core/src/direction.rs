use glam::IVec3;

/// One of 26 neighbor directions in a 3D grid (6 faces + 12 edges + 8 corners).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Direction {
    // 6 face neighbors
    Down = 0,
    Up = 1,
    North = 2,
    South = 3,
    East = 4,
    West = 5,
    // 12 edge neighbors
    DownNorth = 6,
    DownSouth = 7,
    DownEast = 8,
    DownWest = 9,
    UpNorth = 10,
    UpSouth = 11,
    UpEast = 12,
    UpWest = 13,
    NorthEast = 14,
    NorthWest = 15,
    SouthEast = 16,
    SouthWest = 17,
    // 8 corner neighbors
    DownNorthEast = 18,
    DownNorthWest = 19,
    DownSouthEast = 20,
    DownSouthWest = 21,
    UpNorthEast = 22,
    UpNorthWest = 23,
    UpSouthEast = 24,
    UpSouthWest = 25,
}

/// All 26 directions.
pub const ALL_DIRECTIONS: [Direction; 26] = [
    Direction::Down,
    Direction::Up,
    Direction::North,
    Direction::South,
    Direction::East,
    Direction::West,
    Direction::DownNorth,
    Direction::DownSouth,
    Direction::DownEast,
    Direction::DownWest,
    Direction::UpNorth,
    Direction::UpSouth,
    Direction::UpEast,
    Direction::UpWest,
    Direction::NorthEast,
    Direction::NorthWest,
    Direction::SouthEast,
    Direction::SouthWest,
    Direction::DownNorthEast,
    Direction::DownNorthWest,
    Direction::DownSouthEast,
    Direction::DownSouthWest,
    Direction::UpNorthEast,
    Direction::UpNorthWest,
    Direction::UpSouthEast,
    Direction::UpSouthWest,
];

/// M2 gravity sub-pass directions in fixed order (C-SIM-2).
/// Down first, then 4 down-diagonal corners.
pub const GRAVITY_DIRECTIONS: [Direction; 5] = [
    Direction::Down,
    Direction::DownNorthWest,
    Direction::DownNorthEast,
    Direction::DownSouthWest,
    Direction::DownSouthEast,
];

/// M3 movement directions: gravity (powder+liquid), lateral (liquid), rise (gas).
/// 14 directions dispatched in this fixed order (C-SIM-2).
pub const MOVEMENT_DIRECTIONS: [Direction; 14] = [
    // Gravity (POWDER + LIQUID fall)
    Direction::Down,
    Direction::DownNorthWest,
    Direction::DownNorthEast,
    Direction::DownSouthWest,
    Direction::DownSouthEast,
    // Lateral (LIQUID flow)
    Direction::North,
    Direction::South,
    Direction::East,
    Direction::West,
    // Rise (GAS ascend)
    Direction::Up,
    Direction::UpNorthWest,
    Direction::UpNorthEast,
    Direction::UpSouthWest,
    Direction::UpSouthEast,
];

impl Direction {
    /// Offset vector for this direction. Y-up convention: Down = (0,-1,0).
    pub fn offset(self) -> IVec3 {
        match self {
            // Faces
            Direction::Down => IVec3::new(0, -1, 0),
            Direction::Up => IVec3::new(0, 1, 0),
            Direction::North => IVec3::new(0, 0, -1),
            Direction::South => IVec3::new(0, 0, 1),
            Direction::East => IVec3::new(1, 0, 0),
            Direction::West => IVec3::new(-1, 0, 0),
            // Edges (down)
            Direction::DownNorth => IVec3::new(0, -1, -1),
            Direction::DownSouth => IVec3::new(0, -1, 1),
            Direction::DownEast => IVec3::new(1, -1, 0),
            Direction::DownWest => IVec3::new(-1, -1, 0),
            // Edges (up)
            Direction::UpNorth => IVec3::new(0, 1, -1),
            Direction::UpSouth => IVec3::new(0, 1, 1),
            Direction::UpEast => IVec3::new(1, 1, 0),
            Direction::UpWest => IVec3::new(-1, 1, 0),
            // Edges (lateral)
            Direction::NorthEast => IVec3::new(1, 0, -1),
            Direction::NorthWest => IVec3::new(-1, 0, -1),
            Direction::SouthEast => IVec3::new(1, 0, 1),
            Direction::SouthWest => IVec3::new(-1, 0, 1),
            // Corners (down)
            Direction::DownNorthEast => IVec3::new(1, -1, -1),
            Direction::DownNorthWest => IVec3::new(-1, -1, -1),
            Direction::DownSouthEast => IVec3::new(1, -1, 1),
            Direction::DownSouthWest => IVec3::new(-1, -1, 1),
            // Corners (up)
            Direction::UpNorthEast => IVec3::new(1, 1, -1),
            Direction::UpNorthWest => IVec3::new(-1, 1, -1),
            Direction::UpSouthEast => IVec3::new(1, 1, 1),
            Direction::UpSouthWest => IVec3::new(-1, 1, 1),
        }
    }

    /// Classification: face, edge, or corner.
    pub fn kind(self) -> DirectionKind {
        match self as u8 {
            0..=5 => DirectionKind::Face,
            6..=17 => DirectionKind::Edge,
            18..=25 => DirectionKind::Corner,
            _ => DirectionKind::Face, // unreachable
        }
    }
}

/// Classification of a direction by how many axes it moves along.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectionKind {
    Face,
    Edge,
    Corner,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_directions_count() {
        assert_eq!(ALL_DIRECTIONS.len(), 26);
    }

    #[test]
    fn test_all_directions_unique() {
        for (i, a) in ALL_DIRECTIONS.iter().enumerate() {
            for (j, b) in ALL_DIRECTIONS.iter().enumerate() {
                if i != j {
                    assert_ne!(
                        a.offset(),
                        b.offset(),
                        "directions {i} and {j} share offset"
                    );
                }
            }
        }
    }

    #[test]
    fn test_down_offset() {
        assert_eq!(Direction::Down.offset(), IVec3::new(0, -1, 0));
    }

    #[test]
    fn test_corner_offset() {
        assert_eq!(Direction::DownSouthWest.offset(), IVec3::new(-1, -1, 1));
    }

    #[test]
    fn test_no_zero_offset() {
        for dir in ALL_DIRECTIONS {
            assert_ne!(dir.offset(), IVec3::ZERO, "{dir:?} has zero offset");
        }
    }

    #[test]
    fn test_direction_kinds() {
        assert_eq!(Direction::Down.kind(), DirectionKind::Face);
        assert_eq!(Direction::DownEast.kind(), DirectionKind::Edge);
        assert_eq!(Direction::DownNorthEast.kind(), DirectionKind::Corner);
    }

    #[test]
    fn test_gravity_directions_order() {
        // First must be straight down
        assert_eq!(GRAVITY_DIRECTIONS[0], Direction::Down);
        // All must have y = -1
        for dir in GRAVITY_DIRECTIONS {
            assert_eq!(dir.offset().y, -1, "{dir:?} is not downward");
        }
    }
}
