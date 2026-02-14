// ray_march.wgsl — Compute shader for DDA ray marching through a voxel chunk.
// Reads: voxel_buffer (storage), material_colors (storage), camera + light uniforms.
// Writes: output_texture (storage texture, rgba8unorm).
// Workgroup size: 8x8x1 — 64 threads per workgroup, one thread per pixel.

// -- Injected constants: CHUNK_SIZE, VOXELS_PER_CHUNK --
// -- Injected: shaders/common/types.wgsl --
// -- Injected: shaders/common/coords.wgsl --

struct CameraUniforms {
    inv_view_proj: mat4x4<f32>,
    position: vec4<f32>,
    screen_size: vec2<f32>,
    near: f32,
    fov: f32,
}

struct LightUniforms {
    position: vec4<f32>,
    color: vec4<f32>,
    ambient: vec4<f32>,
}

struct MaterialColor {
    color: vec3<f32>,
    emission: f32,
}

@group(0) @binding(0) var<uniform> camera: CameraUniforms;
@group(0) @binding(1) var<uniform> light: LightUniforms;

@group(1) @binding(0) var<storage, read> voxel_buffer: array<vec2<u32>>;
@group(1) @binding(1) var<storage, read> material_colors: array<MaterialColor>;
@group(1) @binding(2) var output_texture: texture_storage_2d<rgba8unorm, write>;

const SKY_COLOR = vec4<f32>(0.05, 0.05, 0.08, 1.0);
const MAX_RAY_STEPS: u32 = 128u;
const MAX_SHADOW_STEPS: u32 = 64u;

// AABB ray intersection. Returns (t_near, t_far). If t_near > t_far, no hit.
fn intersect_aabb(ray_origin: vec3<f32>, ray_dir_inv: vec3<f32>, box_min: vec3<f32>, box_max: vec3<f32>) -> vec2<f32> {
    let t1 = (box_min - ray_origin) * ray_dir_inv;
    let t2 = (box_max - ray_origin) * ray_dir_inv;
    let t_min = min(t1, t2);
    let t_max = max(t1, t2);
    let t_near = max(max(t_min.x, t_min.y), t_min.z);
    let t_far = min(min(t_max.x, t_max.y), t_max.z);
    return vec2<f32>(t_near, t_far);
}

// Sample the voxel at integer position. Returns material ID (0 = air).
fn sample_voxel(pos: vec3<i32>) -> u32 {
    if !in_bounds(pos) {
        return 0u;
    }
    let idx = voxel_index(pos);
    let v = voxel_buffer[idx];
    return unpack_material_id(v);
}

// DDA ray march. Returns hit info: (did_hit, hit_position, normal, material_id).
fn ray_march(ray_origin: vec3<f32>, ray_dir: vec3<f32>) -> vec4<f32> {
    // Inverse direction for AABB test (avoid division by zero with large values)
    let inv_dir = vec3<f32>(
        select(1.0 / ray_dir.x, 1e30, abs(ray_dir.x) < 1e-8),
        select(1.0 / ray_dir.y, 1e30, abs(ray_dir.y) < 1e-8),
        select(1.0 / ray_dir.z, 1e30, abs(ray_dir.z) < 1e-8),
    );

    let chunk_min = vec3<f32>(0.0, 0.0, 0.0);
    let chunk_max = vec3<f32>(f32(CHUNK_SIZE), f32(CHUNK_SIZE), f32(CHUNK_SIZE));

    let aabb_hit = intersect_aabb(ray_origin, inv_dir, chunk_min, chunk_max);
    if aabb_hit.x > aabb_hit.y || aabb_hit.y < 0.0 {
        return vec4<f32>(-1.0, 0.0, 0.0, 0.0); // miss
    }

    // Start position: clamp t to at least a small epsilon past 0
    let t_start = max(aabb_hit.x, 0.001);
    var pos = ray_origin + ray_dir * t_start;

    // Current voxel (integer coordinates)
    var voxel = vec3<i32>(
        i32(floor(pos.x)),
        i32(floor(pos.y)),
        i32(floor(pos.z)),
    );

    // Clamp starting voxel to valid range (edge case: exactly on boundary)
    voxel = clamp(voxel, vec3<i32>(0), vec3<i32>(i32(CHUNK_SIZE) - 1));

    // DDA step direction
    let step = vec3<i32>(
        select(-1, 1, ray_dir.x >= 0.0),
        select(-1, 1, ray_dir.y >= 0.0),
        select(-1, 1, ray_dir.z >= 0.0),
    );

    // Distance along ray to next voxel boundary for each axis
    let next_boundary = vec3<f32>(
        f32(voxel.x) + select(0.0, 1.0, ray_dir.x >= 0.0),
        f32(voxel.y) + select(0.0, 1.0, ray_dir.y >= 0.0),
        f32(voxel.z) + select(0.0, 1.0, ray_dir.z >= 0.0),
    );

    var t_max_axis = (next_boundary - ray_origin) * inv_dir;
    let t_delta = abs(inv_dir);

    // Track which axis was last stepped for face normal
    var last_axis = 0;

    // Check starting voxel first
    let start_mat = sample_voxel(voxel);
    if start_mat != 0u {
        return vec4<f32>(f32(start_mat), f32(voxel.x), f32(voxel.y), f32(voxel.z));
    }

    // DDA traversal
    for (var i = 0u; i < MAX_RAY_STEPS; i++) {
        // Step along the axis with smallest t_max
        if t_max_axis.x < t_max_axis.y {
            if t_max_axis.x < t_max_axis.z {
                voxel.x += step.x;
                t_max_axis.x += t_delta.x;
                last_axis = 0;
            } else {
                voxel.z += step.z;
                t_max_axis.z += t_delta.z;
                last_axis = 2;
            }
        } else {
            if t_max_axis.y < t_max_axis.z {
                voxel.y += step.y;
                t_max_axis.y += t_delta.y;
                last_axis = 1;
            } else {
                voxel.z += step.z;
                t_max_axis.z += t_delta.z;
                last_axis = 2;
            }
        }

        // Out of bounds check
        if !in_bounds(voxel) {
            return vec4<f32>(-1.0, 0.0, 0.0, 0.0);
        }

        let mat = sample_voxel(voxel);
        if mat != 0u {
            // Encode: material in x, hit position in yzw, normal via last_axis
            return vec4<f32>(f32(mat), f32(last_axis), f32(-step[last_axis]), 0.0);
        }
    }

    return vec4<f32>(-1.0, 0.0, 0.0, 0.0); // exceeded max steps
}

// Shadow ray: returns 1.0 if lit, 0.0 if occluded.
fn trace_shadow(origin: vec3<f32>, light_pos: vec3<f32>) -> f32 {
    let to_light = light_pos - origin;
    let light_dist = length(to_light);
    if light_dist < 0.001 {
        return 1.0;
    }
    let ray_dir = to_light / light_dist;

    let inv_dir = vec3<f32>(
        select(1.0 / ray_dir.x, 1e30, abs(ray_dir.x) < 1e-8),
        select(1.0 / ray_dir.y, 1e30, abs(ray_dir.y) < 1e-8),
        select(1.0 / ray_dir.z, 1e30, abs(ray_dir.z) < 1e-8),
    );

    var voxel = vec3<i32>(
        i32(floor(origin.x)),
        i32(floor(origin.y)),
        i32(floor(origin.z)),
    );
    voxel = clamp(voxel, vec3<i32>(0), vec3<i32>(i32(CHUNK_SIZE) - 1));

    let step = vec3<i32>(
        select(-1, 1, ray_dir.x >= 0.0),
        select(-1, 1, ray_dir.y >= 0.0),
        select(-1, 1, ray_dir.z >= 0.0),
    );

    let next_boundary = vec3<f32>(
        f32(voxel.x) + select(0.0, 1.0, ray_dir.x >= 0.0),
        f32(voxel.y) + select(0.0, 1.0, ray_dir.y >= 0.0),
        f32(voxel.z) + select(0.0, 1.0, ray_dir.z >= 0.0),
    );

    var t_max_axis = (next_boundary - origin) * inv_dir;
    let t_delta = abs(inv_dir);
    var t_current = 0.0;

    for (var i = 0u; i < MAX_SHADOW_STEPS; i++) {
        if t_max_axis.x < t_max_axis.y {
            if t_max_axis.x < t_max_axis.z {
                t_current = t_max_axis.x;
                voxel.x += step.x;
                t_max_axis.x += t_delta.x;
            } else {
                t_current = t_max_axis.z;
                voxel.z += step.z;
                t_max_axis.z += t_delta.z;
            }
        } else {
            if t_max_axis.y < t_max_axis.z {
                t_current = t_max_axis.y;
                voxel.y += step.y;
                t_max_axis.y += t_delta.y;
            } else {
                t_current = t_max_axis.z;
                voxel.z += step.z;
                t_max_axis.z += t_delta.z;
            }
        }

        // Past the light? No occlusion.
        if t_current >= light_dist {
            return 1.0;
        }

        if !in_bounds(voxel) {
            return 1.0;
        }

        if sample_voxel(voxel) != 0u {
            return 0.0; // occluded
        }
    }

    return 1.0;
}

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let pixel = vec2<i32>(i32(global_id.x), i32(global_id.y));
    let screen = vec2<i32>(i32(camera.screen_size.x), i32(camera.screen_size.y));

    if pixel.x >= screen.x || pixel.y >= screen.y {
        return;
    }

    // Pixel center in NDC [-1, 1]
    let uv = vec2<f32>(
        (f32(pixel.x) + 0.5) / camera.screen_size.x * 2.0 - 1.0,
        1.0 - (f32(pixel.y) + 0.5) / camera.screen_size.y * 2.0, // flip Y
    );

    // Unproject near and far points through inverse view-projection
    let near_clip = camera.inv_view_proj * vec4<f32>(uv, 0.0, 1.0);
    let far_clip = camera.inv_view_proj * vec4<f32>(uv, 1.0, 1.0);
    let near_world = near_clip.xyz / near_clip.w;
    let far_world = far_clip.xyz / far_clip.w;

    let ray_origin = camera.position.xyz;
    let ray_dir = normalize(far_world - near_world);

    let hit = ray_march(ray_origin, ray_dir);

    if hit.x < 0.0 {
        textureStore(output_texture, vec2<u32>(global_id.xy), SKY_COLOR);
        return;
    }

    let mat_id = u32(hit.x);
    let last_axis = i32(hit.y);
    let step_sign = i32(hit.z);

    // Reconstruct face normal from DDA axis
    var normal = vec3<f32>(0.0, 0.0, 0.0);
    normal[last_axis] = f32(step_sign);

    // Reconstruct hit position by tracing to the exact voxel face
    // We need to re-trace to find the actual hit voxel for position
    let inv_dir_main = vec3<f32>(
        select(1.0 / ray_dir.x, 1e30, abs(ray_dir.x) < 1e-8),
        select(1.0 / ray_dir.y, 1e30, abs(ray_dir.y) < 1e-8),
        select(1.0 / ray_dir.z, 1e30, abs(ray_dir.z) < 1e-8),
    );
    let chunk_min = vec3<f32>(0.0, 0.0, 0.0);
    let chunk_max = vec3<f32>(f32(CHUNK_SIZE), f32(CHUNK_SIZE), f32(CHUNK_SIZE));
    let aabb_hit = intersect_aabb(ray_origin, inv_dir_main, chunk_min, chunk_max);
    let t_start = max(aabb_hit.x, 0.001);
    var trace_pos = ray_origin + ray_dir * t_start;
    var trace_voxel = clamp(
        vec3<i32>(i32(floor(trace_pos.x)), i32(floor(trace_pos.y)), i32(floor(trace_pos.z))),
        vec3<i32>(0),
        vec3<i32>(i32(CHUNK_SIZE) - 1),
    );

    // If the starting voxel is already the hit, use trace_pos
    var hit_pos = trace_pos;
    let start_mat_check = sample_voxel(trace_voxel);
    if start_mat_check == 0u {
        // Step through DDA again to find exact hit T
        let dda_step = vec3<i32>(
            select(-1, 1, ray_dir.x >= 0.0),
            select(-1, 1, ray_dir.y >= 0.0),
            select(-1, 1, ray_dir.z >= 0.0),
        );
        let next_b = vec3<f32>(
            f32(trace_voxel.x) + select(0.0, 1.0, ray_dir.x >= 0.0),
            f32(trace_voxel.y) + select(0.0, 1.0, ray_dir.y >= 0.0),
            f32(trace_voxel.z) + select(0.0, 1.0, ray_dir.z >= 0.0),
        );
        var tm = (next_b - ray_origin) * inv_dir_main;
        let td = abs(inv_dir_main);
        var t_hit = 0.0;

        for (var i = 0u; i < MAX_RAY_STEPS; i++) {
            if tm.x < tm.y {
                if tm.x < tm.z {
                    t_hit = tm.x;
                    trace_voxel.x += dda_step.x;
                    tm.x += td.x;
                } else {
                    t_hit = tm.z;
                    trace_voxel.z += dda_step.z;
                    tm.z += td.z;
                }
            } else {
                if tm.y < tm.z {
                    t_hit = tm.y;
                    trace_voxel.y += dda_step.y;
                    tm.y += td.y;
                } else {
                    t_hit = tm.z;
                    trace_voxel.z += dda_step.z;
                    tm.z += td.z;
                }
            }
            if !in_bounds(trace_voxel) { break; }
            if sample_voxel(trace_voxel) != 0u {
                hit_pos = ray_origin + ray_dir * t_hit;
                break;
            }
        }
    }

    // Material color (C-DESIGN-1: no hardcoded material checks)
    let mat_color = material_colors[mat_id];

    // Lighting
    let surface_pos = hit_pos + normal * 0.001; // bias off surface
    let to_light = light.position.xyz - surface_pos;
    let light_dist = length(to_light);
    let light_dir = to_light / max(light_dist, 0.001);

    let n_dot_l = max(dot(normal, light_dir), 0.0);
    let attenuation = 1.0 / (1.0 + 0.05 * light_dist * light_dist);

    let shadow = trace_shadow(surface_pos, light.position.xyz);

    let diffuse = mat_color.color * light.color.xyz * n_dot_l * attenuation * shadow;
    let ambient_term = mat_color.color * light.ambient.xyz;
    let emission_term = mat_color.color * mat_color.emission;

    let final_color = ambient_term + diffuse + emission_term;
    let clamped = clamp(final_color, vec3<f32>(0.0), vec3<f32>(1.0));

    textureStore(output_texture, vec2<u32>(global_id.xy), vec4<f32>(clamped, 1.0));
}
