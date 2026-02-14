pub mod debug;

use egui_wgpu::ScreenDescriptor;

/// Manages egui context and its wgpu renderer.
pub struct UiState {
    pub ctx: egui::Context,
    pub renderer: egui_wgpu::Renderer,
}

impl UiState {
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat, dpi_scale: f32) -> Self {
        let ctx = egui::Context::default();
        ctx.set_pixels_per_point(dpi_scale);

        let renderer = egui_wgpu::Renderer::new(device, surface_format, None, 1, false);

        Self { ctx, renderer }
    }

    pub fn screen_descriptor(&self, width: u32, height: u32) -> ScreenDescriptor {
        ScreenDescriptor {
            size_in_pixels: [width, height],
            pixels_per_point: self.ctx.pixels_per_point(),
        }
    }
}
