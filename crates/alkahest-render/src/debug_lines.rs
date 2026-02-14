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

/// Generate wireframe vertices for a cube brush preview.
/// `center` is the world-space brush center, `radius` is the brush radius.
pub fn cube_wireframe(center: [f32; 3], radius: f32) -> Vec<DebugVertex> {
    let color = [1.0, 1.0, 0.3, 0.8]; // yellow
    let r = radius + 0.5; // extend to voxel edges
    let cx = center[0];
    let cy = center[1];
    let cz = center[2];

    let corners: [[f32; 3]; 8] = [
        [cx - r, cy - r, cz - r],
        [cx + r, cy - r, cz - r],
        [cx + r, cy + r, cz - r],
        [cx - r, cy + r, cz - r],
        [cx - r, cy - r, cz + r],
        [cx + r, cy - r, cz + r],
        [cx + r, cy + r, cz + r],
        [cx - r, cy + r, cz + r],
    ];

    let edges: [(usize, usize); 12] = [
        (0, 1),
        (1, 2),
        (2, 3),
        (3, 0),
        (4, 5),
        (5, 6),
        (6, 7),
        (7, 4),
        (0, 4),
        (1, 5),
        (2, 6),
        (3, 7),
    ];

    let mut verts = Vec::with_capacity(24);
    for (a, b) in edges {
        verts.push(DebugVertex::new(corners[a], color));
        verts.push(DebugVertex::new(corners[b], color));
    }
    verts
}

/// Generate wireframe vertices for a sphere brush preview.
/// Draws 3 circles (XY, XZ, YZ planes) approximated with line segments.
pub fn sphere_wireframe(center: [f32; 3], radius: f32) -> Vec<DebugVertex> {
    let color = [1.0, 1.0, 0.3, 0.8]; // yellow
    let r = radius + 0.5;
    let segments = 24u32;
    let cx = center[0];
    let cy = center[1];
    let cz = center[2];

    let mut verts = Vec::with_capacity((segments as usize) * 2 * 3);

    for ring in 0..3 {
        for i in 0..segments {
            let a0 = (i as f32) / (segments as f32) * std::f32::consts::TAU;
            let a1 = ((i + 1) as f32) / (segments as f32) * std::f32::consts::TAU;

            let (p0, p1) = match ring {
                0 => (
                    // XY plane
                    [cx + r * a0.cos(), cy + r * a0.sin(), cz],
                    [cx + r * a1.cos(), cy + r * a1.sin(), cz],
                ),
                1 => (
                    // XZ plane
                    [cx + r * a0.cos(), cy, cz + r * a0.sin()],
                    [cx + r * a1.cos(), cy, cz + r * a1.sin()],
                ),
                _ => (
                    // YZ plane
                    [cx, cy + r * a0.cos(), cz + r * a0.sin()],
                    [cx, cy + r * a1.cos(), cz + r * a1.sin()],
                ),
            };

            verts.push(DebugVertex::new(p0, color));
            verts.push(DebugVertex::new(p1, color));
        }
    }
    verts
}
