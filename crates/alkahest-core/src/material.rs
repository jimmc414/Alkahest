use serde::{Deserialize, Serialize};

/// Physical phase of a material, controlling movement behavior.
/// Stored as u32 in GPU buffers to match shader constants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum Phase {
    Gas = 0,
    Liquid = 1,
    Solid = 2,
    Powder = 3,
}

impl Phase {
    /// Convert to f32 for GPU material property buffer.
    pub fn as_f32(self) -> f32 {
        self as u8 as f32
    }
}

/// A single material definition loaded from RON data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaterialDef {
    /// Stable material ID (C-DATA-1). 0 = air.
    pub id: u16,
    /// Human-readable name for debug display.
    pub name: String,
    /// Physical phase controlling movement rules.
    pub phase: Phase,
    /// Density in abstract units. Higher = sinks below lower.
    pub density: f32,
    /// RGB color (0.0–1.0 per channel).
    pub color: (f32, f32, f32),
    /// Emission intensity for rendering (0.0 = none, 5.0 = bright glow).
    #[serde(default)]
    pub emission: f32,
    /// Flammability (0.0 = fireproof, 1.0 = highly flammable).
    #[serde(default)]
    pub flammability: f32,
    /// Ignition temperature in Kelvin. Only relevant if flammability > 0.
    #[serde(default)]
    pub ignition_temp: f32,
    /// Per-tick temperature decrement for self-decay (quantized units).
    #[serde(default)]
    pub decay_rate: u32,
    /// Temperature threshold below which material transforms to decay_product (quantized).
    #[serde(default)]
    pub decay_threshold: u32,
    /// Material ID to transform into when temperature drops below decay_threshold.
    #[serde(default)]
    pub decay_product: u16,
    /// Viscosity for lateral liquid movement (0.0 = free flow, 1.0 = no flow).
    #[serde(default)]
    pub viscosity: f32,
    /// Thermal conductivity (0.0 = insulator, 1.0 = perfect conductor).
    #[serde(default)]
    pub thermal_conductivity: f32,
    /// Temperature in Kelvin at which this material undergoes upward phase change.
    /// 0.0 = no phase change.
    #[serde(default)]
    pub phase_change_temp: f32,
    /// Material ID to transform into when temperature exceeds phase_change_temp.
    #[serde(default)]
    pub phase_change_product: u16,
    /// Structural integrity (0.0–63.0). Pressure exceeding this causes rupture.
    /// 0.0 = no structural role (powders, gases). Higher = stronger containment.
    #[serde(default)]
    pub structural_integrity: f32,
}

/// Collection of material definitions indexed by ID.
#[derive(Debug, Clone, Default)]
pub struct MaterialTable {
    pub materials: Vec<MaterialDef>,
}

impl MaterialTable {
    /// Look up a material by ID. Returns None if not found.
    pub fn get(&self, id: u16) -> Option<&MaterialDef> {
        self.materials.iter().find(|m| m.id == id)
    }

    /// Get the maximum material ID in the table.
    pub fn max_id(&self) -> u16 {
        self.materials.iter().map(|m| m.id).max().unwrap_or(0)
    }

    /// Number of materials.
    pub fn len(&self) -> usize {
        self.materials.len()
    }

    /// Whether the table is empty.
    pub fn is_empty(&self) -> bool {
        self.materials.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_phase_as_f32() {
        assert_eq!(Phase::Gas.as_f32(), 0.0);
        assert_eq!(Phase::Liquid.as_f32(), 1.0);
        assert_eq!(Phase::Solid.as_f32(), 2.0);
        assert_eq!(Phase::Powder.as_f32(), 3.0);
    }

    #[test]
    fn test_material_table_get() {
        let table = MaterialTable {
            materials: vec![MaterialDef {
                id: 2,
                name: "Sand".into(),
                phase: Phase::Powder,
                density: 2500.0,
                color: (0.76, 0.70, 0.50),
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
            }],
        };
        assert!(table.get(2).is_some());
        assert!(table.get(99).is_none());
    }
}
