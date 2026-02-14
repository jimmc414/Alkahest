/// Debug panel displaying adapter info, frame timing, simulation state, and camera info
/// (C-PERF-5: pre-allocated strings).
pub struct DebugPanel {
    adapter_name: String,
    backend: String,
    frame_times: [f64; 60],
    frame_index: usize,
    avg_frame_time_ms: f64,
    camera_pos: [f32; 3],
    camera_target: [f32; 3],
    sim_tick: u64,
    sim_paused: bool,
}

impl DebugPanel {
    pub fn new(adapter_name: String, backend: String) -> Self {
        Self {
            adapter_name,
            backend,
            frame_times: [0.0; 60],
            frame_index: 0,
            avg_frame_time_ms: 0.0,
            camera_pos: [0.0; 3],
            camera_target: [0.0; 3],
            sim_tick: 0,
            sim_paused: false,
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

    /// Update simulation tick count and pause state.
    pub fn set_sim_info(&mut self, tick: u64, paused: bool) {
        self.sim_tick = tick;
        self.sim_paused = paused;
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
                let state_str = if self.sim_paused { "PAUSED" } else { "RUNNING" };
                ui.label(format!("Sim: {} | Tick: {}", state_str, self.sim_tick));
                ui.separator();
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
