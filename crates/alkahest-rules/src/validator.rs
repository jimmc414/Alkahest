use alkahest_core::constants::TEMP_QUANT_MAX_VALUE;
use alkahest_core::material::MaterialTable;
use alkahest_core::rule::RuleSet;
use std::collections::HashSet;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("Duplicate material ID {0}")]
    DuplicateMaterialId(u16),
    #[error("Material '{name}' ignition_temp {value}K exceeds max {max}K (C-DATA-4)")]
    IgnitionTempExceedsMax { name: String, value: f32, max: f32 },
    #[error("Material '{name}' decay_threshold {value} exceeds quantization max {max} (C-DATA-4)")]
    DecayThresholdExceedsMax { name: String, value: u32, max: u16 },
    #[error("Rule '{name}' references unknown material ID {id}")]
    UnknownMaterialRef { name: String, id: u16 },
    #[error(
        "Rule '{name}' has temp_delta > 0 without transforming any material (C-DATA-3: energy from nothing)"
    )]
    EnergyFromNothing { name: String },
    #[error("Potential infinite loop: rules '{a}' and '{b}' form A->B->A cycle with overlapping temp ranges")]
    InfiniteLoop { a: String, b: String },
}

/// Validate a material table for constraint compliance.
pub fn validate_materials(table: &MaterialTable) -> Result<(), Vec<ValidationError>> {
    let mut errors = Vec::new();

    // Check ID uniqueness (C-DATA-1)
    let mut seen_ids = HashSet::new();
    for mat in &table.materials {
        if !seen_ids.insert(mat.id) {
            errors.push(ValidationError::DuplicateMaterialId(mat.id));
        }
    }

    // Check property ranges (C-DATA-4)
    for mat in &table.materials {
        if mat.ignition_temp > 8000.0 {
            errors.push(ValidationError::IgnitionTempExceedsMax {
                name: mat.name.clone(),
                value: mat.ignition_temp,
                max: 8000.0,
            });
        }
        if mat.decay_threshold > TEMP_QUANT_MAX_VALUE as u32 {
            errors.push(ValidationError::DecayThresholdExceedsMax {
                name: mat.name.clone(),
                value: mat.decay_threshold,
                max: TEMP_QUANT_MAX_VALUE,
            });
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Validate a rule set against the material table.
pub fn validate_rules(
    rules: &RuleSet,
    materials: &MaterialTable,
) -> Result<(), Vec<ValidationError>> {
    let mut errors = Vec::new();

    let valid_ids: HashSet<u16> = materials.materials.iter().map(|m| m.id).collect();

    for rule in &rules.rules {
        // Check all referenced material IDs exist
        for &id in &[rule.input_a, rule.input_b, rule.output_a, rule.output_b] {
            if !valid_ids.contains(&id) {
                errors.push(ValidationError::UnknownMaterialRef {
                    name: rule.name.clone(),
                    id,
                });
            }
        }

        // Energy conservation: temp_delta > 0 must transform at least one input (C-DATA-3)
        if rule.temp_delta > 0 && rule.output_a == rule.input_a && rule.output_b == rule.input_b {
            errors.push(ValidationError::EnergyFromNothing {
                name: rule.name.clone(),
            });
        }
    }

    // Infinite loop detection: if rule A->B and B->A with overlapping temp ranges
    for (i, ra) in rules.rules.iter().enumerate() {
        for rb in rules.rules.iter().skip(i + 1) {
            // Check if ra transforms A->B and rb transforms B->A
            let ra_transforms_a_to_b = ra.output_a != ra.input_a || ra.output_b != ra.input_b;
            let rb_transforms_b_to_a = rb.output_a != rb.input_a || rb.output_b != rb.input_b;

            if !ra_transforms_a_to_b || !rb_transforms_b_to_a {
                continue;
            }

            // Check if outputs of one are inputs of the other (cycle)
            let ra_produces = (ra.output_a, ra.output_b);
            let rb_inputs = (rb.input_a, rb.input_b);
            let rb_produces = (rb.output_a, rb.output_b);
            let ra_inputs = (ra.input_a, ra.input_b);

            let forward_cycle = (ra_produces.0 == rb_inputs.0 && ra_produces.1 == rb_inputs.1)
                || (ra_produces.0 == rb_inputs.1 && ra_produces.1 == rb_inputs.0);
            let backward_cycle = (rb_produces.0 == ra_inputs.0 && rb_produces.1 == ra_inputs.1)
                || (rb_produces.0 == ra_inputs.1 && rb_produces.1 == ra_inputs.0);

            if forward_cycle && backward_cycle {
                // Check overlapping temperature ranges
                let ra_min = ra.min_temp;
                let ra_max = if ra.max_temp == 0 {
                    u32::MAX
                } else {
                    ra.max_temp
                };
                let rb_min = rb.min_temp;
                let rb_max = if rb.max_temp == 0 {
                    u32::MAX
                } else {
                    rb.max_temp
                };

                if ra_min <= rb_max && rb_min <= ra_max {
                    errors.push(ValidationError::InfiniteLoop {
                        a: ra.name.clone(),
                        b: rb.name.clone(),
                    });
                }
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alkahest_core::material::{MaterialDef, Phase};
    use alkahest_core::rule::InteractionRule;

    fn make_material(id: u16, name: &str) -> MaterialDef {
        MaterialDef {
            id,
            name: name.into(),
            phase: Phase::Solid,
            density: 1000.0,
            color: (0.5, 0.5, 0.5),
            emission: 0.0,
            flammability: 0.0,
            ignition_temp: 0.0,
            decay_rate: 0,
            decay_threshold: 0,
            decay_product: 0,
            viscosity: 0.0,
        }
    }

    fn ten_materials() -> MaterialTable {
        MaterialTable {
            materials: vec![
                make_material(0, "Air"),
                make_material(1, "Stone"),
                make_material(2, "Sand"),
                make_material(3, "Water"),
                make_material(4, "Oil"),
                make_material(5, "Fire"),
                make_material(6, "Smoke"),
                make_material(7, "Steam"),
                make_material(8, "Wood"),
                make_material(9, "Ash"),
            ],
        }
    }

    #[test]
    fn test_valid_materials_load() {
        let table = ten_materials();
        assert!(validate_materials(&table).is_ok());
    }

    #[test]
    fn test_duplicate_material_id_rejected() {
        let mut table = ten_materials();
        table.materials.push(make_material(2, "DuplicateSand"));
        let result = validate_materials(&table);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors
            .iter()
            .any(|e| matches!(e, ValidationError::DuplicateMaterialId(2))));
    }

    #[test]
    fn test_property_exceeds_quantization_rejected() {
        let table = MaterialTable {
            materials: vec![{
                let mut m = make_material(0, "TooHot");
                m.ignition_temp = 9000.0; // exceeds 8000K
                m
            }],
        };
        let result = validate_materials(&table);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors
            .iter()
            .any(|e| matches!(e, ValidationError::IgnitionTempExceedsMax { .. })));
    }

    #[test]
    fn test_nonexistent_material_ref_rejected() {
        let table = ten_materials();
        let rules = RuleSet {
            rules: vec![InteractionRule {
                name: "BadRef".into(),
                input_a: 5,
                input_b: 99, // doesn't exist
                output_a: 5,
                output_b: 9,
                probability: 1.0,
                temp_delta: 0,
                min_temp: 0,
                max_temp: 0,
            }],
        };
        let result = validate_rules(&rules, &table);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors
            .iter()
            .any(|e| matches!(e, ValidationError::UnknownMaterialRef { id: 99, .. })));
    }

    #[test]
    fn test_energy_from_nothing_rejected() {
        let table = ten_materials();
        let rules = RuleSet {
            rules: vec![InteractionRule {
                name: "FreeEnergy".into(),
                input_a: 1,
                input_b: 2,
                output_a: 1, // unchanged
                output_b: 2, // unchanged
                probability: 1.0,
                temp_delta: 100, // positive delta with no transform = energy from nothing
                min_temp: 0,
                max_temp: 0,
            }],
        };
        let result = validate_rules(&rules, &table);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors
            .iter()
            .any(|e| matches!(e, ValidationError::EnergyFromNothing { .. })));
    }

    #[test]
    fn test_infinite_loop_detected() {
        let table = MaterialTable {
            materials: vec![
                make_material(0, "A"),
                make_material(1, "B"),
                make_material(2, "C"),
                make_material(3, "D"),
            ],
        };
        let rules = RuleSet {
            rules: vec![
                InteractionRule {
                    name: "AtoB".into(),
                    input_a: 0,
                    input_b: 1,
                    output_a: 2,
                    output_b: 3,
                    probability: 1.0,
                    temp_delta: 0,
                    min_temp: 0,
                    max_temp: 0,
                },
                InteractionRule {
                    name: "BtoA".into(),
                    input_a: 2,
                    input_b: 3,
                    output_a: 0,
                    output_b: 1,
                    probability: 1.0,
                    temp_delta: 0,
                    min_temp: 0,
                    max_temp: 0,
                },
            ],
        };
        let result = validate_rules(&rules, &table);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors
            .iter()
            .any(|e| matches!(e, ValidationError::InfiniteLoop { .. })));
    }
}
