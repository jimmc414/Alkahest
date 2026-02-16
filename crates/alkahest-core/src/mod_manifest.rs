use serde::{Deserialize, Serialize};

/// Metadata for a mod pack, parsed from mod.ron.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModManifest {
    /// Human-readable mod name.
    pub name: String,
    /// Semantic version string (e.g. "1.0.0").
    pub version: String,
    /// Mod author name.
    pub author: String,
    /// Brief description of what the mod adds.
    pub description: String,
    /// Lower values load first. Base game is implicitly 0.
    pub load_order_hint: u32,
}
