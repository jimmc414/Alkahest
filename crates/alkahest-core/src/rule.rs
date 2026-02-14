use serde::{Deserialize, Serialize};

/// A single pairwise interaction rule loaded from RON data.
///
/// When voxel A (input_a) is adjacent to voxel B (input_b):
/// - A becomes output_a, B becomes output_b
/// - Subject to probability check and temperature range
///
/// The compiler stores bidirectional GPU entries: one from A's perspective,
/// one from B's perspective, so each thread only writes its own voxel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InteractionRule {
    /// Human-readable name for debug display.
    pub name: String,
    /// Material ID of the first input.
    pub input_a: u16,
    /// Material ID of the second input.
    pub input_b: u16,
    /// Material ID that input_a transforms into.
    pub output_a: u16,
    /// Material ID that input_b transforms into.
    pub output_b: u16,
    /// Probability of the reaction occurring per tick (0.0–1.0).
    pub probability: f32,
    /// Temperature change applied to the reacting voxel (quantized integer delta).
    #[serde(default)]
    pub temp_delta: i32,
    /// Minimum temperature (quantized) for the reaction to occur. 0 = no minimum.
    #[serde(default)]
    pub min_temp: u32,
    /// Maximum temperature (quantized) for the reaction to occur. 0 = no maximum.
    #[serde(default)]
    pub max_temp: u32,
}

/// Collection of interaction rules.
#[derive(Debug, Clone, Default)]
pub struct RuleSet {
    pub rules: Vec<InteractionRule>,
}

impl RuleSet {
    /// Number of rules.
    pub fn len(&self) -> usize {
        self.rules.len()
    }

    /// Whether the set is empty.
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rule_set_len() {
        let set = RuleSet {
            rules: vec![InteractionRule {
                name: "test".into(),
                input_a: 5,
                input_b: 8,
                output_a: 5,
                output_b: 9,
                probability: 0.8,
                temp_delta: 200,
                min_temp: 0,
                max_temp: 0,
            }],
        };
        assert_eq!(set.len(), 1);
        assert!(!set.is_empty());
    }
}
