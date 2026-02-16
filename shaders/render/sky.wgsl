// sky.wgsl — Procedural sky gradient for ray misses.
// Concatenated into ray_march.wgsl. Reads from light_config uniform.
// Uses squared falloff for natural-looking sky gradient.

fn sky_color(ray_dir: vec3<f32>) -> vec3<f32> {
    let t = clamp(ray_dir.y * 0.5 + 0.5, 0.0, 1.0);
    return mix(light_config.sky_horizon_color, light_config.sky_zenith_color, t * t);
}
