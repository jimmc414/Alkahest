use alkahest_core::constants::CHUNK_SIZE;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct DebugVertex {
    pub position: [f32; 3],
    pub color: [f32; 4],
}

impl DebugVertex {
    pub fn new(position: [f32; 3], color: [f32; 4]) -> Self {
        Self { position, color }
    }
}

/// Generate wireframe vertices for a chunk boundary (12 edges x 2 verts = 24 verts).
pub fn chunk_wireframe() -> Vec<DebugVertex> {
    let s = CHUNK_SIZE as f32;
    let color = [0.3, 1.0, 0.3, 0.6]; // green, semi-transparent

    // 8 corners of the cube [0, CHUNK_SIZE]^3
    let corners: [[f32; 3]; 8] = [
        [0.0, 0.0, 0.0],
        [s, 0.0, 0.0],
        [s, s, 0.0],
        [0.0, s, 0.0],
        [0.0, 0.0, s],
        [s, 0.0, s],
        [s, s, s],
        [0.0, s, s],
    ];

    // 12 edges as index pairs
    let edges: [(usize, usize); 12] = [
        (0, 1),
        (1, 2),
        (2, 3),
        (3, 0), // bottom face
        (4, 5),
        (5, 6),
        (6, 7),
        (7, 4), // top face
        (0, 4),
        (1, 5),
        (2, 6),
        (3, 7), // verticals
    ];

    let mut verts = Vec::with_capacity(24);
    for (a, b) in edges {
        verts.push(DebugVertex::new(corners[a], color));
        verts.push(DebugVertex::new(corners[b], color));
    }
    verts
}
