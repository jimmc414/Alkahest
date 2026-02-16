// blit.wgsl — Fullscreen triangle to copy compute output to the surface.
// M10: Adds Reinhard tone mapping + sRGB gamma correction for HDR rendering.
// Uses the 3-vertex fullscreen triangle trick (no vertex buffer needed).
// Bind group: render texture (rgba16float HDR) + nearest-neighbor sampler.

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
    var color = textureSample(render_tex, render_sampler, in.uv).rgb;

    // Reinhard tone mapping (HDR -> LDR)
    color = color / (color + vec3(1.0));

    // sRGB gamma correction (linear -> sRGB)
    color = pow(color, vec3(1.0 / 2.2));

    return vec4<f32>(color, 1.0);
}
