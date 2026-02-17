use alkahest_core::material::MaterialTable;

/// Material phase category for browsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    All,
    Solid,
    Powder,
    Liquid,
    Gas,
}

/// Material entry for the browser (owned strings, built from MaterialTable).
struct MatEntry {
    id: u32,
    name: String,
    phase: Phase,
}

/// Material browser panel state.
pub struct BrowserState {
    search: String,
    phase_filter: Phase,
    materials: Vec<MatEntry>,
}

impl BrowserState {
    /// Build browser state from loaded MaterialTable.
    pub fn new(table: &MaterialTable) -> Self {
        let mut materials: Vec<MatEntry> = table
            .materials
            .iter()
            .map(|m| MatEntry {
                id: m.id as u32,
                name: m.name.clone(),
                phase: match m.phase {
                    alkahest_core::material::Phase::Gas => Phase::Gas,
                    alkahest_core::material::Phase::Liquid => Phase::Liquid,
                    alkahest_core::material::Phase::Solid => Phase::Solid,
                    alkahest_core::material::Phase::Powder => Phase::Powder,
                },
            })
            .collect();
        materials.sort_by_key(|m| m.id);
        Self {
            search: String::new(),
            phase_filter: Phase::All,
            materials,
        }
    }

    /// Filter materials by search text. Returns matching (id, name) pairs.
    pub fn filter_materials(&self, search: &str) -> Vec<(u32, &str)> {
        let search_lower = search.to_lowercase();
        self.materials
            .iter()
            .filter(|mat| {
                search_lower.is_empty() || mat.name.to_lowercase().contains(&search_lower)
            })
            .map(|mat| (mat.id, mat.name.as_str()))
            .collect()
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
                .max_height(300.0)
                .show(ui, |ui| {
                    for mat in &state.materials {
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

#[cfg(test)]
mod tests {
    use super::*;
    use alkahest_core::material::{MaterialDef, MaterialTable, Phase as MatPhase};

    fn test_table() -> MaterialTable {
        MaterialTable {
            materials: vec![
                MaterialDef {
                    id: 0,
                    name: "Air".into(),
                    phase: MatPhase::Gas,
                    density: 0.0,
                    color: (0.0, 0.0, 0.0),
                    emission: 0.0,
                    flammability: 0.0,
                    ignition_temp: 0.0,
                    decay_rate: 0,
                    decay_threshold: 0,
                    decay_product: 0,
                    viscosity: 0.0,
                    thermal_conductivity: 0.0,
                    phase_change_temp: 0.0,
                    phase_change_product: 0,
                    structural_integrity: 0.0,
                    opacity: None,
                    absorption_rate: 0.0,
                    electrical_conductivity: 0.0,
                    electrical_resistance: 0.0,
                    activation_threshold: 0,
                    charge_emission: 0,
                },
                MaterialDef {
                    id: 1,
                    name: "Stone".into(),
                    phase: MatPhase::Solid,
                    density: 5000.0,
                    color: (0.5, 0.5, 0.55),
                    emission: 0.0,
                    flammability: 0.0,
                    ignition_temp: 0.0,
                    decay_rate: 0,
                    decay_threshold: 0,
                    decay_product: 0,
                    viscosity: 0.0,
                    thermal_conductivity: 0.5,
                    phase_change_temp: 0.0,
                    phase_change_product: 0,
                    structural_integrity: 50.0,
                    opacity: None,
                    absorption_rate: 0.0,
                    electrical_conductivity: 0.0,
                    electrical_resistance: 0.0,
                    activation_threshold: 0,
                    charge_emission: 0,
                },
                MaterialDef {
                    id: 2,
                    name: "Sand".into(),
                    phase: MatPhase::Powder,
                    density: 2500.0,
                    color: (0.76, 0.70, 0.50),
                    emission: 0.0,
                    flammability: 0.0,
                    ignition_temp: 0.0,
                    decay_rate: 0,
                    decay_threshold: 0,
                    decay_product: 0,
                    viscosity: 0.0,
                    thermal_conductivity: 0.3,
                    phase_change_temp: 0.0,
                    phase_change_product: 0,
                    structural_integrity: 0.0,
                    opacity: None,
                    absorption_rate: 0.0,
                    electrical_conductivity: 0.0,
                    electrical_resistance: 0.0,
                    activation_threshold: 0,
                    charge_emission: 0,
                },
                MaterialDef {
                    id: 3,
                    name: "Water".into(),
                    phase: MatPhase::Liquid,
                    density: 1000.0,
                    color: (0.2, 0.4, 0.8),
                    emission: 0.1,
                    flammability: 0.0,
                    ignition_temp: 0.0,
                    decay_rate: 0,
                    decay_threshold: 0,
                    decay_product: 0,
                    viscosity: 0.1,
                    thermal_conductivity: 0.6,
                    phase_change_temp: 373.0,
                    phase_change_product: 7,
                    structural_integrity: 0.0,
                    opacity: Some(0.5),
                    absorption_rate: 0.15,
                    electrical_conductivity: 0.0,
                    electrical_resistance: 0.0,
                    activation_threshold: 0,
                    charge_emission: 0,
                },
            ],
        }
    }

    #[test]
    fn test_browser_search_water() {
        let state = BrowserState::new(&test_table());
        let results = state.filter_materials("wat");
        let names: Vec<&str> = results.iter().map(|(_, n)| *n).collect();
        assert!(names.contains(&"Water"), "Should find Water for 'wat'");
        assert!(!names.contains(&"Stone"), "Should not find Stone for 'wat'");
    }

    #[test]
    fn test_browser_search_empty() {
        let state = BrowserState::new(&test_table());
        let results = state.filter_materials("");
        assert_eq!(results.len(), 4, "Empty search returns all");
    }

    #[test]
    fn test_browser_search_case_insensitive() {
        let state = BrowserState::new(&test_table());
        let results = state.filter_materials("SAND");
        let names: Vec<&str> = results.iter().map(|(_, n)| *n).collect();
        assert!(names.contains(&"Sand"));
    }

    #[test]
    fn test_browser_search_no_results() {
        let state = BrowserState::new(&test_table());
        let results = state.filter_materials("zzzzz");
        assert!(results.is_empty(), "No materials match 'zzzzz'");
    }

    #[test]
    fn test_browser_sorted_by_id() {
        let mut table = test_table();
        // Add in reverse order
        table.materials.reverse();
        let state = BrowserState::new(&table);
        let ids: Vec<u32> = state.materials.iter().map(|m| m.id).collect();
        assert_eq!(ids, vec![0, 1, 2, 3], "Materials should be sorted by ID");
    }
}
