//! FFI-Brücke zwischen dem Rust-Core und der Uno-App (C-ABI).
//!
//! Die tatsächliche Bindings-Generierung (uniffi-bindgen-cs vs. csbindgen) und das
//! Streaming-Callback werden in Spike S3 entschieden; diese Oberfläche ist bewusst
//! minimal: nur skalare Werte und Pointer/Länge, keine komplexen Typen über die Grenze.
//!
//! Sicherheitshinweis: Der `unsafe`-Block in `tg_taint_max` ist erforderlich, weil die
//! Funktion einen rohen C-Pointer dereferenziert. Der Vertrag verlangt, dass `levels`
//! auf `len` lesbare Bytes zeigt; Null-Pointer werden als `-1` beantwortet.

use tg_fold::fold;
use tg_policy::{taint_max, ConfidentialityLevel};

pub fn level_from_index(value: u8) -> Option<ConfidentialityLevel> {
    match value {
        0 => Some(ConfidentialityLevel::Offen),
        1 => Some(ConfidentialityLevel::Vertraulich),
        2 => Some(ConfidentialityLevel::StrengGeheim),
        3 => Some(ConfidentialityLevel::NurLokal),
        _ => None,
    }
}

#[no_mangle]
pub extern "C" fn tg_policy_allows_transport(level: u8) -> bool {
    level_from_index(level)
        .map(|parsed| parsed.allows_transport())
        .unwrap_or(false)
}

#[no_mangle]
pub extern "C" fn tg_policy_browser_blocked(level: u8) -> bool {
    level_from_index(level)
        .map(|parsed| parsed.browser_blocked())
        .unwrap_or(true)
}

/// Liefert die strengste Stufe als Index. Fehlercodes: -1 bei Null-Pointer,
/// -2 bei unbekanntem Level-Wert.
///
/// # Safety
/// `levels` muss auf `len` lesbare `u8`-Werte zeigen und darf nicht null sein.
#[no_mangle]
pub unsafe extern "C" fn tg_taint_max(levels: *const u8, len: usize) -> i32 {
    if levels.is_null() {
        return -1;
    }

    let slice = std::slice::from_raw_parts(levels, len);
    let mut parsed = Vec::with_capacity(len);
    for &value in slice {
        match level_from_index(value) {
            Some(level) => parsed.push(level),
            None => return -2,
        }
    }

    taint_max(&parsed).as_index() as i32
}

/// Liefert die gefaltete Nutzlastgröße in Bytes. Fehlercodes: -1 bei NUR_LOKAL
/// (kein Transport), -2 bei unbekanntem Level-Wert.
#[no_mangle]
pub extern "C" fn tg_fold_payload_bytes(delta_bytes: usize, level: u8) -> i64 {
    match level_from_index(level) {
        Some(parsed) => match fold(delta_bytes, parsed) {
            Ok(result) => result.payload_bytes as i64,
            Err(_) => -1,
        },
        None => -2,
    }
}

#[cfg(test)]
mod tests {
    use super::{tg_fold_payload_bytes, tg_policy_allows_transport, tg_policy_browser_blocked, tg_taint_max};

    #[test]
    fn transport_is_allowed_only_below_nur_lokal() {
        assert!(tg_policy_allows_transport(0));
        assert!(tg_policy_allows_transport(1));
        assert!(tg_policy_allows_transport(2));
        assert!(!tg_policy_allows_transport(3));
        assert!(!tg_policy_allows_transport(99));
    }

    #[test]
    fn browser_block_is_conservative_for_unknown_levels() {
        assert!(!tg_policy_browser_blocked(0));
        assert!(tg_policy_browser_blocked(2));
        assert!(tg_policy_browser_blocked(99));
    }

    #[test]
    fn taint_max_over_c_abi_picks_strictest_level() {
        let levels = [0_u8, 1, 0];
        let result = unsafe { tg_taint_max(levels.as_ptr(), levels.len()) };
        assert_eq!(result, 1);

        let empty: [u8; 0] = [];
        assert_eq!(unsafe { tg_taint_max(empty.as_ptr(), 0) }, 0);
        assert_eq!(unsafe { tg_taint_max(std::ptr::null(), 3) }, -1);
        assert_eq!(unsafe { tg_taint_max([9_u8].as_ptr(), 1) }, -2);
    }

    #[test]
    fn fold_over_c_abi_reports_payload_or_error_codes() {
        assert_eq!(tg_fold_payload_bytes(1024, 0), 1024);
        assert_eq!(tg_fold_payload_bytes(1024, 1), 512);
        assert_eq!(tg_fold_payload_bytes(1024, 3), -1);
        assert_eq!(tg_fold_payload_bytes(1024, 42), -2);
    }
}
