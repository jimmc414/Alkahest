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

/// Return the category name for a given material ID.
pub fn get_category(id: u16) -> &'static str {
    match id {
        0 => "Air",
        1..=15 => "Legacy",
        NATURALS_START..=NATURALS_END => "Naturals",
        METALS_START..=METALS_END => "Metals",
        ORGANICS_START..=ORGANICS_END => "Organics",
        ENERGY_START..=ENERGY_END => "Energy",
        SYNTHETICS_START..=SYNTHETICS_END => "Synthetics",
        EXOTIC_START..=EXOTIC_END => "Exotic",
        _ => "Unknown",
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
        assert_eq!(get_category(255), "Unknown");
    }
}
