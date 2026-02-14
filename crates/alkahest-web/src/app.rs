use crate::camera::Camera;
use crate::gpu::GpuContext;
use crate::input::InputState;
use crate::tools::{self, ToolState};
use crate::ui::debug::DebugPanel;
use crate::ui::UiState;
use alkahest_core::constants::*;
use alkahest_render::{MaterialColor, Renderer};
use alkahest_sim::pipeline::SimPipeline;
use alkahest_world::World;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;

type RafClosure = Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>>;

/// Material names for debug display, indexed by material ID.
const MATERIAL_NAMES: &[&str] = &[
    "Air",
    "Stone",
    "Sand",
    "Water",
    "Oil",
    "Fire",
    "Smoke",
    "Steam",
    "Wood",
    "Ash",
    "Ice",
    "Lava",
    "Gunpowder",
    "Sealed-Metal",
    "Glass",
    "Glass Shards",
];

/// Main application struct. Owns all subsystems.
pub struct Application {
    gpu: GpuContext,
    renderer: Renderer,
    sim: SimPipeline,
    world: World,
    camera: Camera,
    input_state: Rc<RefCell<InputState>>,
    ui_state: UiState,
    debug_panel: DebugPanel,
    tool_state: ToolState,
    last_frame_time: f64,
    /// Render mode: 0 = normal, 1 = heatmap.
    render_mode: u32,
    /// Octree for empty-space skipping in ray march.
    octree: alkahest_render::octree::Octree,
    /// Cross-section clip axis: 0 = off, 1 = X, 2 = Y, 3 = Z.
    clip_axis: u32,
    /// Cross-section clip position in world-space.
    clip_position: f32,
    /// Simulation speed multiplier (0.25 – 4.0).
    sim_speed: f32,
    /// Fractional tick accumulator for variable-rate simulation.
    tick_accumulator: f64,
    /// Latest pick result from GPU readback.
    pick_result: alkahest_render::PickResult,
    /// Frame delta in milliseconds (set by start_loop before render_frame).
    frame_delta_ms: f64,
    /// Material browser UI state.
    browser_state: crate::ui::browser::BrowserState,
}

impl Application {
    pub fn new(gpu: GpuContext, dpi_scale: f32, input_state: Rc<RefCell<InputState>>) -> Self {
        let ui_state = UiState::new(&gpu.device, gpu.surface_format, dpi_scale);

        let width = gpu.surface_config.width;
        let height = gpu.surface_config.height;
        let mut renderer =
            Renderer::new(&gpu.device, &gpu.queue, gpu.surface_format, width, height);

        // Load and compile rule engine data (M3)
        let rule_data = Self::load_rules(&gpu.device);

        // Update renderer material colors from compiled data
        let material_colors: Vec<MaterialColor> = rule_data
            .material_colors
            .iter()
            .map(|c| MaterialColor {
                color: c.color,
                emission: c.emission,
            })
            .collect();
        renderer.update_material_colors(&gpu.device, &material_colors);

        let sim = SimPipeline::new(&gpu.device, &gpu.queue, rule_data);

        // Create world and generate terrain
        let world = World::new();
        let pool_slot_count = sim.pool_slot_count();

        // Adjust world chunk map capacity to match actual GPU pool
        // (World::new allocates with MAX_CHUNK_SLOTS, but GPU may have fewer)
        if pool_slot_count < MAX_CHUNK_SLOTS {
            log::warn!(
                "GPU pool has {} slots (less than MAX_CHUNK_SLOTS={})",
                pool_slot_count,
                MAX_CHUNK_SLOTS
            );
        }

        // Upload terrain data to GPU pool
        for (coord, chunk) in world.chunk_map().iter() {
            if let Some(pool_slot) = chunk.pool_slot {
                let data = world.generate_chunk_data(*coord);
                sim.chunk_pool()
                    .upload_chunk_data_both(&gpu.queue, pool_slot, &data);
            }
        }

        // Build initial chunk map for renderer (grid-indexed: cx + cy*X + cz*X*Y)
        let chunk_map_data = Self::build_renderer_chunk_map(world.chunk_map());
        renderer.update_chunk_map(&gpu.queue, &chunk_map_data);

        // Build initial octree for empty-space skipping
        let mut octree = alkahest_render::octree::Octree::new();
        let chunk_occupancy: Vec<_> = world
            .chunk_map()
            .iter()
            .map(|(coord, chunk)| (*coord, chunk.has_non_air))
            .collect();
        octree.rebuild(&chunk_occupancy);
        renderer.update_octree(&gpu.queue, &octree.gpu_data());
        octree.clear_dirty();

        // Bind sim's read pool to renderer
        renderer.update_voxel_pool(&gpu.device, sim.get_read_pool());

        let debug_panel = DebugPanel::new(gpu.adapter_name.clone(), gpu.backend.clone());
        let tool_state = ToolState::new();

        // Camera centered on the world
        let mut camera = Camera::new();
        let world_center_x = (WORLD_CHUNKS_X * CHUNK_SIZE) as f32 / 2.0;
        let world_center_y = (WORLD_CHUNKS_Y * CHUNK_SIZE) as f32 / 4.0;
        let world_center_z = (WORLD_CHUNKS_Z * CHUNK_SIZE) as f32 / 2.0;
        camera.target = glam::Vec3::new(world_center_x, world_center_y, world_center_z);

        Self {
            gpu,
            renderer,
            sim,
            world,
            camera,
            input_state,
            ui_state,
            debug_panel,
            tool_state,
            last_frame_time: 0.0,
            render_mode: 0,
            octree,
            clip_axis: 0,
            clip_position: 64.0, // middle of world Y
            sim_speed: 1.0,
            tick_accumulator: 0.0,
            pick_result: alkahest_render::PickResult::default(),
            frame_delta_ms: 16.67,
            browser_state: crate::ui::browser::BrowserState::new(),
        }
    }

    /// Build a flat grid-indexed chunk map for the renderer.
    /// Index: cz * WORLD_CHUNKS_X * WORLD_CHUNKS_Y + cy * WORLD_CHUNKS_X + cx
    /// Value: pool_slot * BYTES_PER_CHUNK (byte offset) or SENTINEL_NEIGHBOR
    fn build_renderer_chunk_map(chunk_map: &alkahest_world::chunk_map::ChunkMap) -> Vec<u32> {
        let total = (WORLD_CHUNKS_X * WORLD_CHUNKS_Y * WORLD_CHUNKS_Z) as usize;
        let mut data = vec![SENTINEL_NEIGHBOR; total];

        for (coord, chunk) in chunk_map.iter() {
            if let Some(pool_slot) = chunk.pool_slot {
                let idx = coord.z as u32 * WORLD_CHUNKS_X * WORLD_CHUNKS_Y
                    + coord.y as u32 * WORLD_CHUNKS_X
                    + coord.x as u32;
                if (idx as usize) < total {
                    data[idx as usize] = pool_slot * BYTES_PER_CHUNK;
                }
            }
        }

        data
    }

    /// Load, validate, and compile rule engine data from embedded RON files.
    fn load_rules(device: &wgpu::Device) -> alkahest_rules::GpuRuleData {
        // Embed RON data at compile time (avoids async fetch for M3)
        let naturals_ron = include_str!("../../../data/materials/naturals.ron");
        let organics_ron = include_str!("../../../data/materials/organics.ron");
        let energy_ron = include_str!("../../../data/materials/energy.ron");
        let explosives_ron = include_str!("../../../data/materials/explosives.ron");
        let combustion_ron = include_str!("../../../data/rules/combustion.ron");
        let structural_ron = include_str!("../../../data/rules/structural.ron");

        let materials = alkahest_rules::loader::load_all_materials(&[
            naturals_ron,
            organics_ron,
            energy_ron,
            explosives_ron,
        ])
        .expect("failed to parse material RON data");

        let rules = alkahest_rules::loader::load_all_rules(&[combustion_ron, structural_ron])
            .expect("failed to parse rules RON data");

        // Validate
        if let Err(errors) = alkahest_rules::validator::validate_materials(&materials) {
            for e in &errors {
                log::error!("Material validation error: {e}");
            }
            panic!("Material validation failed with {} errors", errors.len());
        }
        if let Err(errors) = alkahest_rules::validator::validate_rules(&rules, &materials) {
            for e in &errors {
                log::error!("Rule validation error: {e}");
            }
            panic!("Rule validation failed with {} errors", errors.len());
        }

        log::info!(
            "Loaded {} materials and {} rules",
            materials.len(),
            rules.len()
        );

        alkahest_rules::compiler::compile(device, &materials, &rules)
    }

    /// Start the requestAnimationFrame loop.
    /// Creates the rAF closure ONCE (C-RUST-3: no closure leak per frame).
    pub fn start_loop(app: Rc<RefCell<Self>>) {
        let closure: RafClosure = Rc::new(RefCell::new(None));
        let closure_clone = closure.clone();

        let window = web_sys::window().expect("no global window");

        *closure.borrow_mut() = Some(Closure::wrap(Box::new(move |timestamp: f64| {
            let mut app_ref = app.borrow_mut();

            let delta = timestamp - app_ref.last_frame_time;

            // Skip frame if tab was backgrounded (C-BROWSER-5: >100ms gap)
            if app_ref.last_frame_time > 0.0 && delta > 100.0 {
                app_ref.last_frame_time = timestamp;
                // Re-schedule without rendering
                let window = web_sys::window().expect("no global window");
                window
                    .request_animation_frame(
                        closure_clone
                            .borrow()
                            .as_ref()
                            .expect("rAF closure missing")
                            .as_ref()
                            .unchecked_ref(),
                    )
                    .expect("rAF registration failed");
                return;
            }

            app_ref.last_frame_time = timestamp;
            app_ref.frame_delta_ms = delta;
            app_ref.debug_panel.update(delta);
            app_ref.render_frame();

            // Schedule next frame
            let window = web_sys::window().expect("no global window");
            window
                .request_animation_frame(
                    closure_clone
                        .borrow()
                        .as_ref()
                        .expect("rAF closure missing")
                        .as_ref()
                        .unchecked_ref(),
                )
                .expect("rAF registration failed");
        }) as Box<dyn FnMut(f64)>));

        // Kick off first frame
        window
            .request_animation_frame(
                closure
                    .borrow()
                    .as_ref()
                    .expect("rAF closure missing")
                    .as_ref()
                    .unchecked_ref(),
            )
            .expect("rAF registration failed");
    }

    /// Convert world-space voxel coordinates to (chunk_dispatch_idx, local_x, local_y, local_z).
    /// Returns None if the position is outside world bounds or the chunk is not dispatched.
    fn world_to_dispatch_coords(
        wx: i32,
        wy: i32,
        wz: i32,
        dispatch_list: &alkahest_world::dispatch::DispatchList,
    ) -> Option<(u32, i32, i32, i32)> {
        let cs = CHUNK_SIZE as i32;
        let cx = wx.div_euclid(cs);
        let cy = wy.div_euclid(cs);
        let cz = wz.div_euclid(cs);

        // Find this chunk in the dispatch list
        let chunk_coord = glam::IVec3::new(cx, cy, cz);
        for (i, entry) in dispatch_list.entries.iter().enumerate() {
            if entry.coord == chunk_coord {
                let lx = wx.rem_euclid(cs);
                let ly = wy.rem_euclid(cs);
                let lz = wz.rem_euclid(cs);
                return Some((i as u32, lx, ly, lz));
            }
        }
        None
    }

    /// Compute all chunks overlapping a brush centered at world-space (wx, wy, wz) with given radius.
    /// For each overlapping chunk, returns (chunk_dispatch_idx, local_center_x, local_center_y, local_center_z).
    /// The local center may be outside [0,CHUNK_SIZE) — the shader's in_bounds() clips per-voxel.
    fn brush_affected_chunks(
        wx: i32,
        wy: i32,
        wz: i32,
        radius: u32,
        dispatch_list: &alkahest_world::dispatch::DispatchList,
    ) -> Vec<(u32, i32, i32, i32)> {
        let cs = CHUNK_SIZE as i32;
        let r = radius as i32;

        if r == 0 {
            // Single voxel: just the one chunk
            if let Some(result) = Self::world_to_dispatch_coords(wx, wy, wz, dispatch_list) {
                return vec![result];
            }
            return Vec::new();
        }

        // Compute chunk range the brush can touch
        let min_cx = (wx - r).div_euclid(cs);
        let max_cx = (wx + r).div_euclid(cs);
        let min_cy = (wy - r).div_euclid(cs);
        let max_cy = (wy + r).div_euclid(cs);
        let min_cz = (wz - r).div_euclid(cs);
        let max_cz = (wz + r).div_euclid(cs);

        let mut results = Vec::new();
        for cz in min_cz..=max_cz {
            for cy in min_cy..=max_cy {
                for cx in min_cx..=max_cx {
                    let chunk_coord = glam::IVec3::new(cx, cy, cz);
                    for (i, entry) in dispatch_list.entries.iter().enumerate() {
                        if entry.coord == chunk_coord {
                            // Express brush center in this chunk's local space
                            let lx = wx - cx * cs;
                            let ly = wy - cy * cs;
                            let lz = wz - cz * cs;
                            results.push((i as u32, lx, ly, lz));
                            break;
                        }
                    }
                }
            }
        }
        results
    }

    /// Render a single frame.
    fn render_frame(&mut self) {
        // Destructure self for disjoint field borrows
        let Application {
            gpu,
            renderer,
            sim,
            world,
            camera,
            input_state,
            ui_state,
            debug_panel,
            tool_state,
            render_mode,
            octree,
            clip_axis,
            clip_position,
            sim_speed,
            tick_accumulator,
            pick_result,
            frame_delta_ms,
            browser_state,
            ..
        } = self;

        let width = gpu.surface_config.width;
        let height = gpu.surface_config.height;

        // 1. Poll GPU readback from previous frame (C-GPU-8: non-blocking)
        let dispatch_list = world.update(camera.target);

        if let Some(flags) = sim.poll_readback(&gpu.device, dispatch_list.len() as u32) {
            world.process_activity(&flags);
        }

        // Poll pick buffer readback (1–2 frame latency)
        if let Some(result) = renderer.pick.poll_readback(&gpu.device) {
            *pick_result = result;
        }

        // Rebuild dispatch list after activity processing
        let dispatch_list = world.update(camera.target);
        let descriptor_data = dispatch_list.build_descriptor_data();
        let active_chunk_count = dispatch_list.len() as u32;
        let active_slots: Vec<u32> = dispatch_list.entries.iter().map(|e| e.pool_slot).collect();

        // 2. Read input and update camera + handle sim controls
        {
            let mut input = input_state.borrow_mut();

            // Simulation controls
            if input.was_just_pressed(" ") {
                sim.toggle_pause();
            }
            if input.was_just_pressed(".") {
                sim.single_step();
            }

            // Number keys 1-9: select material for placement
            for key in 1..=9u32 {
                let key_str = key.to_string();
                if input.was_just_pressed(&key_str) {
                    tool_state.place_material = key;
                    log::info!(
                        "Selected material: {} ({})",
                        MATERIAL_NAMES.get(key as usize).unwrap_or(&"Unknown"),
                        key
                    );
                }
            }
            // Key 0: select air (erase)
            if input.was_just_pressed("0") {
                tool_state.place_material = 0;
                log::info!("Selected material: Air (0)");
            }

            // Letter keys for M6 materials (12-15)
            let letter_materials: &[(&str, u32)] = &[
                ("g", 12), // Gunpowder
                ("m", 13), // Sealed-Metal
                ("v", 14), // Glass
                ("b", 15), // Glass Shards
            ];
            for &(key, mat_id) in letter_materials {
                if input.was_just_pressed(key) {
                    tool_state.place_material = mat_id;
                    log::info!(
                        "Selected material: {} ({})",
                        MATERIAL_NAMES.get(mat_id as usize).unwrap_or(&"Unknown"),
                        mat_id
                    );
                }
            }

            // T key: toggle heatmap visualization
            if input.was_just_pressed("t") {
                *render_mode = if *render_mode == 0 { 1 } else { 0 };
                log::info!(
                    "Render mode: {}",
                    if *render_mode == 0 {
                        "normal"
                    } else {
                        "heatmap"
                    }
                );
            }

            // Simulation speed: [ to decrease, ] to increase (0.25x – 4.0x)
            if input.was_just_pressed("[") {
                *sim_speed = (*sim_speed - 0.25).max(0.25);
                log::info!("Sim speed: {:.2}x", *sim_speed);
            }
            if input.was_just_pressed("]") {
                *sim_speed = (*sim_speed + 0.25).min(4.0);
                log::info!("Sim speed: {:.2}x", *sim_speed);
            }

            // X key: cycle cross-section clip axis (off → X → Y → Z → off)
            if input.was_just_pressed("x") {
                *clip_axis = (*clip_axis + 1) % 4;
                let axis_name = match *clip_axis {
                    1 => "X",
                    2 => "Y",
                    3 => "Z",
                    _ => "Off",
                };
                log::info!("Cross-section: {}", axis_name);
            }

            // Tool selection: p = Place, e = Remove, j = Push
            // (h/f remain as held-key tools for heat/freeze)
            if input.was_just_pressed("p") {
                tool_state.active = tools::ActiveTool::Place;
                log::info!("Tool: Place");
            }
            if input.was_just_pressed("e") {
                tool_state.active = tools::ActiveTool::Remove;
                log::info!("Tool: Remove");
            }
            if input.was_just_pressed("j") {
                tool_state.active = tools::ActiveTool::Push;
                log::info!("Tool: Push");
            }

            // Brush radius: - to decrease, = to increase (0 – 16)
            if input.was_just_pressed("-") {
                tool_state.brush.decrease_radius();
                log::info!("Brush radius: {}", tool_state.brush.radius);
            }
            if input.was_just_pressed("=") {
                tool_state.brush.increase_radius();
                log::info!("Brush radius: {}", tool_state.brush.radius);
            }

            // Brush shape: Shift+[ to cycle shape
            if input.shift_down && input.was_just_pressed("{") {
                tool_state.brush.shape = tool_state.brush.shape.next();
                log::info!("Brush shape: {}", tool_state.brush.shape.name());
            }

            // Check if egui wants pointer input — if so, suppress camera controls
            if !ui_state.ctx.wants_pointer_input() {
                if input.left_button_down {
                    camera.orbit(input.mouse_dx, input.mouse_dy);
                }
                if input.middle_button_down {
                    camera.pan(input.mouse_dx, input.mouse_dy);
                }
                if input.scroll_delta.abs() > 0.01 {
                    camera.zoom(input.scroll_delta);
                }

                // Right-click: place/remove voxels with multi-chunk brush
                if input.right_button_down {
                    let target = camera.target;
                    let wx = target.x as i32;
                    let wy = (target.y + 1.0) as i32;
                    let wz = target.z as i32;
                    let br = tool_state.brush.radius;
                    let bs = tool_state.brush.shape.as_u32();
                    for (cdi, lx, ly, lz) in
                        Self::brush_affected_chunks(wx, wy, wz, br, &dispatch_list)
                    {
                        if input.shift_down {
                            tools::remove::execute(sim, lx, ly, lz, cdi, br, bs);
                        } else {
                            tools::place::execute(
                                sim,
                                lx,
                                ly,
                                lz,
                                tool_state.place_material,
                                cdi,
                                br,
                                bs,
                            );
                        }
                    }
                }

                // H key: heat tool with brush
                if input.keys_down.contains("h") {
                    let target = camera.target;
                    let wx = target.x as i32;
                    let wy = (target.y + 1.0) as i32;
                    let wz = target.z as i32;
                    let br = tool_state.brush.radius;
                    let bs = tool_state.brush.shape.as_u32();
                    for (cdi, lx, ly, lz) in
                        Self::brush_affected_chunks(wx, wy, wz, br, &dispatch_list)
                    {
                        tools::heat::execute_heat(
                            sim,
                            lx,
                            ly,
                            lz,
                            alkahest_core::constants::TOOL_HEAT_DELTA,
                            cdi,
                            br,
                            bs,
                        );
                    }
                }

                // F key: freeze tool with brush
                if input.keys_down.contains("f") {
                    let target = camera.target;
                    let wx = target.x as i32;
                    let wy = (target.y + 1.0) as i32;
                    let wz = target.z as i32;
                    let br = tool_state.brush.radius;
                    let bs = tool_state.brush.shape.as_u32();
                    for (cdi, lx, ly, lz) in
                        Self::brush_affected_chunks(wx, wy, wz, br, &dispatch_list)
                    {
                        tools::heat::execute_heat(
                            sim,
                            lx,
                            ly,
                            lz,
                            alkahest_core::constants::TOOL_FREEZE_DELTA,
                            cdi,
                            br,
                            bs,
                        );
                    }
                }
            }

            input.clear_deltas();
        }

        // 3. Upload camera uniforms
        let cursor_x = {
            let input = input_state.borrow();
            input.mouse_x as u32
        };
        let cursor_y = {
            let input = input_state.borrow();
            input.mouse_y as u32
        };
        let cam_uniforms = camera.to_uniforms(
            width,
            height,
            *render_mode,
            *clip_axis,
            *clip_position,
            cursor_x,
            cursor_y,
        );
        renderer.update_camera(&gpu.queue, cam_uniforms);

        // Upload debug line view-projection matrix
        let vp = camera.view_proj(width as f32, height as f32);
        renderer.update_debug_uniforms(&gpu.queue, vp.to_cols_array_2d());

        // Update debug panel
        let eye = camera.eye_position();
        debug_panel.set_camera_info(eye.into(), camera.target.into());
        debug_panel.set_sim_info(sim.tick_count(), sim.is_paused());
        let (total, active, static_count) = world.chunk_counts();
        debug_panel.set_chunk_info(total, active, static_count);

        // 4. Get surface texture, handle Lost by reconfiguring
        let output = match gpu.surface.get_current_texture() {
            Ok(output) => output,
            Err(wgpu::SurfaceError::Lost) => {
                gpu.surface.configure(&gpu.device, &gpu.surface_config);
                return;
            }
            Err(wgpu::SurfaceError::OutOfMemory) => {
                log::error!("GPU out of memory");
                return;
            }
            Err(e) => {
                log::error!("Surface error: {e:?}");
                return;
            }
        };

        let surface_view = output.texture.create_view(&Default::default());

        // 5. Run egui frame (before GPU encoding)
        let screen = ui_state.screen_descriptor(width, height);

        let raw_input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(
                    screen.size_in_pixels[0] as f32 / screen.pixels_per_point,
                    screen.size_in_pixels[1] as f32 / screen.pixels_per_point,
                ),
            )),
            ..Default::default()
        };

        let full_output = ui_state.ctx.run(raw_input, |ctx| {
            debug_panel.show(ctx);
            crate::ui::toolbar::show(ctx, tool_state);
            if let Some(mat_id) =
                crate::ui::browser::show(ctx, browser_state, tool_state.place_material)
            {
                tool_state.place_material = mat_id;
            }
            crate::ui::hud::show(ctx, tool_state, MATERIAL_NAMES, *sim_speed, sim.is_paused());
            crate::ui::hover::show(ctx, pick_result, MATERIAL_NAMES);
            crate::ui::settings::show(ctx, clip_axis, clip_position, sim_speed, render_mode);
        });

        let clipped_primitives = ui_state
            .ctx
            .tessellate(full_output.shapes, full_output.pixels_per_point);

        // 6. Create command encoder
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame-encoder"),
            });

        // 7. Upload chunk descriptors and sim commands, dispatch simulation ticks
        //    Tick accumulator: sim_speed controls how many ticks per second.
        //    At 60fps: 0.25x → ~15 ticks/sec, 1x → ~60, 4x → ~240.
        //    Cap at 4 ticks per frame to prevent frame-time explosion.
        let delta_sec = *frame_delta_ms / 1000.0;
        *tick_accumulator += delta_sec * (*sim_speed as f64) * 60.0;
        let mut ticks_this_frame = 0u32;
        while *tick_accumulator >= 1.0 && ticks_this_frame < 4 {
            sim.upload_chunk_descriptors(&gpu.queue, &descriptor_data);
            sim.upload_commands(&gpu.queue);
            sim.tick(
                &gpu.device,
                &gpu.queue,
                &mut encoder,
                active_chunk_count,
                &active_slots,
            );
            *tick_accumulator -= 1.0;
            ticks_this_frame += 1;
        }
        // Ensure at least descriptor upload even if no ticks ran
        if ticks_this_frame == 0 {
            sim.upload_chunk_descriptors(&gpu.queue, &descriptor_data);
            sim.upload_commands(&gpu.queue);
        }

        // 8. Bind the sim's read pool to the renderer (update bind group)
        // We do this every frame because the read buffer alternates after each tick.
        renderer.update_voxel_pool(&gpu.device, sim.get_read_pool());

        // Update renderer chunk map (in case chunks loaded/unloaded)
        let chunk_map_data = Self::build_renderer_chunk_map(world.chunk_map());
        renderer.update_chunk_map(&gpu.queue, &chunk_map_data);

        // Update octree if dirty
        if octree.is_dirty() {
            renderer.update_octree(&gpu.queue, &octree.gpu_data());
            octree.clear_dirty();
        }

        // 9. Compute ray march + blit + debug lines
        renderer.render(&mut encoder, &surface_view, width, height);

        // 9b. Request pick buffer readback (results available next frame)
        renderer.pick.request_readback(&mut encoder);

        // 10. Upload egui textures and buffers
        for (id, delta) in &full_output.textures_delta.set {
            ui_state
                .renderer
                .update_texture(&gpu.device, &gpu.queue, *id, delta);
        }

        ui_state.renderer.update_buffers(
            &gpu.device,
            &gpu.queue,
            &mut encoder,
            &clipped_primitives,
            &screen,
        );

        // 11. egui render pass with LoadOp::Load (C-EGUI-2: after scene)
        //     forget_lifetime() shifts the encoder guard from compile-time to run-time,
        //     avoiding borrow checker conflicts between encoder and renderer lifetimes.
        {
            let mut pass = encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("egui-pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &surface_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                })
                .forget_lifetime();
            ui_state
                .renderer
                .render(&mut pass, &clipped_primitives, &screen);
        }

        // 12. Free textures after rendering
        for id in &full_output.textures_delta.free {
            ui_state.renderer.free_texture(id);
        }

        // 13. Submit and present
        gpu.queue.submit(std::iter::once(encoder.finish()));
        output.present();
    }
}
