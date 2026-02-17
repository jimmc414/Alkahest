use alkahest_core::material::MaterialTable;
use alkahest_core::rule::RuleSet;
use std::collections::HashMap;

/// Maps external mod IDs (10000+) to contiguous internal IDs starting after
/// the base material range. This keeps the compiler's dense lookup table compact.
#[derive(Debug, Clone)]
pub struct IdRemap {
    /// External mod ID → internal contiguous ID.
    external_to_internal: HashMap<u16, u16>,
    /// Internal contiguous ID → external mod ID (for save compatibility).
    internal_to_external: HashMap<u16, u16>,
    /// Next available internal ID.
    next_internal_id: u16,
}

impl IdRemap {
    /// Create a new remap starting internal IDs at `base_max_id + 1`.
    pub fn new(base_max_id: u16) -> Self {
        Self {
            external_to_internal: HashMap::new(),
            internal_to_external: HashMap::new(),
            next_internal_id: base_max_id.saturating_add(1),
        }
    }

    /// Remap an external ID to a contiguous internal ID.
    /// Returns the existing mapping if already remapped, otherwise assigns next ID.
    pub fn remap(&mut self, external_id: u16) -> u16 {
        if let Some(&internal) = self.external_to_internal.get(&external_id) {
            return internal;
        }
        let internal = self.next_internal_id;
        self.next_internal_id = self.next_internal_id.saturating_add(1);
        self.external_to_internal.insert(external_id, internal);
        self.internal_to_external.insert(internal, external_id);
        internal
    }

    /// Look up the internal ID for an external ID, if mapped.
    pub fn get_internal(&self, external_id: u16) -> Option<u16> {
        self.external_to_internal.get(&external_id).copied()
    }

    /// Look up the external ID for an internal ID, if mapped.
    pub fn get_external(&self, internal_id: u16) -> Option<u16> {
        self.internal_to_external.get(&internal_id).copied()
    }

    /// Number of remapped IDs.
    pub fn len(&self) -> usize {
        self.external_to_internal.len()
    }

    /// Whether no IDs have been remapped.
    pub fn is_empty(&self) -> bool {
        self.external_to_internal.is_empty()
    }
}

/// Remap all material IDs in a table, including cross-references
/// (decay_product, phase_change_product).
pub fn remap_material_table(table: &mut MaterialTable, remap: &mut IdRemap) {
    for mat in &mut table.materials {
        let new_id = remap.remap(mat.id);
        mat.id = new_id;

        // Remap cross-references: only remap if the referenced ID is in the remap table
        // (i.e., it's a mod ID). Base game IDs (< MOD_ID_START) pass through unchanged.
        if let Some(internal) = remap.get_internal(mat.decay_product) {
            mat.decay_product = internal;
        }
        if let Some(internal) = remap.get_internal(mat.phase_change_product) {
            mat.phase_change_product = internal;
        }
    }
}

/// Remap material IDs in all rules. IDs not in the remap table are left unchanged
/// (base game IDs pass through).
pub fn remap_rule_set(rules: &mut RuleSet, remap: &IdRemap) {
    for rule in &mut rules.rules {
        if let Some(id) = remap.get_internal(rule.input_a) {
            rule.input_a = id;
        }
        if let Some(id) = remap.get_internal(rule.input_b) {
            rule.input_b = id;
        }
        if let Some(id) = remap.get_internal(rule.output_a) {
            rule.output_a = id;
        }
        if let Some(id) = remap.get_internal(rule.output_b) {
            rule.output_b = id;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alkahest_core::material::{MaterialDef, Phase};
    use alkahest_core::rule::InteractionRule;

    fn make_mod_material(id: u16, name: &str) -> MaterialDef {
        MaterialDef {
            id,
            name: name.into(),
            phase: Phase::Solid,
            density: 3000.0,
            color: (0.5, 0.5, 0.5),
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
            structural_integrity: 30.0,
            opacity: None,
            absorption_rate: 0.0,
            electrical_conductivity: 0.0,
            electrical_resistance: 0.0,
            activation_threshold: 0,
            charge_emission: 0,
        }
    }

    #[test]
    fn test_remap_assigns_contiguous_ids() {
        let mut remap = IdRemap::new(249);
        // Non-contiguous external IDs should get contiguous internal IDs
        let id1 = remap.remap(10001);
        let id2 = remap.remap(10005);
        let id3 = remap.remap(10002);
        assert_eq!(id1, 250);
        assert_eq!(id2, 251);
        assert_eq!(id3, 252);
        assert_eq!(remap.len(), 3);
    }

    #[test]
    fn test_remap_idempotent() {
        let mut remap = IdRemap::new(249);
        let id1 = remap.remap(10001);
        let id2 = remap.remap(10001);
        assert_eq!(id1, id2);
        assert_eq!(remap.len(), 1);
    }

    #[test]
    fn test_remap_rules_updates_all_refs() {
        let mut remap = IdRemap::new(249);
        remap.remap(10001); // → 250
        remap.remap(10002); // → 251

        let mut rules = RuleSet {
            rules: vec![InteractionRule {
                name: "ModRule".into(),
                input_a: 10001,
                input_b: 10002,
                output_a: 10001,
                output_b: 0, // base ID, should pass through
                probability: 1.0,
                temp_delta: 0,
                min_temp: 0,
                max_temp: 0,
                pressure_delta: 0,
                min_charge: 0,
                max_charge: 0,
            }],
        };

        remap_rule_set(&mut rules, &remap);
        assert_eq!(rules.rules[0].input_a, 250);
        assert_eq!(rules.rules[0].input_b, 251);
        assert_eq!(rules.rules[0].output_a, 250);
        assert_eq!(rules.rules[0].output_b, 0); // unchanged base ID
    }

    #[test]
    fn test_remap_preserves_base_ids() {
        let remap = IdRemap::new(249);
        // Base IDs are never in the remap table, so they pass through
        let mut rules = RuleSet {
            rules: vec![InteractionRule {
                name: "BaseRule".into(),
                input_a: 5,
                input_b: 8,
                output_a: 5,
                output_b: 9,
                probability: 0.8,
                temp_delta: 200,
                min_temp: 0,
                max_temp: 0,
                pressure_delta: 0,
                min_charge: 0,
                max_charge: 0,
            }],
        };

        remap_rule_set(&mut rules, &remap);
        assert_eq!(rules.rules[0].input_a, 5);
        assert_eq!(rules.rules[0].input_b, 8);
        assert_eq!(rules.rules[0].output_a, 5);
        assert_eq!(rules.rules[0].output_b, 9);
    }

    #[test]
    fn test_remap_cross_references() {
        let mut remap = IdRemap::new(249);
        remap.remap(10001); // → 250
        remap.remap(10002); // → 251

        let mut table = MaterialTable {
            materials: vec![{
                let mut m = make_mod_material(10001, "Crystal");
                m.decay_product = 10002;
                m.phase_change_product = 10002;
                m
            }],
        };

        remap_material_table(&mut table, &mut remap);
        assert_eq!(table.materials[0].id, 250);
        assert_eq!(table.materials[0].decay_product, 251);
        assert_eq!(table.materials[0].phase_change_product, 251);
    }

    #[test]
    fn test_remap_reverse_lookup() {
        let mut remap = IdRemap::new(249);
        remap.remap(10001); // → 250
        assert_eq!(remap.get_external(250), Some(10001));
        assert_eq!(remap.get_internal(10001), Some(250));
        assert_eq!(remap.get_external(5), None);
    }
}
