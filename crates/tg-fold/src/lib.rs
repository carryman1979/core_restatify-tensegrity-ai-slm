use tg_policy::{ConfidentialityLevel, is_browser_blocked};

#[derive(Debug, PartialEq, Eq)]
pub struct FoldResult {
    pub level: ConfidentialityLevel,
    pub payload_bytes: usize,
    pub noise_applied: bool,
    pub message: String,
}

pub fn fold(delta_bytes: usize, level: ConfidentialityLevel) -> Result<FoldResult, String> {
    if matches!(level, ConfidentialityLevel::NurLokal) {
        return Err("NUR_LOKAL must stay local and produce zero bytes for network transport.".to_string());
    }

    let payload_bytes = match level {
        ConfidentialityLevel::Offen => delta_bytes,
        ConfidentialityLevel::Vertraulich => delta_bytes / 2,
        ConfidentialityLevel::StrengGeheim => delta_bytes / 4,
        ConfidentialityLevel::NurLokal => 0,
    };

    let noise_applied = matches!(level, ConfidentialityLevel::Vertraulich | ConfidentialityLevel::StrengGeheim);
    let message = match level {
        ConfidentialityLevel::Offen => "plain delta folded".to_string(),
        ConfidentialityLevel::Vertraulich => "differential privacy applied".to_string(),
        ConfidentialityLevel::StrengGeheim => "strict-secret metadata retained, limited payload".to_string(),
        ConfidentialityLevel::NurLokal => "local-only; no transport".to_string(),
    };

    Ok(FoldResult {
        level,
        payload_bytes,
        noise_applied,
        message,
    })
}

pub fn local_only_structure(level: ConfidentialityLevel) -> bool {
    matches!(level, ConfidentialityLevel::NurLokal) || is_browser_blocked(level)
}

#[cfg(test)]
mod tests {
    use super::fold;
    use tg_policy::ConfidentialityLevel;

    #[test]
    fn local_only_submission_is_rejected() {
        let result = fold(1024, ConfidentialityLevel::NurLokal);
        assert!(result.is_err());
    }

    #[test]
    fn strict_secret_keeps_payload_small() {
        let result = fold(1024, ConfidentialityLevel::StrengGeheim).unwrap();
        assert_eq!(result.payload_bytes, 256);
        assert!(result.noise_applied);
    }

    #[test]
    fn confidential_payload_is_smaller_and_noisy() {
        let result = fold(1024, ConfidentialityLevel::Vertraulich).unwrap();
        assert_eq!(result.payload_bytes, 512);
        assert!(result.noise_applied);
    }
}
