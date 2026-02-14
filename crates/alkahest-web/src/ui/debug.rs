/// Debug panel displaying adapter info, frame timing, voxel count, and camera info
/// (C-PERF-5: pre-allocated strings).
pub struct DebugPanel {
    adapter_name: String,
    backend: String,
    frame_times: [f64; 60],
    frame_index: usize,
    avg_frame_time_ms: f64,
    voxel_count: u32,
    camera_pos: [f32; 3],
    camera_target: [f32; 3],
}

impl DebugPanel {
    pub fn new(adapter_name: String, backend: String, voxel_count: u32) -> Self {
        Self {
            adapter_name,
            backend,
            frame_times: [0.0; 60],
            frame_index: 0,
            avg_frame_time_ms: 0.0,
            voxel_count,
            camera_pos: [0.0; 3],
            camera_target: [0.0; 3],
        }
    }

    /// Record a frame's delta time and update rolling average.
    pub fn update(&mut self, delta_ms: f64) {
        self.frame_times[self.frame_index] = delta_ms;
        self.frame_index = (self.frame_index + 1) % 60;
        let sum: f64 = self.frame_times.iter().sum();
        self.avg_frame_time_ms = sum / 60.0;
    }

    /// Update camera position and target for display.
    pub fn set_camera_info(&mut self, pos: [f32; 3], target: [f32; 3]) {
        self.camera_pos = pos;
        self.camera_target = target;
    }

    /// Render the debug panel using egui.
    pub fn show(&self, ctx: &egui::Context) {
        egui::Window::new("Debug")
            .default_open(true)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label(&self.adapter_name);
                ui.label(&self.backend);
                ui.separator();
                let fps = if self.avg_frame_time_ms > 0.0 {
                    1000.0 / self.avg_frame_time_ms
                } else {
                    0.0
                };
                ui.label(format!("{:.2} ms", self.avg_frame_time_ms));
                ui.label(format!("{:.0} FPS", fps));
                ui.separator();
                ui.label(format!("Voxels: {}", self.voxel_count));
                ui.label(format!(
                    "Cam: ({:.1}, {:.1}, {:.1})",
                    self.camera_pos[0], self.camera_pos[1], self.camera_pos[2]
                ));
                ui.label(format!(
                    "Target: ({:.1}, {:.1}, {:.1})",
                    self.camera_target[0], self.camera_target[1], self.camera_target[2]
                ));
            });
    }
}
