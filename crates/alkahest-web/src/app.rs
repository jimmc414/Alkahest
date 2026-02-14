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
    "Air", "Stone", "Sand", "Water", "Oil", "Fire", "Smoke", "Steam", "Wood", "Ash", "Ice", "Lava",
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
        let combustion_ron = include_str!("../../../data/rules/combustion.ron");

        let materials =
            alkahest_rules::loader::load_all_materials(&[naturals_ron, organics_ron, energy_ron])
                .expect("failed to parse material RON data");

        let rules = alkahest_rules::loader::load_all_rules(&[combustion_ron])
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
            ..
        } = self;

        let width = gpu.surface_config.width;
        let height = gpu.surface_config.height;

        // 1. Poll GPU readback from previous frame (C-GPU-8: non-blocking)
        let dispatch_list = world.update(camera.target);

        if let Some(flags) = sim.poll_readback(&gpu.device, dispatch_list.len() as u32) {
            world.process_activity(&flags);
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

                // Right-click: place/remove voxels (world-space → local + dispatch idx)
                if input.right_button_down {
                    let target = camera.target;
                    let wx = target.x as i32;
                    let wy = (target.y + 1.0) as i32;
                    let wz = target.z as i32;
                    if let Some((cdi, lx, ly, lz)) =
                        Self::world_to_dispatch_coords(wx, wy, wz, &dispatch_list)
                    {
                        if input.shift_down {
                            tools::remove::execute(sim, lx, ly, lz, cdi);
                        } else {
                            tools::place::execute(sim, lx, ly, lz, tool_state.place_material, cdi);
                        }
                    }
                }

                // H key: heat tool (apply to voxel at camera target)
                if input.keys_down.contains("h") {
                    let target = camera.target;
                    let wx = target.x as i32;
                    let wy = (target.y + 1.0) as i32;
                    let wz = target.z as i32;
                    if let Some((cdi, lx, ly, lz)) =
                        Self::world_to_dispatch_coords(wx, wy, wz, &dispatch_list)
                    {
                        tools::heat::execute_heat(
                            sim,
                            lx,
                            ly,
                            lz,
                            alkahest_core::constants::TOOL_HEAT_DELTA,
                            cdi,
                        );
                    }
                }

                // F key: freeze tool (apply to voxel at camera target)
                if input.keys_down.contains("f") {
                    let target = camera.target;
                    let wx = target.x as i32;
                    let wy = (target.y + 1.0) as i32;
                    let wz = target.z as i32;
                    if let Some((cdi, lx, ly, lz)) =
                        Self::world_to_dispatch_coords(wx, wy, wz, &dispatch_list)
                    {
                        tools::heat::execute_heat(
                            sim,
                            lx,
                            ly,
                            lz,
                            alkahest_core::constants::TOOL_FREEZE_DELTA,
                            cdi,
                        );
                    }
                }
            }

            input.clear_deltas();
        }

        // 3. Upload camera uniforms
        let cam_uniforms = camera.to_uniforms(width, height, *render_mode);
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

        // 7. Upload chunk descriptors and sim commands, dispatch simulation tick
        sim.upload_chunk_descriptors(&gpu.queue, &descriptor_data);
        sim.upload_commands(&gpu.queue);
        sim.tick(
            &gpu.device,
            &gpu.queue,
            &mut encoder,
            active_chunk_count,
            &active_slots,
        );

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
