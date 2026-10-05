#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ConfidentialityLevel {
    Offen,
    Vertraulich,
    StrengGeheim,
    NurLokal,
}

impl ConfidentialityLevel {
    pub fn as_index(self) -> u8 {
        match self {
            Self::Offen => 0,
            Self::Vertraulich => 1,
            Self::StrengGeheim => 2,
            Self::NurLokal => 3,
        }
    }

    pub fn max(self, other: Self) -> Self {
        std::cmp::max(self, other)
    }

    pub fn allows_transport(self) -> bool {
        !matches!(self, Self::NurLokal)
    }

    pub fn browser_blocked(self) -> bool {
        matches!(self, Self::StrengGeheim | Self::NurLokal)
    }
}

pub fn taint_max(levels: &[ConfidentialityLevel]) -> ConfidentialityLevel {
    levels
        .iter()
        .copied()
        .fold(ConfidentialityLevel::Offen, |current, value| current.max(value))
}

pub fn is_browser_blocked(level: ConfidentialityLevel) -> bool {
    level.browser_blocked()
}

pub fn max_level(levels: &[ConfidentialityLevel]) -> Option<ConfidentialityLevel> {
    if levels.is_empty() {
        return None;
    }

    Some(taint_max(levels))
}

#[cfg(test)]
mod tests {
    use super::{max_level, taint_max, ConfidentialityLevel, is_browser_blocked};

    #[test]
    fn ordering_is_monotonic() {
        assert!(ConfidentialityLevel::Offen < ConfidentialityLevel::Vertraulich);
        assert!(ConfidentialityLevel::Vertraulich < ConfidentialityLevel::StrengGeheim);
        assert!(ConfidentialityLevel::StrengGeheim < ConfidentialityLevel::NurLokal);
    }

    #[test]
    fn taint_max_tracks_highest_level() {
        let levels = [
            ConfidentialityLevel::Offen,
            ConfidentialityLevel::Vertraulich,
            ConfidentialityLevel::Offen,
        ];

        assert_eq!(taint_max(&levels), ConfidentialityLevel::Vertraulich);
        assert_eq!(max_level(&levels), Some(ConfidentialityLevel::Vertraulich));
    }

    #[test]
    fn browser_is_blocked_for_sensitive_levels() {
        assert!(!is_browser_blocked(ConfidentialityLevel::Offen));
        assert!(is_browser_blocked(ConfidentialityLevel::StrengGeheim));
        assert!(is_browser_blocked(ConfidentialityLevel::NurLokal));
    }
}
