use tg_fold::fold;
use tg_policy::ConfidentialityLevel;

#[derive(Debug, PartialEq, Eq)]
pub struct DeltaSubmission {
    pub submission_id: String,
    pub app_instance_id: String,
    pub level: ConfidentialityLevel,
    pub payload_bytes: usize,
    pub structure_message: String,
    pub status: SubmissionStatus,
}

#[derive(Debug, PartialEq, Eq)]
pub enum SubmissionStatus {
    Ready,
    SkippedLocalOnly,
}

/// Baut eine Einreichung für den Hive.
///
/// `NUR_LOKAL` erzeugt niemals eine Netzwerk-Nutzlast: Die Funktion liefert eine
/// Submission mit `SkippedLocalOnly` und `payload_bytes == 0`, damit Aufrufer den
/// Transport sauber überspringen können, statt ein Delta zu versenden.
pub fn build_submission(
    submission_id: &str,
    app_instance_id: &str,
    delta_bytes: usize,
    level: ConfidentialityLevel,
) -> Result<DeltaSubmission, String> {
    if app_instance_id.trim().is_empty() {
        return Err("app_instance_id is required".to_string());
    }

    if matches!(level, ConfidentialityLevel::NurLokal) {
        return Ok(DeltaSubmission {
            submission_id: submission_id.to_string(),
            app_instance_id: app_instance_id.to_string(),
            level,
            payload_bytes: 0,
            structure_message: "local-only; no transport".to_string(),
            status: SubmissionStatus::SkippedLocalOnly,
        });
    }

    let folded = fold(delta_bytes, level)?;

    Ok(DeltaSubmission {
        submission_id: submission_id.to_string(),
        app_instance_id: app_instance_id.to_string(),
        level,
        payload_bytes: folded.payload_bytes,
        structure_message: folded.message,
        status: SubmissionStatus::Ready,
    })
}

#[cfg(test)]
mod tests {
    use super::{build_submission, SubmissionStatus};
    use tg_policy::ConfidentialityLevel;

    #[test]
    fn nur_lokal_produces_zero_bytes_and_is_skipped() {
        let submission = build_submission("sub-1", "app-1", 1024, ConfidentialityLevel::NurLokal).unwrap();

        assert_eq!(submission.status, SubmissionStatus::SkippedLocalOnly);
        assert_eq!(submission.payload_bytes, 0);
    }

    #[test]
    fn offen_submission_is_ready_with_full_payload() {
        let submission = build_submission("sub-2", "app-1", 1024, ConfidentialityLevel::Offen).unwrap();

        assert_eq!(submission.status, SubmissionStatus::Ready);
        assert_eq!(submission.payload_bytes, 1024);
    }

    #[test]
    fn vertraulich_submission_carries_dp_structure_message() {
        let submission = build_submission("sub-3", "app-1", 1024, ConfidentialityLevel::Vertraulich).unwrap();

        assert_eq!(submission.status, SubmissionStatus::Ready);
        assert_eq!(submission.payload_bytes, 512);
        assert!(submission.structure_message.contains("differential privacy"));
    }

    #[test]
    fn streng_geheim_submission_keeps_only_limited_structure() {
        let submission = build_submission("sub-4", "app-1", 1024, ConfidentialityLevel::StrengGeheim).unwrap();

        assert_eq!(submission.status, SubmissionStatus::Ready);
        assert_eq!(submission.payload_bytes, 256);
        assert!(submission.structure_message.contains("strict-secret"));
    }

    #[test]
    fn empty_app_instance_id_is_rejected() {
        let result = build_submission("sub-5", "  ", 1024, ConfidentialityLevel::Offen);

        assert!(result.is_err());
    }
}
