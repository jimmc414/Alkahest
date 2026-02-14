use crate::gpu::GpuContext;
use crate::ui::debug::DebugPanel;
use crate::ui::UiState;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;

type RafClosure = Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>>;

/// Main application struct. Owns all subsystems.
pub struct Application {
    gpu: GpuContext,
    ui_state: UiState,
    debug_panel: DebugPanel,
    last_frame_time: f64,
}

impl Application {
    pub fn new(gpu: GpuContext, dpi_scale: f32) -> Self {
        let ui_state = UiState::new(&gpu.device, gpu.surface_format, dpi_scale);
        let debug_panel = DebugPanel::new(gpu.adapter_name.clone(), gpu.backend.clone());

        Self {
            gpu,
            ui_state,
            debug_panel,
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
        // Destructure self for disjoint field borrows — avoids borrow checker
        // conflicts when renderer.render() ties its lifetime to the render pass.
        let Application {
            gpu,
            ui_state,
            debug_panel,
            ..
        } = self;

        // Get surface texture, handle Lost by reconfiguring
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

        let view = output.texture.create_view(&Default::default());

        // Run egui frame first (no encoder needed)
        let screen =
            ui_state.screen_descriptor(gpu.surface_config.width, gpu.surface_config.height);

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

        // GPU work
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame-encoder"),
            });

        // 1. Clear render pass (solid color background)
        {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("clear-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.05,
                            g: 0.05,
                            b: 0.08,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
        }

        // 2. Upload egui textures and update buffers
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

        // 3. egui render pass with LoadOp::Load (C-EGUI-2: after scene pass)
        //    forget_lifetime() shifts the encoder guard from compile-time to run-time,
        //    avoiding borrow checker conflicts between encoder and renderer lifetimes.
        {
            let mut pass = encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("egui-pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
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

        // 4. Free textures after rendering
        for id in &full_output.textures_delta.free {
            ui_state.renderer.free_texture(id);
        }

        // 5. Submit and present
        gpu.queue.submit(std::iter::once(encoder.finish()));
        output.present();
    }
}
