use alkahest_core::material::{MaterialDef, MaterialTable};
use alkahest_core::mod_manifest::ModManifest;
use alkahest_core::rule::{InteractionRule, RuleSet};
use thiserror::Error;

use crate::migration::{remap_material_table, remap_rule_set, IdRemap};

#[derive(Debug, Error)]
pub enum LoadError {
    #[error("Failed to parse materials RON: {0}")]
    MaterialParseError(String),
    #[error("Failed to parse rules RON: {0}")]
    RuleParseError(String),
    #[error("Failed to parse mod manifest RON: {0}")]
    ManifestParseError(String),
}

/// Parse a single materials RON string into a MaterialTable.
pub fn load_materials_from_str(ron_str: &str) -> Result<MaterialTable, LoadError> {
    let options = ron::Options::default();
    let materials: Vec<MaterialDef> = options
        .from_str(ron_str)
        .map_err(|e| LoadError::MaterialParseError(e.to_string()))?;
    Ok(MaterialTable { materials })
}

/// Parse a single rules RON string into a RuleSet.
pub fn load_rules_from_str(ron_str: &str) -> Result<RuleSet, LoadError> {
    let options = ron::Options::default();
    let rules: Vec<InteractionRule> = options
        .from_str(ron_str)
        .map_err(|e| LoadError::RuleParseError(e.to_string()))?;
    Ok(RuleSet { rules })
}

/// Load and merge multiple material sources into a single MaterialTable.
pub fn load_all_materials(sources: &[&str]) -> Result<MaterialTable, LoadError> {
    let mut all_materials = Vec::new();
    for source in sources {
        let table = load_materials_from_str(source)?;
        all_materials.extend(table.materials);
    }
    Ok(MaterialTable {
        materials: all_materials,
    })
}

/// Load and merge multiple rule sources into a single RuleSet.
pub fn load_all_rules(sources: &[&str]) -> Result<RuleSet, LoadError> {
    let mut all_rules = Vec::new();
    for source in sources {
        let set = load_rules_from_str(source)?;
        all_rules.extend(set.rules);
    }
    Ok(RuleSet { rules: all_rules })
}

/// Parse a mod manifest from a RON string.
pub fn load_mod_manifest(ron_str: &str) -> Result<ModManifest, LoadError> {
    let options = ron::Options::default();
    options
        .from_str(ron_str)
        .map_err(|e| LoadError::ManifestParseError(e.to_string()))
}

/// Result of loading a single mod pack.
#[derive(Debug)]
pub struct ModLoadResult {
    pub manifest: ModManifest,
    pub materials: MaterialTable,
    pub rules: RuleSet,
    pub warnings: Vec<String>,
}

/// Load a complete mod pack from its manifest and data sources.
pub fn load_mod(
    manifest_str: &str,
    material_sources: &[&str],
    rule_sources: &[&str],
) -> Result<ModLoadResult, LoadError> {
    let manifest = load_mod_manifest(manifest_str)?;
    let materials = load_all_materials(material_sources)?;
    let rules = load_all_rules(rule_sources)?;
    Ok(ModLoadResult {
        manifest,
        materials,
        rules,
        warnings: Vec::new(),
    })
}

/// Merge a loaded mod into the base material table and rule set.
/// Remaps mod IDs to contiguous internal IDs via the provided IdRemap.
/// Returns a list of conflict warning strings (e.g. rule overrides).
pub fn merge_mod(
    base_materials: &mut MaterialTable,
    base_rules: &mut RuleSet,
    mod_result: &ModLoadResult,
    remap: &mut IdRemap,
) -> Vec<String> {
    let mut warnings = Vec::new();

    // Clone mod data so we can remap in place
    let mut mod_materials = mod_result.materials.clone();
    let mut mod_rules = mod_result.rules.clone();

    // Remap mod material IDs to contiguous internal IDs
    remap_material_table(&mut mod_materials, remap);

    // Remap mod rule IDs (base IDs pass through unchanged)
    remap_rule_set(&mut mod_rules, remap);

    // Append mod materials to base
    base_materials.materials.extend(mod_materials.materials);

    // Merge rules: check for conflicts (same input_a, input_b pair)
    for mod_rule in mod_rules.rules {
        let existing_idx = base_rules.rules.iter().position(|r| {
            (r.input_a == mod_rule.input_a && r.input_b == mod_rule.input_b)
                || (r.input_a == mod_rule.input_b && r.input_b == mod_rule.input_a)
        });

        if let Some(idx) = existing_idx {
            warnings.push(format!(
                "Mod '{}': rule '{}' overrides base rule '{}'",
                mod_result.manifest.name, mod_rule.name, base_rules.rules[idx].name
            ));
            base_rules.rules[idx] = mod_rule;
        } else {
            base_rules.rules.push(mod_rule);
        }
    }

    warnings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_single_material() {
        let ron = r#"[
            (
                id: 0,
                name: "Air",
                phase: Gas,
                density: 0.0,
                color: (0.0, 0.0, 0.0),
                emission: 0.0,
            ),
        ]"#;
        let table = load_materials_from_str(ron).expect("should parse");
        assert_eq!(table.len(), 1);
        assert_eq!(table.materials[0].name, "Air");
    }

    #[test]
    fn test_load_single_rule() {
        let ron = r#"[
            (
                name: "Fire+Wood",
                input_a: 5,
                input_b: 8,
                output_a: 5,
                output_b: 9,
                probability: 0.8,
                temp_delta: 200,
            ),
        ]"#;
        let set = load_rules_from_str(ron).expect("should parse");
        assert_eq!(set.len(), 1);
        assert_eq!(set.rules[0].name, "Fire+Wood");
    }

    #[test]
    fn test_malformed_ron_rejected() {
        let ron = r#"[this is not valid RON {"#;
        let result = load_materials_from_str(ron);
        assert!(result.is_err());
    }

    #[test]
    fn test_load_all_merges() {
        let src1 = r#"[
            (id: 0, name: "Air", phase: Gas, density: 0.0, color: (0.0, 0.0, 0.0), emission: 0.0),
        ]"#;
        let src2 = r#"[
            (id: 1, name: "Stone", phase: Solid, density: 5000.0, color: (0.5, 0.5, 0.55), emission: 0.0),
        ]"#;
        let table = load_all_materials(&[src1, src2]).expect("should merge");
        assert_eq!(table.len(), 2);
    }

    #[test]
    fn test_valid_materials_load() {
        // Load all materials from all category files
        let naturals = include_str!("../../../data/materials/naturals.ron");
        let organics = include_str!("../../../data/materials/organics.ron");
        let energy = include_str!("../../../data/materials/energy.ron");
        let explosives = include_str!("../../../data/materials/explosives.ron");
        let metals = include_str!("../../../data/materials/metals.ron");
        let synthetics = include_str!("../../../data/materials/synthetics.ron");
        let exotic = include_str!("../../../data/materials/exotic.ron");
        let table = load_all_materials(&[
            naturals, organics, energy, explosives, metals, synthetics, exotic,
        ])
        .expect("should load");
        assert!(
            table.len() >= 200,
            "expected at least 200 materials, got {}",
            table.len()
        );
    }

    #[test]
    fn test_valid_rules_load() {
        // Load all rules from all rule files
        let combustion = include_str!("../../../data/rules/combustion.ron");
        let structural = include_str!("../../../data/rules/structural.ron");
        let phase_change = include_str!("../../../data/rules/phase_change.ron");
        let dissolution = include_str!("../../../data/rules/dissolution.ron");
        let displacement = include_str!("../../../data/rules/displacement.ron");
        let biological = include_str!("../../../data/rules/biological.ron");
        let thermal = include_str!("../../../data/rules/thermal.ron");
        let synthesis = include_str!("../../../data/rules/synthesis.ron");
        let set = load_all_rules(&[
            combustion,
            structural,
            phase_change,
            dissolution,
            displacement,
            biological,
            thermal,
            synthesis,
        ])
        .expect("should load");
        assert!(
            set.len() >= 500,
            "expected at least 500 rules, got {}",
            set.len()
        );
    }

    #[test]
    fn test_new_materials_load() {
        // M6: verify all 4 explosive materials parse correctly
        let explosives = include_str!("../../../data/materials/explosives.ron");
        let table = load_materials_from_str(explosives).expect("should parse explosives.ron");
        assert_eq!(table.len(), 4);

        let gunpowder = table.get(12).expect("Gunpowder (id=12) missing");
        assert_eq!(gunpowder.name, "Gunpowder");
        assert_eq!(gunpowder.structural_integrity, 5.0);

        let sealed_metal = table.get(13).expect("Sealed-Metal (id=13) missing");
        assert_eq!(sealed_metal.name, "Sealed-Metal");
        assert_eq!(sealed_metal.structural_integrity, 60.0);

        let glass = table.get(14).expect("Glass (id=14) missing");
        assert_eq!(glass.name, "Glass");
        assert_eq!(glass.structural_integrity, 8.0);

        let shards = table.get(15).expect("Glass Shards (id=15) missing");
        assert_eq!(shards.name, "Glass Shards");
        assert_eq!(shards.structural_integrity, 0.0);
    }

    #[test]
    fn test_gunpowder_rule_loads() {
        // Verify gunpowder combustion rules parse with pressure_delta
        let structural = include_str!("../../../data/rules/structural.ron");
        let set = load_rules_from_str(structural).expect("should parse structural.ron");
        assert!(
            set.len() >= 2,
            "expected at least 2 structural rules, got {}",
            set.len()
        );

        let fire_rule = set
            .rules
            .iter()
            .find(|r| r.name == "Gunpowder+Fire explosion")
            .expect("Gunpowder+Fire explosion rule missing");
        assert_eq!(fire_rule.pressure_delta, 60);
        assert_eq!(fire_rule.temp_delta, 500);

        let lava_rule = set
            .rules
            .iter()
            .find(|r| r.name == "Gunpowder+Lava explosion")
            .expect("Gunpowder+Lava explosion rule missing");
        assert_eq!(lava_rule.pressure_delta, 55);
    }

    #[test]
    fn test_mod_manifest_parse() {
        let ron = r#"(
            name: "Test Mod",
            version: "1.0.0",
            author: "Tester",
            description: "A test mod",
            load_order_hint: 100,
        )"#;
        let manifest = load_mod_manifest(ron).expect("should parse manifest");
        assert_eq!(manifest.name, "Test Mod");
        assert_eq!(manifest.version, "1.0.0");
        assert_eq!(manifest.author, "Tester");
        assert_eq!(manifest.load_order_hint, 100);
    }

    #[test]
    fn test_mod_manifest_malformed_rejected() {
        let ron = r#"{ not valid ron }"#;
        let result = load_mod_manifest(ron);
        assert!(result.is_err());
        match result.unwrap_err() {
            LoadError::ManifestParseError(_) => {}
            other => panic!("expected ManifestParseError, got {:?}", other),
        }
    }

    #[test]
    fn test_mod_load_example() {
        let manifest_ron = include_str!("../../../data/mods/example-mod/mod.ron");
        let materials_ron = include_str!("../../../data/mods/example-mod/materials/crystals.ron");
        let rules_ron =
            include_str!("../../../data/mods/example-mod/rules/crystal_interactions.ron");

        let result = load_mod(manifest_ron, &[materials_ron], &[rules_ron])
            .expect("should load example mod");
        assert_eq!(result.manifest.name, "Crystal Pack");
        assert!(
            result.materials.len() >= 20,
            "expected at least 20 mod materials, got {}",
            result.materials.len()
        );
        assert!(
            result.rules.len() >= 50,
            "expected at least 50 mod rules, got {}",
            result.rules.len()
        );
    }

    #[test]
    fn test_merge_mod_into_base() {
        // Minimal base
        let mut base_materials = MaterialTable {
            materials: vec![
                alkahest_core::material::MaterialDef {
                    id: 0,
                    name: "Air".into(),
                    phase: alkahest_core::material::Phase::Gas,
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
                alkahest_core::material::MaterialDef {
                    id: 1,
                    name: "Stone".into(),
                    phase: alkahest_core::material::Phase::Solid,
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
                    structural_integrity: 63.0,
                    opacity: None,
                    absorption_rate: 0.0,
                    electrical_conductivity: 0.0,
                    electrical_resistance: 0.0,
                    activation_threshold: 0,
                    charge_emission: 0,
                },
            ],
        };
        let mut base_rules = RuleSet { rules: vec![] };

        let manifest_ron = r#"(
            name: "Test Mod",
            version: "1.0.0",
            author: "Tester",
            description: "Test",
            load_order_hint: 100,
        )"#;
        let materials_ron = r#"[
            (id: 10001, name: "TestCrystal", phase: Solid, density: 3000.0, color: (0.5, 0.5, 1.0)),
        ]"#;
        let rules_ron = r#"[
            (name: "TestCrystal+Stone", input_a: 10001, input_b: 1, output_a: 10001, output_b: 1, probability: 0.5),
        ]"#;

        let mod_result =
            load_mod(manifest_ron, &[materials_ron], &[rules_ron]).expect("should load test mod");

        let base_max_id = base_materials.max_id();
        let mut remap = IdRemap::new(base_max_id);
        let warnings = merge_mod(
            &mut base_materials,
            &mut base_rules,
            &mod_result,
            &mut remap,
        );

        assert!(warnings.is_empty(), "unexpected warnings: {:?}", warnings);
        // Base (2) + mod (1) = 3 materials
        assert_eq!(base_materials.len(), 3);
        // The mod material should be remapped to base_max_id + 1 = 2
        assert!(base_materials.get(2).is_some());
        assert_eq!(
            base_materials.get(2).expect("should exist").name,
            "TestCrystal"
        );
        // Rule should reference remapped ID
        assert_eq!(base_rules.len(), 1);
        assert_eq!(base_rules.rules[0].input_a, 2); // remapped from 10001
        assert_eq!(base_rules.rules[0].input_b, 1); // base ID unchanged
    }
}
