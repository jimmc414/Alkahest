use std::hash::{Hash, Hasher};

use alkahest_core::constants::NO_RULE;
use alkahest_core::material::MaterialTable;
use alkahest_core::math::temp_to_quantized;
use alkahest_core::rule::RuleSet;
use wgpu::util::DeviceExt;

/// Color + rendering data extracted from material definitions for the renderer.
/// 32 bytes per entry, matching the GPU MaterialColor struct.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CompiledMaterialColor {
    pub color: [f32; 3],
    pub opacity: f32,
    pub emission: f32,
    pub absorption_rate: f32,
    pub phase: f32,
    pub _padding: f32,
}

/// Compiled GPU rule data ready for upload. Created once at init (C-PERF-2).
pub struct GpuRuleData {
    /// Material properties buffer: 48 bytes (3x vec4<f32>) per material.
    pub material_props_buffer: wgpu::Buffer,
    /// Flat 2D lookup: `rule_lookup[a * material_count + b]` = rule index or NO_RULE.
    pub rule_lookup_buffer: wgpu::Buffer,
    /// Rule data buffer: 32 bytes (2x vec4<u32>) per packed rule entry.
    pub rule_data_buffer: wgpu::Buffer,
    /// Number of materials (used for uniform upload).
    pub material_count: u32,
    /// Number of compiled rule entries (each bidirectional rule creates 2).
    pub rule_count: u32,
    /// Material colors extracted from material definitions for the renderer.
    pub material_colors: Vec<CompiledMaterialColor>,
    /// Deterministic hash of the rule set for save/load compatibility checking.
    pub rule_hash: u64,
}

/// GPU material property layout: 3x vec4<f32> = 48 bytes per material.
///
/// ```text
/// vec4<f32>[0]: density, phase, flammability, ignition_temp_quantized
/// vec4<f32>[1]: decay_rate, decay_threshold, decay_product_id, viscosity
/// vec4<f32>[2]: thermal_conductivity, phase_change_temp_quantized, phase_change_product_id, structural_integrity
/// ```
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuMaterialProps {
    density: f32,
    phase: f32,
    flammability: f32,
    ignition_temp_quantized: f32,
    decay_rate: f32,
    decay_threshold: f32,
    decay_product_id: f32,
    viscosity: f32,
    thermal_conductivity: f32,
    phase_change_temp_quantized: f32,
    phase_change_product_id: f32,
    structural_integrity: f32,
}

/// GPU rule data layout: 2x vec4<u32> = 32 bytes per rule entry.
///
/// ```text
/// vec4<u32>[0]: input_a_becomes, pressure_delta (bitcast i32), _unused, probability_u32
/// vec4<u32>[1]: temp_delta_i32, _unused, min_temp, max_temp
/// ```
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuRuleEntry {
    input_a_becomes: u32,
    pressure_delta: i32,
    _unused1: u32,
    probability_u32: u32,
    temp_delta: i32,
    _unused2: u32,
    min_temp: u32,
    max_temp: u32,
}

/// Compute a deterministic hash from material definitions and interaction rules.
///
/// Materials are sorted by ID, rules by (input_a, input_b). All numeric fields
/// are hashed to detect any change that could affect simulation behavior.
pub fn compute_rule_hash(materials: &MaterialTable, rules: &RuleSet) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();

    // Sort materials by ID for determinism
    let mut sorted_mats: Vec<_> = materials.materials.iter().collect();
    sorted_mats.sort_by_key(|m| m.id);

    for mat in &sorted_mats {
        mat.id.hash(&mut hasher);
        (mat.phase as u8).hash(&mut hasher);
        mat.density.to_bits().hash(&mut hasher);
        mat.flammability.to_bits().hash(&mut hasher);
        mat.ignition_temp.to_bits().hash(&mut hasher);
        mat.decay_rate.hash(&mut hasher);
        mat.decay_threshold.hash(&mut hasher);
        mat.decay_product.hash(&mut hasher);
        mat.viscosity.to_bits().hash(&mut hasher);
        mat.thermal_conductivity.to_bits().hash(&mut hasher);
        mat.phase_change_temp.to_bits().hash(&mut hasher);
        mat.phase_change_product.hash(&mut hasher);
        mat.structural_integrity.to_bits().hash(&mut hasher);
    }

    // Sort rules by (input_a, input_b) for determinism
    let mut sorted_rules: Vec<_> = rules.rules.iter().collect();
    sorted_rules.sort_by_key(|r| (r.input_a, r.input_b));

    for rule in &sorted_rules {
        rule.input_a.hash(&mut hasher);
        rule.input_b.hash(&mut hasher);
        rule.output_a.hash(&mut hasher);
        rule.output_b.hash(&mut hasher);
        rule.probability.to_bits().hash(&mut hasher);
        rule.temp_delta.hash(&mut hasher);
        rule.min_temp.hash(&mut hasher);
        rule.max_temp.hash(&mut hasher);
        rule.pressure_delta.hash(&mut hasher);
    }

    hasher.finish()
}

/// Compile material and rule data into GPU buffers.
pub fn compile(device: &wgpu::Device, materials: &MaterialTable, rules: &RuleSet) -> GpuRuleData {
    let material_count = (materials.max_id() as u32) + 1;

    // Build material properties buffer
    let mut props = vec![
        GpuMaterialProps {
            density: 0.0,
            phase: 0.0,
            flammability: 0.0,
            ignition_temp_quantized: 0.0,
            decay_rate: 0.0,
            decay_threshold: 0.0,
            decay_product_id: 0.0,
            viscosity: 0.0,
            thermal_conductivity: 0.0,
            phase_change_temp_quantized: 0.0,
            phase_change_product_id: 0.0,
            structural_integrity: 0.0,
        };
        material_count as usize
    ];

    for mat in &materials.materials {
        let idx = mat.id as usize;
        if idx < props.len() {
            props[idx] = GpuMaterialProps {
                density: mat.density,
                phase: mat.phase.as_f32(),
                flammability: mat.flammability,
                ignition_temp_quantized: temp_to_quantized(mat.ignition_temp) as f32,
                decay_rate: mat.decay_rate as f32,
                decay_threshold: mat.decay_threshold as f32,
                decay_product_id: mat.decay_product as f32,
                viscosity: mat.viscosity,
                thermal_conductivity: mat.thermal_conductivity,
                phase_change_temp_quantized: temp_to_quantized(mat.phase_change_temp) as f32,
                phase_change_product_id: mat.phase_change_product as f32,
                structural_integrity: mat.structural_integrity,
            };
        }
    }

    let material_props_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("material-props-compiled"),
        contents: bytemuck::cast_slice(&props),
        usage: wgpu::BufferUsages::STORAGE,
    });

    // Build rule lookup and rule data buffers
    // Each bidirectional rule creates 2 GPU entries
    let mut rule_entries: Vec<GpuRuleEntry> = Vec::new();
    let mut lookup = vec![NO_RULE; (material_count * material_count) as usize];

    for rule in &rules.rules {
        let a = rule.input_a as u32;
        let b = rule.input_b as u32;

        if a >= material_count || b >= material_count {
            continue;
        }

        // Convert probability to u32 (0.0-1.0 -> 0-u32::MAX)
        let probability_u32 = (rule.probability.clamp(0.0, 1.0) * u32::MAX as f32) as u32;

        // Entry from A's perspective: A sees neighbor B
        // A becomes output_a
        let entry_a_idx = rule_entries.len() as u32;
        rule_entries.push(GpuRuleEntry {
            input_a_becomes: rule.output_a as u32,
            pressure_delta: rule.pressure_delta,
            _unused1: 0,
            probability_u32,
            temp_delta: rule.temp_delta,
            _unused2: 0,
            min_temp: rule.min_temp,
            max_temp: rule.max_temp,
        });

        // Entry from B's perspective: B sees neighbor A
        // B becomes output_b
        let entry_b_idx = rule_entries.len() as u32;
        rule_entries.push(GpuRuleEntry {
            input_a_becomes: rule.output_b as u32,
            pressure_delta: rule.pressure_delta,
            _unused1: 0,
            probability_u32,
            temp_delta: rule.temp_delta,
            _unused2: 0,
            min_temp: rule.min_temp,
            max_temp: rule.max_temp,
        });

        // Store in lookup table (first rule wins if duplicate)
        let idx_ab = (a * material_count + b) as usize;
        let idx_ba = (b * material_count + a) as usize;
        if lookup[idx_ab] == NO_RULE {
            lookup[idx_ab] = entry_a_idx;
        }
        if lookup[idx_ba] == NO_RULE {
            lookup[idx_ba] = entry_b_idx;
        }
    }

    // Ensure at least one rule entry exists (GPU buffer can't be empty)
    if rule_entries.is_empty() {
        rule_entries.push(GpuRuleEntry {
            input_a_becomes: 0,
            pressure_delta: 0,
            _unused1: 0,
            probability_u32: 0,
            temp_delta: 0,
            _unused2: 0,
            min_temp: 0,
            max_temp: 0,
        });
    }

    let rule_lookup_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("rule-lookup"),
        contents: bytemuck::cast_slice(&lookup),
        usage: wgpu::BufferUsages::STORAGE,
    });

    let rule_data_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("rule-data"),
        contents: bytemuck::cast_slice(&rule_entries),
        usage: wgpu::BufferUsages::STORAGE,
    });

    // Build material colors for the renderer (32 bytes per entry)
    let mut material_colors = vec![
        CompiledMaterialColor {
            color: [0.0, 0.0, 0.0],
            opacity: 0.0,
            emission: 0.0,
            absorption_rate: 0.0,
            phase: 0.0,
            _padding: 0.0,
        };
        material_count as usize
    ];

    for mat in &materials.materials {
        let idx = mat.id as usize;
        if idx < material_colors.len() {
            let opacity = mat.opacity.unwrap_or(match mat.phase {
                alkahest_core::material::Phase::Gas => 0.3,
                alkahest_core::material::Phase::Liquid => 0.7,
                alkahest_core::material::Phase::Solid | alkahest_core::material::Phase::Powder => {
                    1.0
                }
            });
            material_colors[idx] = CompiledMaterialColor {
                color: [mat.color.0, mat.color.1, mat.color.2],
                opacity,
                emission: mat.emission,
                absorption_rate: mat.absorption_rate,
                phase: mat.phase.as_f32(),
                _padding: 0.0,
            };
        }
    }

    let rule_hash = compute_rule_hash(materials, rules);

    GpuRuleData {
        material_props_buffer,
        rule_lookup_buffer,
        rule_data_buffer,
        material_count,
        rule_count: rule_entries.len() as u32,
        material_colors,
        rule_hash,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alkahest_core::material::{MaterialDef, MaterialTable, Phase};
    use alkahest_core::rule::{InteractionRule, RuleSet};

    fn test_materials() -> MaterialTable {
        MaterialTable {
            materials: vec![
                MaterialDef {
                    id: 0,
                    name: "Air".into(),
                    phase: Phase::Gas,
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
                    phase: Phase::Solid,
                    density: 2500.0,
                    color: (0.5, 0.5, 0.5),
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
        }
    }

    fn test_rules() -> RuleSet {
        RuleSet {
            rules: vec![InteractionRule {
                name: "test".into(),
                input_a: 0,
                input_b: 1,
                output_a: 0,
                output_b: 1,
                probability: 0.5,
                temp_delta: 0,
                min_temp: 0,
                max_temp: 0,
                pressure_delta: 0,
                min_charge: 0,
                max_charge: 0,
            }],
        }
    }

    #[test]
    fn test_rule_hash_deterministic() {
        let materials = test_materials();
        let rules = test_rules();

        let hash1 = compute_rule_hash(&materials, &rules);
        let hash2 = compute_rule_hash(&materials, &rules);
        assert_eq!(hash1, hash2, "same input should produce same hash");
    }

    #[test]
    fn test_rule_hash_changes_on_modification() {
        let materials = test_materials();
        let rules1 = test_rules();
        let mut rules2 = test_rules();
        rules2.rules[0].probability = 0.9;

        let hash1 = compute_rule_hash(&materials, &rules1);
        let hash2 = compute_rule_hash(&materials, &rules2);
        assert_ne!(
            hash1, hash2,
            "different rules should produce different hash"
        );
    }
}
