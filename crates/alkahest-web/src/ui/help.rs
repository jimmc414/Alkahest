/// Help overlay window listing all keybindings.
pub fn show(ctx: &egui::Context, open: &mut bool) {
    if !*open {
        return;
    }

    egui::Window::new("Keyboard Shortcuts")
        .open(open)
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            egui::Grid::new("shortcuts-grid")
                .striped(true)
                .show(ui, |ui| {
                    let bindings: &[(&str, &str)] = &[
                        // Camera
                        ("Left Drag", "Orbit camera"),
                        ("Middle Drag", "Pan camera"),
                        ("Scroll", "Zoom"),
                        ("Tab", "Toggle first-person / orbit"),
                        ("WASD", "Move (first-person)"),
                        ("Space / Ctrl", "Up / Down (first-person)"),
                        // Tools
                        ("Right Click", "Place / Remove voxels"),
                        ("Shift+Right Click", "Remove voxels"),
                        ("H (hold)", "Heat tool"),
                        ("F (hold)", "Freeze tool"),
                        ("P", "Select Place tool"),
                        ("E", "Select Remove tool"),
                        ("J", "Select Push tool"),
                        // Brush
                        ("- / =", "Decrease / Increase brush radius"),
                        ("Shift+{", "Cycle brush shape"),
                        // Materials
                        ("1-9, 0", "Select material by number"),
                        ("G", "Gunpowder"),
                        ("M", "Sealed-Metal"),
                        ("V", "Glass"),
                        ("B", "Glass Shards"),
                        // Simulation
                        ("Space", "Pause / Unpause simulation"),
                        (".", "Single step"),
                        ("[ / ]", "Decrease / Increase sim speed"),
                        // Display
                        ("T", "Toggle heatmap"),
                        ("X", "Cycle cross-section axis"),
                        ("? / F1", "Toggle this help"),
                    ];

                    for (key, action) in bindings {
                        ui.strong(*key);
                        ui.label(*action);
                        ui.end_row();
                    }
                });
        });
}
