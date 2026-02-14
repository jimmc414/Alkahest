use alkahest_core::constants::CHUNK_SLEEP_TICKS;
use alkahest_core::types::ChunkCoord;

/// Chunk lifecycle state per architecture.md Section 4.2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChunkState {
    /// Not loaded; no GPU resources allocated.
    Unloaded,
    /// Actively simulated every tick.
    Active,
    /// No voxel changes for CHUNK_SLEEP_TICKS ticks; excluded from dispatch.
    Static,
}

/// Per-chunk metadata managed on the CPU.
#[derive(Debug, Clone)]
pub struct Chunk {
    /// Chunk coordinate in chunk-space.
    pub coord: ChunkCoord,
    /// Current lifecycle state.
    pub state: ChunkState,
    /// Pool slot index (offset into pool buffer). None if Unloaded.
    pub pool_slot: Option<u32>,
    /// Consecutive ticks with no activity (for sleep transition).
    pub idle_ticks: u32,
    /// Whether this chunk contains any non-air voxels.
    pub has_non_air: bool,
    /// Index in the current frame's dispatch list (None if not dispatched).
    pub dispatch_index: Option<u32>,
}

impl Chunk {
    /// Create a new chunk in Active state with an assigned pool slot.
    pub fn new_active(coord: ChunkCoord, pool_slot: u32, has_non_air: bool) -> Self {
        Self {
            coord,
            state: ChunkState::Active,
            pool_slot: Some(pool_slot),
            idle_ticks: 0,
            has_non_air,
            dispatch_index: None,
        }
    }

    /// Create an unloaded chunk (no pool slot).
    pub fn new_unloaded(coord: ChunkCoord) -> Self {
        Self {
            coord,
            state: ChunkState::Unloaded,
            pool_slot: None,
            idle_ticks: 0,
            has_non_air: false,
            dispatch_index: None,
        }
    }

    /// Mark this chunk as active (resets idle counter).
    pub fn activate(&mut self) {
        self.state = ChunkState::Active;
        self.idle_ticks = 0;
    }

    /// Record one idle tick. Returns true if chunk should transition to Static.
    pub fn tick_idle(&mut self) -> bool {
        self.idle_ticks += 1;
        self.idle_ticks >= CHUNK_SLEEP_TICKS
    }

    /// Transition to Static (sleeping).
    pub fn sleep(&mut self) {
        self.state = ChunkState::Static;
    }

    /// Whether this chunk should be included in the simulation dispatch.
    pub fn is_dispatched(&self) -> bool {
        self.state == ChunkState::Active
    }
}
