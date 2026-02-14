use alkahest_core::material::{MaterialDef, MaterialTable};
use alkahest_core::rule::{InteractionRule, RuleSet};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum LoadError {
    #[error("Failed to parse materials RON: {0}")]
    MaterialParseError(String),
    #[error("Failed to parse rules RON: {0}")]
    RuleParseError(String),
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
        // Load all 12 materials from embedded RON data
        let naturals = include_str!("../../../data/materials/naturals.ron");
        let organics = include_str!("../../../data/materials/organics.ron");
        let energy = include_str!("../../../data/materials/energy.ron");
        let table = load_all_materials(&[naturals, organics, energy]).expect("should load");
        assert_eq!(table.len(), 12);
    }

    #[test]
    fn test_valid_rules_load() {
        // Load all 15 rules from embedded RON data
        let combustion = include_str!("../../../data/rules/combustion.ron");
        let set = load_all_rules(&[combustion]).expect("should load");
        assert!(
            set.len() >= 15,
            "expected at least 15 rules, got {}",
            set.len()
        );
    }
}
