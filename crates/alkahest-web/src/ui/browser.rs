/// Material phase category for browsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    All,
    Solid,
    Powder,
    Liquid,
    Gas,
}

/// Material entry for the browser.
struct MatEntry {
    id: u32,
    name: &'static str,
    phase: Phase,
}

const MATERIALS: &[MatEntry] = &[
    MatEntry {
        id: 0,
        name: "Air",
        phase: Phase::Gas,
    },
    MatEntry {
        id: 1,
        name: "Stone",
        phase: Phase::Solid,
    },
    MatEntry {
        id: 2,
        name: "Sand",
        phase: Phase::Powder,
    },
    MatEntry {
        id: 3,
        name: "Water",
        phase: Phase::Liquid,
    },
    MatEntry {
        id: 4,
        name: "Oil",
        phase: Phase::Liquid,
    },
    MatEntry {
        id: 5,
        name: "Fire",
        phase: Phase::Gas,
    },
    MatEntry {
        id: 6,
        name: "Smoke",
        phase: Phase::Gas,
    },
    MatEntry {
        id: 7,
        name: "Steam",
        phase: Phase::Gas,
    },
    MatEntry {
        id: 8,
        name: "Wood",
        phase: Phase::Solid,
    },
    MatEntry {
        id: 9,
        name: "Ash",
        phase: Phase::Powder,
    },
    MatEntry {
        id: 10,
        name: "Ice",
        phase: Phase::Solid,
    },
    MatEntry {
        id: 11,
        name: "Lava",
        phase: Phase::Liquid,
    },
    MatEntry {
        id: 12,
        name: "Gunpowder",
        phase: Phase::Powder,
    },
    MatEntry {
        id: 13,
        name: "Sealed-Metal",
        phase: Phase::Solid,
    },
    MatEntry {
        id: 14,
        name: "Glass",
        phase: Phase::Solid,
    },
    MatEntry {
        id: 15,
        name: "Glass Shards",
        phase: Phase::Powder,
    },
];

/// Material browser panel state.
pub struct BrowserState {
    search: String,
    phase_filter: Phase,
}

impl Default for BrowserState {
    fn default() -> Self {
        Self {
            search: String::new(),
            phase_filter: Phase::All,
        }
    }
}

impl BrowserState {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Show the material browser window. Returns Some(material_id) if a material was selected.
pub fn show(ctx: &egui::Context, state: &mut BrowserState, current_material: u32) -> Option<u32> {
    let mut selected = None;

    egui::Window::new("Materials")
        .anchor(egui::Align2::LEFT_BOTTOM, egui::vec2(8.0, -8.0))
        .resizable(false)
        .collapsible(true)
        .default_open(false)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Search:");
                ui.text_edit_singleline(&mut state.search);
            });

            ui.horizontal(|ui| {
                ui.selectable_value(&mut state.phase_filter, Phase::All, "All");
                ui.selectable_value(&mut state.phase_filter, Phase::Solid, "Solid");
                ui.selectable_value(&mut state.phase_filter, Phase::Powder, "Powder");
                ui.selectable_value(&mut state.phase_filter, Phase::Liquid, "Liquid");
                ui.selectable_value(&mut state.phase_filter, Phase::Gas, "Gas");
            });

            ui.separator();

            let search_lower = state.search.to_lowercase();
            egui::ScrollArea::vertical()
                .max_height(200.0)
                .show(ui, |ui| {
                    for mat in MATERIALS {
                        // Filter by phase
                        if state.phase_filter != Phase::All && mat.phase != state.phase_filter {
                            continue;
                        }
                        // Filter by search text
                        if !search_lower.is_empty()
                            && !mat.name.to_lowercase().contains(&search_lower)
                        {
                            continue;
                        }
                        let label = format!("[{}] {}", mat.id, mat.name);
                        let is_selected = mat.id == current_material;
                        if ui.selectable_label(is_selected, &label).clicked() {
                            selected = Some(mat.id);
                        }
                    }
                });
        });

    selected
}
