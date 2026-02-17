//! Material category ID range constants.
//! Existing IDs 0–15 are unchanged for save compatibility.

pub const NATURALS_START: u16 = 16;
pub const NATURALS_END: u16 = 49;

pub const METALS_START: u16 = 50;
pub const METALS_END: u16 = 89;

pub const ORGANICS_START: u16 = 90;
pub const ORGANICS_END: u16 = 129;

pub const ENERGY_START: u16 = 130;
pub const ENERGY_END: u16 = 169;

pub const SYNTHETICS_START: u16 = 170;
pub const SYNTHETICS_END: u16 = 209;

pub const EXOTIC_START: u16 = 210;
pub const EXOTIC_END: u16 = 249;

// ── M14: Expansion Ranges ─────────────────────────────────────────

pub const NATURALS_EXT_START: u16 = 250;
pub const NATURALS_EXT_END: u16 = 299;

pub const METALS_EXT_START: u16 = 300;
pub const METALS_EXT_END: u16 = 349;

pub const ORGANICS_EXT_START: u16 = 350;
pub const ORGANICS_EXT_END: u16 = 399;

pub const ENERGY_EXT_START: u16 = 400;
pub const ENERGY_EXT_END: u16 = 449;

pub const SYNTHETICS_EXT_START: u16 = 450;
pub const SYNTHETICS_EXT_END: u16 = 499;

pub const EXOTIC_EXT_START: u16 = 500;
pub const EXOTIC_EXT_END: u16 = 549;

// ── M15: Electrical Range ─────────────────────────────────────────

pub const ELECTRICAL_START: u16 = 550;
pub const ELECTRICAL_END: u16 = 569;

/// First valid material ID for mod-defined materials.
pub const MOD_ID_START: u16 = 10000;

/// Return the category name for a given material ID.
pub fn get_category(id: u16) -> &'static str {
    match id {
        0 => "Air",
        1..=15 => "Legacy",
        NATURALS_START..=NATURALS_END | NATURALS_EXT_START..=NATURALS_EXT_END => "Naturals",
        METALS_START..=METALS_END | METALS_EXT_START..=METALS_EXT_END => "Metals",
        ORGANICS_START..=ORGANICS_END | ORGANICS_EXT_START..=ORGANICS_EXT_END => "Organics",
        ENERGY_START..=ENERGY_END | ENERGY_EXT_START..=ENERGY_EXT_END => "Energy",
        SYNTHETICS_START..=SYNTHETICS_END | SYNTHETICS_EXT_START..=SYNTHETICS_EXT_END => {
            "Synthetics"
        }
        EXOTIC_START..=EXOTIC_END | EXOTIC_EXT_START..=EXOTIC_EXT_END => "Exotic",
        ELECTRICAL_START..=ELECTRICAL_END => "Electrical",
        570..=9999 => "Reserved",
        MOD_ID_START.. => "Mod",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_category_ranges() {
        assert_eq!(get_category(0), "Air");
        assert_eq!(get_category(1), "Legacy");
        assert_eq!(get_category(15), "Legacy");
        assert_eq!(get_category(16), "Naturals");
        assert_eq!(get_category(49), "Naturals");
        assert_eq!(get_category(50), "Metals");
        assert_eq!(get_category(89), "Metals");
        assert_eq!(get_category(90), "Organics");
        assert_eq!(get_category(129), "Organics");
        assert_eq!(get_category(130), "Energy");
        assert_eq!(get_category(169), "Energy");
        assert_eq!(get_category(170), "Synthetics");
        assert_eq!(get_category(209), "Synthetics");
        assert_eq!(get_category(210), "Exotic");
        assert_eq!(get_category(249), "Exotic");
        // Extension ranges
        assert_eq!(get_category(250), "Naturals");
        assert_eq!(get_category(299), "Naturals");
        assert_eq!(get_category(300), "Metals");
        assert_eq!(get_category(349), "Metals");
        assert_eq!(get_category(350), "Organics");
        assert_eq!(get_category(399), "Organics");
        assert_eq!(get_category(400), "Energy");
        assert_eq!(get_category(449), "Energy");
        assert_eq!(get_category(450), "Synthetics");
        assert_eq!(get_category(499), "Synthetics");
        assert_eq!(get_category(500), "Exotic");
        assert_eq!(get_category(549), "Exotic");
        // Electrical
        assert_eq!(get_category(550), "Electrical");
        assert_eq!(get_category(569), "Electrical");
        // Reserved and Mod
        assert_eq!(get_category(570), "Reserved");
        assert_eq!(get_category(9999), "Reserved");
        assert_eq!(get_category(10000), "Mod");
        assert_eq!(get_category(10001), "Mod");
        assert_eq!(get_category(u16::MAX), "Mod");
    }
}
