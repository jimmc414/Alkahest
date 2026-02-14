// blit.wgsl — Fullscreen triangle to copy compute output to the surface.
// Uses the 3-vertex fullscreen triangle trick (no vertex buffer needed).
// Bind group: render texture + nearest-neighbor sampler.

@group(0) @binding(0) var render_tex: texture_2d<f32>;
@group(0) @binding(1) var render_sampler: sampler;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    // Generate fullscreen triangle from vertex_index (0, 1, 2)
    let x = f32(i32(vertex_index & 1u) * 4 - 1);
    let y = f32(i32(vertex_index >> 1u) * 4 - 1);
    var out: VertexOutput;
    out.position = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(render_tex, render_sampler, in.uv);
}
