use crate::error::PersistError;
use crate::format::{SaveHeader, FORMAT_VERSION, MAGIC};

/// Validate a save file header and return any compatibility warnings.
///
/// Returns Ok(warnings) on success, Err on fatal errors.
/// A rule hash mismatch produces a warning, not an error.
pub fn validate_header(
    header: &SaveHeader,
    current_rule_hash: u64,
) -> Result<Vec<String>, PersistError> {
    // Check magic
    if header.magic != MAGIC {
        return Err(PersistError::InvalidMagic);
    }

    // Check version
    if header.version != FORMAT_VERSION {
        return Err(PersistError::UnsupportedVersion(header.version));
    }

    let mut warnings = Vec::new();

    // Rule hash mismatch = warning (world still loads, but behavior may differ)
    if header.rule_hash != current_rule_hash {
        warnings.push(format!(
            "Rule set has changed since this save was created \
             (save: {:016x}, current: {:016x}). \
             Material interactions may behave differently.",
            header.rule_hash, current_rule_hash
        ));
    }

    Ok(warnings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::CameraState;

    fn test_header(rule_hash: u64) -> SaveHeader {
        SaveHeader {
            magic: MAGIC,
            version: FORMAT_VERSION,
            _pad0: 0,
            rule_hash,
            tick_count: 0,
            chunk_count: 0,
            world_seed: 0,
            camera: CameraState {
                mode: 0,
                yaw: 0.0,
                pitch: 0.0,
                target: [0.0; 3],
                distance: 0.0,
            },
            _pad1: 0,
        }
    }

    #[test]
    fn test_valid_header_no_warnings() {
        let header = test_header(42);
        let warnings = validate_header(&header, 42).expect("should succeed");
        assert!(warnings.is_empty());
    }

    #[test]
    fn test_rule_hash_mismatch_warns() {
        let header = test_header(42);
        let warnings = validate_header(&header, 99).expect("should succeed with warnings");
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("Rule set has changed"));
    }

    #[test]
    fn test_invalid_magic_rejected() {
        let mut header = test_header(42);
        header.magic = *b"NOPE";
        let result = validate_header(&header, 42);
        assert!(matches!(result, Err(PersistError::InvalidMagic)));
    }

    #[test]
    fn test_unsupported_version_rejected() {
        let mut header = test_header(42);
        header.version = 99;
        let result = validate_header(&header, 42);
        assert!(matches!(result, Err(PersistError::UnsupportedVersion(99))));
    }
}
