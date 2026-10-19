//! Lokale Inferenz des Basis-Modells (ADR-0012).
//!
//! Phase 1 (MVP): Aufruf eines vorgefertigten `llama-cli`-Binaries als Subprozess,
//! weil keine C++-Toolchain für in-process Bindings vorhanden ist. Das Backend steckt
//! hinter dem Trait [`InferenceBackend`], damit Tests ohne Modell laufen (Echo-Backend)
//! und Phase 2 (llama-cpp-2 in-process) die Oberfläche nicht ändert.

use std::path::PathBuf;
use std::process::Command;

/// Konfiguration der lokalen Laufzeit: Pfade zum llama-cli-Binary und zum GGUF-Modell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalInferenceConfig {
    pub llama_cli_path: PathBuf,
    pub model_path: PathBuf,
    pub max_tokens: u32,
}

#[derive(Debug, PartialEq, Eq)]
pub enum InferError {
    /// Pfade sind nicht konfiguriert oder zeigen auf fehlende Dateien.
    NotConfigured,
    /// Das Backend (Subprozess) ist fehlgeschlagen.
    BackendFailed(String),
}

pub trait InferenceBackend {
    fn generate(&self, prompt: &str) -> Result<String, InferError>;
}

/// Test-Backend ohne Modell: antwortet deterministisch, damit App-Tests offline laufen.
pub struct EchoBackend;

impl InferenceBackend for EchoBackend {
    fn generate(&self, prompt: &str) -> Result<String, InferError> {
        Ok(format!("Lokale Antwort (Echo-Backend): {prompt}"))
    }
}

/// Phase-1-Backend: ruft `llama-cli` als Subprozess auf und liest stdout.
pub struct CliBackend {
    config: LocalInferenceConfig,
}

impl CliBackend {
    pub fn new(config: LocalInferenceConfig) -> Result<Self, InferError> {
        if !config.llama_cli_path.is_file() || !config.model_path.is_file() {
            return Err(InferError::NotConfigured);
        }

        Ok(Self { config })
    }

    fn build_command(&self, prompt: &str) -> Command {
        let mut command = Command::new(&self.config.llama_cli_path);
        command
            .arg("-m")
            .arg(&self.config.model_path)
            .arg("-p")
            .arg(prompt)
            .arg("-n")
            .arg(self.config.max_tokens.to_string())
            // single-turn: ohne dieses Flag geht llama-cli bei Instruct-Modellen in den
            // interaktiven Modus und wartet auf stdin – der Subprozess würde hängen.
            .arg("-st")
            .arg("--no-display-prompt");
        command
    }
}

impl InferenceBackend for CliBackend {
    fn generate(&self, prompt: &str) -> Result<String, InferError> {
        let output = self
            .build_command(prompt)
            .output()
            .map_err(|err| InferError::BackendFailed(err.to_string()))?;

        if !output.status.success() {
            return Err(InferError::BackendFailed(format!(
                "llama-cli exit {:?}",
                output.status.code()
            )));
        }

        String::from_utf8(output.stdout)
            .map(|text| extract_answer(&text))
            .map_err(|err| InferError::BackendFailed(err.to_string()))
    }
}

/// Extrahiert die Antwort aus der llama-cli-Ausgabe: sie liegt zwischen dem
/// Prompt-Echo (letzte Zeile mit ">"-Präfix) und den Statistikzeilen
/// ("[ Prompt:" / "Exiting"). Fällt ohne Echo auf den Gesamttext zurück.
fn extract_answer(output: &str) -> String {
    let after_echo = match output.rfind("\n>") {
        Some(index) => {
            let slice = &output[index + 2..];
            match slice.find('\n') {
                Some(newline) => &slice[newline + 1..],
                None => return String::new(),
            }
        }
        None => output,
    };

    let end = after_echo
        .find("[ Prompt:")
        .or_else(|| after_echo.find("Exiting"))
        .unwrap_or(after_echo.len());

    after_echo[..end].trim().to_string()
}

/// Sucht das GGUF-Modell: Umgebungsvariable TENSEGRITY_MODEL_PATH zuerst,
/// danach der App-Datenordner des Nutzers.
pub fn resolve_model_path() -> Option<PathBuf> {
    resolve_path(
        "TENSEGRITY_MODEL_PATH",
        &["Tensegrity", "models", "qwen2.5-0.5b-instruct-q4_k_m.gguf"],
    )
}

/// Sucht das llama-cli-Binary: Umgebungsvariable TENSEGRITY_LLAMA_CLI zuerst,
/// danach der App-Datenordner des Nutzers.
pub fn resolve_llama_cli_path() -> Option<PathBuf> {
    let binary = if cfg!(windows) { "llama-cli.exe" } else { "llama-cli" };
    resolve_path("TENSEGRITY_LLAMA_CLI", &["Tensegrity", "bin", binary])
}

fn resolve_path(env_var: &str, appdata_parts: &[&str]) -> Option<PathBuf> {
    if let Some(from_env) = std::env::var_os(env_var) {
        let candidate = PathBuf::from(from_env);
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
        let mut candidate = PathBuf::from(local_app_data);
        for part in appdata_parts {
            candidate.push(part);
        }
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::{CliBackend, EchoBackend, InferError, InferenceBackend, LocalInferenceConfig};
    use std::path::PathBuf;

    #[test]
    fn echo_backend_answers_deterministically() {
        let backend = EchoBackend;

        let answer = backend.generate("Hallo").unwrap();

        assert!(answer.contains("Hallo"));
    }

    #[test]
    fn cli_backend_requires_existing_files() {
        let config = LocalInferenceConfig {
            llama_cli_path: PathBuf::from("does-not-exist.exe"),
            model_path: PathBuf::from("missing.gguf"),
            max_tokens: 32,
        };

        let result = CliBackend::new(config);

        assert!(matches!(result, Err(InferError::NotConfigured)));
    }

    #[test]
    fn cli_backend_reports_spawn_failure_as_backend_error() {
        // Existierende Datei, die kein llama-cli ist: Spawn scheitert oder Exit-Code != 0.
        let temp = std::env::temp_dir().join("tg-infer-not-a-cli.txt");
        std::fs::write(&temp, "kein binary").unwrap();
        let model = std::env::temp_dir().join("tg-infer-model.gguf");
        std::fs::write(&model, "kein modell").unwrap();

        let config = LocalInferenceConfig {
            llama_cli_path: temp.clone(),
            model_path: model.clone(),
            max_tokens: 32,
        };
        let backend = CliBackend::new(config).unwrap();
        let result = backend.generate("Hallo");

        assert!(matches!(result, Err(InferError::BackendFailed(_))));

        let _ = std::fs::remove_file(temp);
        let _ = std::fs::remove_file(model);
    }

    #[test]
    fn resolve_path_prefers_environment_variable() {
        let temp = std::env::temp_dir().join("tg-infer-resolve.bin");
        std::fs::write(&temp, "x").unwrap();
        std::env::set_var("TG_INFER_TEST_RESOLVE", &temp);

        let found = super::resolve_path("TG_INFER_TEST_RESOLVE", &["nirgends"]);

        assert_eq!(found, Some(temp.clone()));
        std::env::remove_var("TG_INFER_TEST_RESOLVE");
        let _ = std::fs::remove_file(temp);
    }

    #[test]
    fn resolve_path_returns_none_when_nothing_exists() {
        let found = super::resolve_path("TG_INFER_TEST_MISSING", &["gibts", "nicht"]);

        assert_eq!(found, None);
    }

    #[test]
    fn extract_answer_strips_banner_echo_and_statistics() {
        let output = "Loading model...\n\navailable commands:\n  /exit or Ctrl+C\n\n\n> Nenne drei Vorteile lokaler KI-Modelle.\n1. Datenschutz\n2. Offline nutzbar\n3. Schnell\n\n[ Prompt: 949.9 t/s | Generation: 123.9 t/s ]\n\n\nExiting...\n";

        let answer = super::extract_answer(output);

        assert_eq!(answer, "1. Datenschutz\n2. Offline nutzbar\n3. Schnell");
    }

    #[test]
    fn extract_answer_without_echo_falls_back_to_full_text() {
        let answer = super::extract_answer("  Einfache Antwort ohne Banner  ");

        assert_eq!(answer, "Einfache Antwort ohne Banner");
    }
}
