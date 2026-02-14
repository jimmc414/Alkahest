// ray_march.wgsl — Compute shader for two-level DDA ray marching through a multi-chunk voxel world.
// Reads: voxel_pool (storage), material_colors (storage), chunk_map (storage),
//         octree_nodes (storage, reserved), camera + light uniforms.
// Writes: output_texture (storage texture, rgba8unorm).
// Workgroup size: 8x8x1 — 64 threads per workgroup, one thread per pixel.
//
// World dimensions: WORLD_CHUNKS_X * CHUNK_SIZE x WORLD_CHUNKS_Y * CHUNK_SIZE x WORLD_CHUNKS_Z * CHUNK_SIZE
//                   = 256 x 128 x 256 voxels across an 8x4x8 chunk grid.
//
// Outer DDA: steps through chunk-sized cells (8x4x8 grid).
// Inner DDA: steps through 32^3 voxels within a non-empty chunk.

// -- Injected constants: CHUNK_SIZE, VOXELS_PER_CHUNK, WORLD_CHUNKS_X, WORLD_CHUNKS_Y, WORLD_CHUNKS_Z, SENTINEL_NEIGHBOR --
// -- Injected: shaders/common/types.wgsl --
// -- Injected: shaders/common/coords.wgsl --

struct CameraUniforms {
    inv_view_proj: mat4x4<f32>,
    position: vec4<f32>,
    screen_size: vec2<f32>,
    near: f32,
    fov: f32,
    render_mode: u32,
    clip_axis: u32,
    clip_position: u32,
    cursor_packed: u32,
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

// Group 0: uniforms (unchanged)
@group(0) @binding(0) var<uniform> camera: CameraUniforms;
@group(0) @binding(1) var<uniform> light: LightUniforms;

// Group 1: scene data (multi-chunk layout)
@group(1) @binding(0) var<storage, read> voxel_pool: array<vec2<u32>>;
@group(1) @binding(1) var<storage, read> material_colors: array<MaterialColor>;
@group(1) @binding(2) var output_texture: texture_storage_2d<rgba8unorm, write>;
@group(1) @binding(3) var<storage, read> chunk_map: array<u32>;
@group(1) @binding(4) var<storage, read> octree_nodes: array<vec2<u32>>;
@group(1) @binding(5) var<storage, read_write> pick_result: array<u32>;

const SKY_COLOR = vec4<f32>(0.05, 0.05, 0.08, 1.0);
const MAX_RAY_STEPS: u32 = 512u;
const MAX_SHADOW_STEPS: u32 = 256u;

// World dimensions in voxels (derived from chunk grid and chunk size)
const WORLD_VOXELS_X: u32 = WORLD_CHUNKS_X * CHUNK_SIZE;
const WORLD_VOXELS_Y: u32 = WORLD_CHUNKS_Y * CHUNK_SIZE;
const WORLD_VOXELS_Z: u32 = WORLD_CHUNKS_Z * CHUNK_SIZE;

// ─── Coordinate helpers ────────────────────────────────────────────────

/// Convert world-space voxel position to chunk coordinate.
fn world_to_chunk(world_pos: vec3<i32>) -> vec3<i32> {
    return vec3<i32>(
        world_pos.x / i32(CHUNK_SIZE),
        world_pos.y / i32(CHUNK_SIZE),
        world_pos.z / i32(CHUNK_SIZE),
    );
}

/// Convert world-space voxel position to local position within its chunk [0, CHUNK_SIZE).
fn world_to_local(world_pos: vec3<i32>) -> vec3<i32> {
    let cs = i32(CHUNK_SIZE);
    return vec3<i32>(
        ((world_pos.x % cs) + cs) % cs,
        ((world_pos.y % cs) + cs) % cs,
        ((world_pos.z % cs) + cs) % cs,
    );
}

/// Linear index into the chunk_map array from a chunk coordinate.
fn chunk_map_index(chunk_coord: vec3<i32>) -> u32 {
    return u32(chunk_coord.z) * WORLD_CHUNKS_X * WORLD_CHUNKS_Y
         + u32(chunk_coord.y) * WORLD_CHUNKS_X
         + u32(chunk_coord.x);
}

/// Check if a world-space voxel position is within world bounds.
fn in_world_bounds(world_pos: vec3<i32>) -> bool {
    return world_pos.x >= 0 && world_pos.x < i32(WORLD_VOXELS_X)
        && world_pos.y >= 0 && world_pos.y < i32(WORLD_VOXELS_Y)
        && world_pos.z >= 0 && world_pos.z < i32(WORLD_VOXELS_Z);
}

/// Check if a chunk coordinate is within the chunk grid bounds.
fn in_chunk_grid(cc: vec3<i32>) -> bool {
    return cc.x >= 0 && cc.x < i32(WORLD_CHUNKS_X)
        && cc.y >= 0 && cc.y < i32(WORLD_CHUNKS_Y)
        && cc.z >= 0 && cc.z < i32(WORLD_CHUNKS_Z);
}

// ─── Voxel sampling (world-space) ──────────────────────────────────────

/// Sample material ID at a world-space voxel position. Returns 0 (air) for out-of-bounds
/// or unloaded chunks.
fn sample_voxel_world(world_pos: vec3<i32>) -> u32 {
    if !in_world_bounds(world_pos) {
        return 0u;
    }
    let cc = world_to_chunk(world_pos);
    let slot_offset = chunk_map[chunk_map_index(cc)];
    if slot_offset == 0xFFFFFFFFu {
        return 0u;
    }
    let local = world_to_local(world_pos);
    let vi = voxel_index(local);
    return unpack_material_id(voxel_pool[(slot_offset / 8u) + vi]);
}

/// Sample temperature at a world-space voxel position. Returns 0 for out-of-bounds
/// or unloaded chunks.
fn sample_temperature_world(world_pos: vec3<i32>) -> u32 {
    if !in_world_bounds(world_pos) {
        return 0u;
    }
    let cc = world_to_chunk(world_pos);
    let slot_offset = chunk_map[chunk_map_index(cc)];
    if slot_offset == 0xFFFFFFFFu {
        return 0u;
    }
    let local = world_to_local(world_pos);
    let vi = voxel_index(local);
    return unpack_temperature(voxel_pool[(slot_offset / 8u) + vi]);
}

// ─── AABB intersection ─────────────────────────────────────────────────

/// AABB ray intersection. Returns (t_near, t_far). If t_near > t_far, no hit.
fn intersect_aabb(ray_origin: vec3<f32>, ray_dir_inv: vec3<f32>, box_min: vec3<f32>, box_max: vec3<f32>) -> vec2<f32> {
    let t1 = (box_min - ray_origin) * ray_dir_inv;
    let t2 = (box_max - ray_origin) * ray_dir_inv;
    let t_min = min(t1, t2);
    let t_max = max(t1, t2);
    let t_near = max(max(t_min.x, t_min.y), t_min.z);
    let t_far = min(min(t_max.x, t_max.y), t_max.z);
    return vec2<f32>(t_near, t_far);
}

// ─── Heatmap color ─────────────────────────────────────────────────────

/// Convert temperature to heatmap color: blue(cold) -> cyan -> green -> yellow -> red(hot).
fn heatmap_color(temp: u32) -> vec3<f32> {
    // Normalize to [0,1] range over 0-4095 quantized range
    let t = clamp(f32(temp) / 4095.0, 0.0, 1.0);
    // 5-stop gradient: blue -> cyan -> green -> yellow -> red
    if t < 0.25 {
        let f = t / 0.25;
        return mix(vec3<f32>(0.0, 0.0, 1.0), vec3<f32>(0.0, 1.0, 1.0), f);
    } else if t < 0.5 {
        let f = (t - 0.25) / 0.25;
        return mix(vec3<f32>(0.0, 1.0, 1.0), vec3<f32>(0.0, 1.0, 0.0), f);
    } else if t < 0.75 {
        let f = (t - 0.5) / 0.25;
        return mix(vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(1.0, 1.0, 0.0), f);
    } else {
        let f = (t - 0.75) / 0.25;
        return mix(vec3<f32>(1.0, 1.0, 0.0), vec3<f32>(1.0, 0.0, 0.0), f);
    }
}

// ─── Clip plane helper ────────────────────────────────────────────────

/// Check if a world-space voxel position should be clipped by the cross-section plane.
/// clip_axis: 0=off, 1=X, 2=Y, 3=Z. Clips voxels on the positive side of the plane.
fn is_clipped(world_pos: vec3<i32>, clip_axis: u32, clip_pos: f32) -> bool {
    if clip_axis == 0u {
        return false;
    }
    let p = clip_pos;
    if clip_axis == 1u {
        return f32(world_pos.x) >= p;
    } else if clip_axis == 2u {
        return f32(world_pos.y) >= p;
    } else {
        return f32(world_pos.z) >= p;
    }
}

/// Write voxel data to the pick buffer for hover info display.
fn write_pick(world_pos: vec3<i32>, voxel: vec2<u32>) {
    pick_result[0] = u32(world_pos.x);
    pick_result[1] = u32(world_pos.y);
    pick_result[2] = u32(world_pos.z);
    pick_result[3] = unpack_material_id(voxel);
    pick_result[4] = unpack_temperature(voxel);
    pick_result[5] = unpack_pressure(voxel);
    // Pack velocity as 3 biased-128 u8s
    let vx = unpack_vel_x(voxel);
    let vy = unpack_vel_y(voxel);
    let vz = unpack_vel_z(voxel);
    let vx_u8 = u32(vx + 128) & 0xFFu;
    let vy_u8 = u32(vy + 128) & 0xFFu;
    let vz_u8 = u32(vz + 128) & 0xFFu;
    pick_result[6] = vx_u8 | (vy_u8 << 8u) | (vz_u8 << 16u);
    pick_result[7] = unpack_flags(voxel);
}

// ─── Inverse direction helper ──────────────────────────────────────────

/// Compute safe inverse ray direction (avoid division by zero).
fn safe_inv_dir(ray_dir: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        select(1.0 / ray_dir.x, 1e30, abs(ray_dir.x) < 1e-8),
        select(1.0 / ray_dir.y, 1e30, abs(ray_dir.y) < 1e-8),
        select(1.0 / ray_dir.z, 1e30, abs(ray_dir.z) < 1e-8),
    );
}

// ─── Two-level DDA ray march ───────────────────────────────────────────

/// Result of a ray march hit. Encoded as:
///   x = material_id (negative if miss)
///   y = last_axis stepped (0=x, 1=y, 2=z)
///   z = step sign on last_axis (for normal reconstruction)
///   w = t value at hit point (distance along ray)
struct RayHit {
    mat_id: i32,
    last_axis: i32,
    step_sign: i32,
    t_hit: f32,
    hit_voxel: vec3<i32>,
}

/// Two-level DDA ray march through the multi-chunk world.
/// Outer level: DDA through chunk grid (8x4x8).
/// Inner level: DDA through voxels within a non-empty chunk (32^3).
fn ray_march(ray_origin: vec3<f32>, ray_dir: vec3<f32>) -> RayHit {
    var result: RayHit;
    result.mat_id = -1;
    result.last_axis = 0;
    result.step_sign = 0;
    result.t_hit = 0.0;
    result.hit_voxel = vec3<i32>(0, 0, 0);

    let inv_dir = safe_inv_dir(ray_dir);

    // World AABB
    let world_min = vec3<f32>(0.0, 0.0, 0.0);
    let world_max = vec3<f32>(f32(WORLD_VOXELS_X), f32(WORLD_VOXELS_Y), f32(WORLD_VOXELS_Z));

    let aabb_hit = intersect_aabb(ray_origin, inv_dir, world_min, world_max);
    if aabb_hit.x > aabb_hit.y || aabb_hit.y < 0.0 {
        return result; // miss
    }

    // Entry point into the world AABB
    let t_start = max(aabb_hit.x, 0.001);
    var pos = ray_origin + ray_dir * t_start;

    // Clamp entry position to world bounds (handle floating point edge cases)
    pos = clamp(pos, world_min + vec3<f32>(0.0001), world_max - vec3<f32>(0.0001));

    // Current voxel in world space
    var voxel = vec3<i32>(
        i32(floor(pos.x)),
        i32(floor(pos.y)),
        i32(floor(pos.z)),
    );
    voxel = clamp(voxel, vec3<i32>(0), vec3<i32>(i32(WORLD_VOXELS_X) - 1, i32(WORLD_VOXELS_Y) - 1, i32(WORLD_VOXELS_Z) - 1));

    // DDA step direction
    let step = vec3<i32>(
        select(-1, 1, ray_dir.x >= 0.0),
        select(-1, 1, ray_dir.y >= 0.0),
        select(-1, 1, ray_dir.z >= 0.0),
    );

    // DDA t_max and t_delta for voxel-level traversal
    let next_boundary = vec3<f32>(
        f32(voxel.x) + select(0.0, 1.0, ray_dir.x >= 0.0),
        f32(voxel.y) + select(0.0, 1.0, ray_dir.y >= 0.0),
        f32(voxel.z) + select(0.0, 1.0, ray_dir.z >= 0.0),
    );
    var t_max_axis = (next_boundary - ray_origin) * inv_dir;
    let t_delta = abs(inv_dir);

    // Track DDA state
    var last_axis = 0;
    var total_steps = 0u;

    // Check starting voxel
    let start_mat = sample_voxel_world(voxel);
    if start_mat != 0u {
        result.mat_id = i32(start_mat);
        result.last_axis = 0;
        result.step_sign = 0;
        result.t_hit = t_start;
        result.hit_voxel = voxel;
        return result;
    }

    // Determine the current chunk for the starting voxel
    var current_chunk = world_to_chunk(voxel);
    var current_slot_offset = 0xFFFFFFFFu;
    if in_chunk_grid(current_chunk) {
        current_slot_offset = chunk_map[chunk_map_index(current_chunk)];
    }

    // Main DDA loop — two-level traversal
    // We step voxel-by-voxel, but when crossing chunk boundaries we check
    // the chunk_map. If the new chunk is empty/unloaded, we skip ahead to
    // the next chunk boundary by fast-forwarding the DDA.
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

        // Out of world bounds — ray exited the world
        if !in_world_bounds(voxel) {
            return result;
        }

        // Check if we crossed into a new chunk
        let new_chunk = world_to_chunk(voxel);
        if new_chunk.x != current_chunk.x || new_chunk.y != current_chunk.y || new_chunk.z != current_chunk.z {
            current_chunk = new_chunk;
            current_slot_offset = chunk_map[chunk_map_index(current_chunk)];

            // If chunk is empty/unloaded, skip to the far side of this chunk
            if current_slot_offset == 0xFFFFFFFFu {
                // Compute the AABB of this chunk in world-space
                let chunk_min = vec3<f32>(
                    f32(current_chunk.x * i32(CHUNK_SIZE)),
                    f32(current_chunk.y * i32(CHUNK_SIZE)),
                    f32(current_chunk.z * i32(CHUNK_SIZE)),
                );
                let chunk_max = chunk_min + vec3<f32>(f32(CHUNK_SIZE));

                // Find the t value where the ray exits this chunk
                let t_exit = intersect_aabb(ray_origin, inv_dir, chunk_min, chunk_max);

                // Advance to just past the chunk exit
                let t_skip = t_exit.y + 0.001;
                let skip_pos = ray_origin + ray_dir * t_skip;

                // Compute the new voxel at the skip position
                let new_voxel = vec3<i32>(
                    i32(floor(skip_pos.x)),
                    i32(floor(skip_pos.y)),
                    i32(floor(skip_pos.z)),
                );

                // Check if we are still in world bounds after skip
                if !in_world_bounds(new_voxel) {
                    return result;
                }

                // Reset DDA state at the new position
                voxel = clamp(new_voxel, vec3<i32>(0), vec3<i32>(i32(WORLD_VOXELS_X) - 1, i32(WORLD_VOXELS_Y) - 1, i32(WORLD_VOXELS_Z) - 1));
                let new_boundary = vec3<f32>(
                    f32(voxel.x) + select(0.0, 1.0, ray_dir.x >= 0.0),
                    f32(voxel.y) + select(0.0, 1.0, ray_dir.y >= 0.0),
                    f32(voxel.z) + select(0.0, 1.0, ray_dir.z >= 0.0),
                );
                t_max_axis = (new_boundary - ray_origin) * inv_dir;

                // Update current_chunk for the position we skipped to
                current_chunk = world_to_chunk(voxel);
                if in_chunk_grid(current_chunk) {
                    current_slot_offset = chunk_map[chunk_map_index(current_chunk)];
                } else {
                    current_slot_offset = 0xFFFFFFFFu;
                }

                // Check voxel at new position
                if current_slot_offset != 0xFFFFFFFFu {
                    let local = world_to_local(voxel);
                    let vi = voxel_index(local);
                    let mat = unpack_material_id(voxel_pool[(current_slot_offset / 8u) + vi]);
                    if mat != 0u {
                        result.mat_id = i32(mat);
                        result.last_axis = last_axis;
                        result.step_sign = -step[last_axis];
                        result.t_hit = t_skip;
                        result.hit_voxel = voxel;
                        return result;
                    }
                }
                continue;
            }
        }

        // Current chunk is loaded — sample the voxel directly from pool
        let local = world_to_local(voxel);
        let vi = voxel_index(local);
        let mat = unpack_material_id(voxel_pool[(current_slot_offset / 8u) + vi]);
        if mat != 0u {
            // Compute approximate t at hit
            let t_hit_val = select(
                select(t_max_axis.z - t_delta.z, t_max_axis.y - t_delta.y, last_axis == 1),
                t_max_axis.x - t_delta.x,
                last_axis == 0,
            );
            result.mat_id = i32(mat);
            result.last_axis = last_axis;
            result.step_sign = -step[last_axis];
            result.t_hit = t_hit_val;
            result.hit_voxel = voxel;
            return result;
        }
    }

    return result; // exceeded max steps
}

// ─── Shadow ray ────────────────────────────────────────────────────────

/// Shadow ray using sample_voxel_world (simple single-level DDA for shadows).
/// Returns 1.0 if lit, 0.0 if occluded.
fn trace_shadow(origin: vec3<f32>, light_pos: vec3<f32>) -> f32 {
    let to_light = light_pos - origin;
    let light_dist = length(to_light);
    if light_dist < 0.001 {
        return 1.0;
    }
    let ray_dir = to_light / light_dist;

    let inv_dir = safe_inv_dir(ray_dir);

    // World AABB intersection for early exit
    let world_min = vec3<f32>(0.0, 0.0, 0.0);
    let world_max = vec3<f32>(f32(WORLD_VOXELS_X), f32(WORLD_VOXELS_Y), f32(WORLD_VOXELS_Z));
    let aabb_hit = intersect_aabb(origin, inv_dir, world_min, world_max);
    if aabb_hit.x > aabb_hit.y || aabb_hit.y < 0.0 {
        return 1.0; // ray does not intersect world
    }

    var voxel = vec3<i32>(
        i32(floor(origin.x)),
        i32(floor(origin.y)),
        i32(floor(origin.z)),
    );
    voxel = clamp(voxel, vec3<i32>(0), vec3<i32>(i32(WORLD_VOXELS_X) - 1, i32(WORLD_VOXELS_Y) - 1, i32(WORLD_VOXELS_Z) - 1));

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

        // Out of world bounds — no more voxels to check.
        if !in_world_bounds(voxel) {
            return 1.0;
        }

        if sample_voxel_world(voxel) != 0u {
            return 0.0; // occluded
        }
    }

    return 1.0;
}

// ─── Main entry point ──────────────────────────────────────────────────

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

    let clip_pos = bitcast<f32>(camera.clip_position);
    let hit = ray_march(ray_origin, ray_dir);

    // Check if this is the cursor pixel — for pick buffer write
    let cursor_x = camera.cursor_packed & 0xFFFFu;
    let cursor_y = camera.cursor_packed >> 16u;
    let is_cursor_pixel = (u32(pixel.x) == cursor_x) && (u32(pixel.y) == cursor_y);

    if hit.mat_id < 0 {
        // Clear pick on miss
        if is_cursor_pixel {
            pick_result[3] = 0u; // material_id = 0 means no hit
        }
        textureStore(output_texture, vec2<u32>(global_id.xy), SKY_COLOR);
        return;
    }

    // Check clip plane — if hit voxel is clipped, draw sky
    if is_clipped(hit.hit_voxel, camera.clip_axis, clip_pos) {
        if is_cursor_pixel {
            pick_result[3] = 0u;
        }
        textureStore(output_texture, vec2<u32>(global_id.xy), SKY_COLOR);
        return;
    }

    let mat_id = u32(hit.mat_id);

    // Write pick data for cursor pixel
    if is_cursor_pixel {
        // Read full voxel data at hit position
        let cc = world_to_chunk(hit.hit_voxel);
        let slot_offset = chunk_map[chunk_map_index(cc)];
        if slot_offset != 0xFFFFFFFFu {
            let local = world_to_local(hit.hit_voxel);
            let vi = voxel_index(local);
            let voxel = voxel_pool[(slot_offset / 8u) + vi];
            write_pick(hit.hit_voxel, voxel);
        }
    }

    // Reconstruct face normal from DDA axis and step sign
    var normal = vec3<f32>(0.0, 0.0, 0.0);
    // Handle the case where step_sign is 0 (starting voxel hit) — default to Y-up normal
    if hit.step_sign == 0 {
        normal = vec3<f32>(0.0, 1.0, 0.0);
    } else {
        normal[hit.last_axis] = f32(hit.step_sign);
    }

    // Compute hit position from t_hit
    let hit_pos = ray_origin + ray_dir * hit.t_hit;

    // Heatmap mode: render temperature as color gradient
    if camera.render_mode == 1u {
        let temp = sample_temperature_world(hit.hit_voxel);
        let heat_color = heatmap_color(temp);

        // Apply basic shading to heatmap
        let surface_pos_h = hit_pos + normal * 0.001;
        let to_light_h = light.position.xyz - surface_pos_h;
        let light_dist_h = length(to_light_h);
        let light_dir_h = to_light_h / max(light_dist_h, 0.001);
        let n_dot_l_h = max(dot(normal, light_dir_h), 0.0);
        let shading = 0.3 + 0.7 * n_dot_l_h;

        let final_heat = heat_color * shading;
        textureStore(output_texture, vec2<u32>(global_id.xy), vec4<f32>(clamp(final_heat, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0));
        return;
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
