# CLAUDE.md – core_restatify-tensegrity-ai-slm

Rust-Workspace des Edge-Kerns. Fachliche Referenz: Docs-Repo (Whitepaper V1.0.26 Abschnitt 2–3, ADR-0003/4/5/7).

## Regeln
- `tg-policy` ist die einzige Quelle für Stufen/Maske/Taint (Skill `vertraulichkeits-maske` im Docs-Repo).
- Jede Funktion, die Daten nach außen gibt, prüft die Maske; „Nur lokal“ = 0 Byte.
- Keine Panics über die FFI-Grenze; Fehler als Ergebnis-Typen.
- Mobil-Budget beachten: kein unnötiges Kopieren von Gewichten, mmap, Adapter lazy laden.
- Logs ohne Prompts/Antworten/Adapter-Inhalte.
- Tests: `cargo nextest run`, Property-Tests für Policy und Fold, `cargo deny check`, Fuzzing für Paket-Parser.
- Kein `git push` ohne Freigabe.
