use sha2::{Digest, Sha256};
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

#[derive(Debug, PartialEq, Eq)]
pub struct UpdateRequest {
    pub device_id: String,
    pub current_version: String,
    pub known_slot_ids: Vec<String>,
}

/// Baut eine Update-Anfrage eines Geräts an den Hive (release.proto: UpdateRequest).
pub fn build_update_request(
    device_id: &str,
    current_version: &str,
    known_slot_ids: &[String],
) -> Result<UpdateRequest, String> {
    if device_id.trim().is_empty() {
        return Err("device_id is required".to_string());
    }
    if current_version.trim().is_empty() {
        return Err("current_version is required".to_string());
    }

    Ok(UpdateRequest {
        device_id: device_id.to_string(),
        current_version: current_version.to_string(),
        known_slot_ids: known_slot_ids.to_vec(),
    })
}

#[derive(Debug, PartialEq, Eq)]
pub struct ReleaseBundleInfo {
    pub release_id: String,
    pub version: String,
    pub manifest: String,
    pub manifest_hash: String,
    pub signature: String,
}

/// Validiert ein Release-Bundle auf dem Gerät (REQ-SYS-009):
/// Version und Signatur müssen vorhanden sein, der Manifest-Hash muss stimmen,
/// die Signatur muss zum Hash passen, und Downgrades werden abgelehnt.
pub fn validate_release_bundle(bundle: &ReleaseBundleInfo, current_version: &str) -> Result<(), String> {
    if bundle.version.trim().is_empty() || bundle.signature.trim().is_empty() {
        return Err("missing version or signature".to_string());
    }

    let computed_hash = format!("{:x}", Sha256::digest(bundle.manifest.as_bytes()));
    if bundle.manifest_hash != computed_hash {
        return Err("manifest hash mismatch".to_string());
    }

    if bundle.signature != format!("sig:{computed_hash}") {
        return Err("invalid signature".to_string());
    }

    if is_downgrade(current_version, &bundle.version) {
        return Err("downgrade is not allowed".to_string());
    }

    Ok(())
}

fn is_downgrade(current: &str, candidate: &str) -> bool {
    fn parts(version: &str) -> Vec<u64> {
        version
            .split('.')
            .map(|part| part.trim().parse::<u64>().unwrap_or(0))
            .collect()
    }

    parts(candidate) < parts(current)
}

#[cfg(test)]
mod tests {
    use super::{
        build_submission, build_update_request, validate_release_bundle, ReleaseBundleInfo,
        SubmissionStatus,
    };
    use sha2::{Digest, Sha256};

    fn signed_bundle(version: &str, manifest: &str) -> ReleaseBundleInfo {
        let hash = format!("{:x}", Sha256::digest(manifest.as_bytes()));
        ReleaseBundleInfo {
            release_id: "rel-1".to_string(),
            version: version.to_string(),
            manifest: manifest.to_string(),
            manifest_hash: hash.clone(),
            signature: format!("sig:{hash}"),
        }
    }
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

    #[test]
    fn update_request_requires_device_and_version() {
        assert!(build_update_request("", "1.0.0", &[]).is_err());
        assert!(build_update_request("device-1", "", &[]).is_err());

        let request = build_update_request("device-1", "1.0.0", &["legal".to_string()]).unwrap();
        assert_eq!(request.device_id, "device-1");
        assert_eq!(request.known_slot_ids, vec!["legal".to_string()]);
    }

    #[test]
    fn release_bundle_with_valid_hash_and_signature_is_accepted() {
        let bundle = signed_bundle("1.1.0", "version=1.1.0");

        assert!(validate_release_bundle(&bundle, "1.0.0").is_ok());
    }

    #[test]
    fn release_bundle_with_hash_mismatch_is_rejected() {
        let mut bundle = signed_bundle("1.1.0", "version=1.1.0");
        bundle.manifest_hash = "deadbeef".to_string();

        let result = validate_release_bundle(&bundle, "1.0.0");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("hash"));
    }

    #[test]
    fn release_bundle_with_wrong_signature_is_rejected() {
        let mut bundle = signed_bundle("1.1.0", "version=1.1.0");
        bundle.signature = "sig:wrong".to_string();

        let result = validate_release_bundle(&bundle, "1.0.0");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("signature"));
    }

    #[test]
    fn release_bundle_downgrade_is_rejected() {
        let bundle = signed_bundle("1.0.0", "version=1.0.0");

        let result = validate_release_bundle(&bundle, "1.1.0");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("downgrade"));
    }

    #[test]
    fn release_bundle_with_missing_version_is_rejected() {
        let mut bundle = signed_bundle("1.1.0", "version=1.1.0");
        bundle.version = " ".to_string();

        assert!(validate_release_bundle(&bundle, "1.0.0").is_err());
    }
}
