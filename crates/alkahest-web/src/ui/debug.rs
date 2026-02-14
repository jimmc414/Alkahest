/// Debug panel displaying adapter info and frame timing (C-PERF-5: pre-allocated strings).
pub struct DebugPanel {
    adapter_name: String,
    backend: String,
    frame_times: [f64; 60],
    frame_index: usize,
    avg_frame_time_ms: f64,
}

impl DebugPanel {
    pub fn new(adapter_name: String, backend: String) -> Self {
        Self {
            adapter_name,
            backend,
            frame_times: [0.0; 60],
            frame_index: 0,
            avg_frame_time_ms: 0.0,
        }
    }

    /// Record a frame's delta time and update rolling average.
    pub fn update(&mut self, delta_ms: f64) {
        self.frame_times[self.frame_index] = delta_ms;
        self.frame_index = (self.frame_index + 1) % 60;
        let sum: f64 = self.frame_times.iter().sum();
        self.avg_frame_time_ms = sum / 60.0;
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
            });
    }
}
