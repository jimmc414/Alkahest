use alkahest_core::constants::NO_RULE;
use alkahest_core::material::MaterialTable;
use alkahest_core::math::temp_to_quantized;
use alkahest_core::rule::RuleSet;
use wgpu::util::DeviceExt;

/// Color + emission data extracted from material definitions for the renderer.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CompiledMaterialColor {
    pub color: [f32; 3],
    pub emission: f32,
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
}

/// GPU material property layout: 3x vec4<f32> = 48 bytes per material.
///
/// ```text
/// vec4<f32>[0]: density, phase, flammability, ignition_temp_quantized
/// vec4<f32>[1]: decay_rate, decay_threshold, decay_product_id, viscosity
/// vec4<f32>[2]: thermal_conductivity, phase_change_temp_quantized, phase_change_product_id, _pad
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
    _pad: f32,
}

/// GPU rule data layout: 2x vec4<u32> = 32 bytes per rule entry.
///
/// ```text
/// vec4<u32>[0]: input_a_becomes, _unused, _unused, probability_u32
/// vec4<u32>[1]: temp_delta_i32, _unused, min_temp, max_temp
/// ```
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuRuleEntry {
    input_a_becomes: u32,
    _unused0: u32,
    _unused1: u32,
    probability_u32: u32,
    temp_delta: i32,
    _unused2: u32,
    min_temp: u32,
    max_temp: u32,
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
            _pad: 0.0,
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
                _pad: 0.0,
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
            _unused0: 0,
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
            _unused0: 0,
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
            _unused0: 0,
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

    // Build material colors for the renderer
    let mut material_colors = vec![
        CompiledMaterialColor {
            color: [0.0, 0.0, 0.0],
            emission: 0.0,
        };
        material_count as usize
    ];

    for mat in &materials.materials {
        let idx = mat.id as usize;
        if idx < material_colors.len() {
            material_colors[idx] = CompiledMaterialColor {
                color: [mat.color.0, mat.color.1, mat.color.2],
                emission: mat.emission,
            };
        }
    }

    GpuRuleData {
        material_props_buffer,
        rule_lookup_buffer,
        rule_data_buffer,
        material_count,
        rule_count: rule_entries.len() as u32,
        material_colors,
    }
}
