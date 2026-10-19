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
use tg_infer::{CliBackend, InferError, InferenceBackend, LocalInferenceConfig};
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

// ---------------------------------------------------------------------------
// Lokale Inferenz (ADR-0012): tg_infer_configure + tg_generate_local
// ---------------------------------------------------------------------------

static LOCAL_CONFIG: std::sync::RwLock<Option<LocalInferenceConfig>> =
    std::sync::RwLock::new(None);

/// Liest `ptr/len` als UTF-8-String. Fehlercode -1 bei Null-Pointer oder
/// ungültigem UTF-8.
///
/// # Safety
/// `ptr` muss auf `len` lesbare Bytes zeigen und darf nicht null sein.
unsafe fn read_str(ptr: *const u8, len: usize) -> Result<String, i32> {
    if ptr.is_null() {
        return Err(-1);
    }

    let slice = std::slice::from_raw_parts(ptr, len);
    std::str::from_utf8(slice)
        .map(|text| text.to_string())
        .map_err(|_| -1)
}

/// Konfiguriert die lokale Laufzeit (Pfade zu llama-cli und GGUF-Modell).
/// Rückgabe: 0 ok, -1 bei ungültigen String-Argumenten.
///
/// # Safety
/// Beide Pointer müssen auf `len` lesbare Bytes zeigen (siehe `read_str`).
#[no_mangle]
pub unsafe extern "C" fn tg_infer_configure(
    cli_path: *const u8,
    cli_len: usize,
    model_path: *const u8,
    model_len: usize,
) -> i32 {
    let cli = match read_str(cli_path, cli_len) {
        Ok(value) => value,
        Err(code) => return code,
    };
    let model = match read_str(model_path, model_len) {
        Ok(value) => value,
        Err(code) => return code,
    };

    let config = LocalInferenceConfig {
        llama_cli_path: cli.into(),
        model_path: model.into(),
        max_tokens: 256,
    };

    match LOCAL_CONFIG.write() {
        Ok(mut slot) => {
            *slot = Some(config);
            0
        }
        Err(_) => -3,
    }
}

/// Konfiguriert die lokale Laufzeit automatisch aus den Standardpfaden
/// (Umgebungsvariablen TENSEGRITY_LLAMA_CLI / TENSEGRITY_MODEL_PATH oder
/// App-Datenordner). Rückgabe: 0 ok, -1 wenn Binary oder Modell fehlen.
#[no_mangle]
pub extern "C" fn tg_infer_configure_auto() -> i32 {
    match (tg_infer::resolve_llama_cli_path(), tg_infer::resolve_model_path()) {
        (Some(cli), Some(model)) => {
            let config = LocalInferenceConfig {
                llama_cli_path: cli,
                model_path: model,
                max_tokens: 256,
            };
            match LOCAL_CONFIG.write() {
                Ok(mut slot) => {
                    *slot = Some(config);
                    0
                }
                Err(_) => -3,
            }
        }
        _ => -1,
    }
}

/// Generiert eine Antwort lokal in den Aufrufer-Puffer.
/// Rückgabe: >=0 geschriebene Bytes, -1 nicht konfiguriert/Modell fehlt,
/// -2 ungültiger Prompt, -3 Puffer zu klein, -4 Backend-Fehler.
///
/// # Safety
/// `prompt` muss auf `prompt_len` lesbare Bytes zeigen; `out` auf `out_cap`
/// beschreibbare Bytes.
#[no_mangle]
pub unsafe extern "C" fn tg_generate_local(
    prompt: *const u8,
    prompt_len: usize,
    out: *mut u8,
    out_cap: usize,
) -> i64 {
    let prompt = match read_str(prompt, prompt_len) {
        Ok(value) => value,
        Err(_) => return -2,
    };

    let config = match LOCAL_CONFIG.read() {
        Ok(guard) => match guard.clone() {
            Some(config) => config,
            None => return -1,
        },
        Err(_) => return -1,
    };

    let backend = match CliBackend::new(config) {
        Ok(backend) => backend,
        Err(InferError::NotConfigured) => return -1,
        Err(InferError::BackendFailed(_)) => return -4,
    };

    generate_into(&backend, &prompt, out, out_cap)
}

/// Schreibt die Backend-Antwort in den Aufrufer-Puffer (gemeinsame Logik,
/// damit Tests das Echo-Backend ohne globale Konfiguration nutzen können).
///
/// # Safety
/// `out` muss auf `out_cap` beschreibbare Bytes zeigen.
unsafe fn generate_into<B: InferenceBackend>(
    backend: &B,
    prompt: &str,
    out: *mut u8,
    out_cap: usize,
) -> i64 {
    if out.is_null() {
        return -3;
    }

    let answer = match backend.generate(prompt) {
        Ok(answer) => answer,
        Err(InferError::NotConfigured) => return -1,
        Err(InferError::BackendFailed(_)) => return -4,
    };

    let bytes = answer.as_bytes();
    if bytes.len() > out_cap {
        return -3;
    }

    std::ptr::copy_nonoverlapping(bytes.as_ptr(), out, bytes.len());
    bytes.len() as i64
}

#[cfg(test)]
mod tests {
    use super::{
        generate_into, tg_fold_payload_bytes, tg_generate_local, tg_infer_configure,
        tg_policy_allows_transport, tg_policy_browser_blocked, tg_taint_max,
    };
    use tg_infer::EchoBackend;

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

    #[test]
    fn generate_into_writes_echo_answer_and_checks_buffer() {
        let backend = EchoBackend;
        let mut buffer = [0_u8; 512];

        let written = unsafe { generate_into(&backend, "Hallo", buffer.as_mut_ptr(), buffer.len()) };

        assert!(written > 0);
        let text = std::str::from_utf8(&buffer[..written as usize]).unwrap();
        assert!(text.contains("Hallo"));

        let mut tiny = [0_u8; 4];
        assert_eq!(
            unsafe { generate_into(&backend, "Hallo", tiny.as_mut_ptr(), tiny.len()) },
            -3
        );
        assert_eq!(
            unsafe { generate_into(&backend, "Hallo", std::ptr::null_mut(), 16) },
            -3
        );
    }

    #[test]
    fn configure_and_generate_fail_cleanly_without_files() {
        let cli = "definitely-missing-llama-cli.exe";
        let model = "definitely-missing-model.gguf";

        let configured =
            unsafe { tg_infer_configure(cli.as_ptr(), cli.len(), model.as_ptr(), model.len()) };
        assert_eq!(configured, 0);

        let mut buffer = [0_u8; 128];
        let result =
            unsafe { tg_generate_local("Hallo".as_ptr(), 5, buffer.as_mut_ptr(), buffer.len()) };
        assert_eq!(result, -1);

        assert_eq!(
            unsafe { tg_infer_configure(std::ptr::null(), 3, model.as_ptr(), model.len()) },
            -1
        );
    }
}
