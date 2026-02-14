use crate::camera::Camera;
use crate::gpu::GpuContext;
use crate::input::InputState;
use crate::tools::{self, ToolState};
use crate::ui::debug::DebugPanel;
use crate::ui::UiState;
use alkahest_render::Renderer;
use alkahest_sim::pipeline::SimPipeline;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;

type RafClosure = Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>>;

/// Main application struct. Owns all subsystems.
pub struct Application {
    gpu: GpuContext,
    renderer: Renderer,
    sim: SimPipeline,
    camera: Camera,
    input_state: Rc<RefCell<InputState>>,
    ui_state: UiState,
    debug_panel: DebugPanel,
    tool_state: ToolState,
    last_frame_time: f64,
}

impl Application {
    pub fn new(gpu: GpuContext, dpi_scale: f32, input_state: Rc<RefCell<InputState>>) -> Self {
        let ui_state = UiState::new(&gpu.device, gpu.surface_format, dpi_scale);

        let width = gpu.surface_config.width;
        let height = gpu.surface_config.height;
        let renderer = Renderer::new(&gpu.device, &gpu.queue, gpu.surface_format, width, height);

        let sim = SimPipeline::new(&gpu.device, &gpu.queue);

        let debug_panel = DebugPanel::new(gpu.adapter_name.clone(), gpu.backend.clone());
        let tool_state = ToolState::new();
        let camera = Camera::new();

        Self {
            gpu,
            renderer,
            sim,
            camera,
            input_state,
            ui_state,
            debug_panel,
            tool_state,
            last_frame_time: 0.0,
        }
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

    /// Render a single frame.
    fn render_frame(&mut self) {
        // Destructure self for disjoint field borrows
        let Application {
            gpu,
            renderer,
            sim,
            camera,
            input_state,
            ui_state,
            debug_panel,
            tool_state,
            ..
        } = self;

        let width = gpu.surface_config.width;
        let height = gpu.surface_config.height;

        // 1. Read input and update camera + handle sim controls
        {
            let mut input = input_state.borrow_mut();

            // Simulation controls
            if input.was_just_pressed(" ") {
                sim.toggle_pause();
            }
            if input.was_just_pressed(".") {
                sim.single_step();
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

                // Right-click: place/remove voxels
                // Simple fixed-position placement for M2 (no raycasting yet)
                if input.right_button_down {
                    let target = camera.target;
                    let x = target.x as i32;
                    let y = (target.y + 1.0) as i32;
                    let z = target.z as i32;
                    if input.shift_down {
                        tools::remove::execute(sim, x, y, z);
                    } else {
                        tools::place::execute(sim, x, y, z, tool_state.place_material);
                    }
                }
            }

            input.clear_deltas();
        }

        // 2. Upload camera uniforms
        let cam_uniforms = camera.to_uniforms(width, height);
        renderer.update_camera(&gpu.queue, cam_uniforms);

        // Upload debug line view-projection matrix
        let vp = camera.view_proj(width as f32, height as f32);
        renderer.update_debug_uniforms(&gpu.queue, vp.to_cols_array_2d());

        // Update debug panel camera info
        let eye = camera.eye_position();
        debug_panel.set_camera_info(eye.into(), camera.target.into());

        // Update debug panel sim info
        debug_panel.set_sim_info(sim.tick_count(), sim.is_paused());

        // 3. Get surface texture, handle Lost by reconfiguring
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

        // 4. Run egui frame (before GPU encoding)
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

        // 5. Create command encoder
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame-encoder"),
            });

        // 6. Upload sim commands and dispatch simulation tick
        sim.upload_commands(&gpu.queue);
        sim.tick(&gpu.device, &gpu.queue, &mut encoder);

        // 7. Bind the sim's read buffer to the renderer (update bind group)
        // We do this every frame because the read buffer alternates after each tick.
        renderer.update_voxel_buffer(&gpu.device, sim.get_read_buffer());

        // 8. Compute ray march + blit + debug lines
        renderer.render(&mut encoder, &surface_view, width, height);

        // 9. Upload egui textures and buffers
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

        // 10. egui render pass with LoadOp::Load (C-EGUI-2: after scene)
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

        // 11. Free textures after rendering
        for id in &full_output.textures_delta.free {
            ui_state.renderer.free_texture(id);
        }

        // 12. Submit and present
        gpu.queue.submit(std::iter::once(encoder.finish()));
        output.present();
    }
}
