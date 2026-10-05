# core_restatify-tensegrity-ai-slm

**Tensegrity AI SLM – Core**: der Rust-Kern auf dem Gerät. Lokale Inferenz mit einem Open-Source-SLM, Experten-Router,
Leerlauf-Training von LoRA-Adaptern, Falten der Deltas je Vertraulichkeitsstufe, signierte Updates und P2P-Austausch.
Wird von der Uno-App per FFI im selben Prozess genutzt.

## Geplante Crates

| Crate | Aufgabe |
|---|---|
| `tg-model` | Inferenz (llama.cpp/GGUF), LoRA-Hot-Swap |
| `tg-experts` | Router, Experten-Auswahl, Slot-Registry-Spiegel |
| `tg-signals` | Feedback-Ereignisse (Daumen, Korrektur mit Beleg, positive Rückmeldung) |
| `tg-verify` | Prüfung neuer Informationen gegen Experten |
| `tg-learn` | Leerlauf-LoRA-Training, maskierte Gradienten |
| `tg-policy` | Vertraulichkeitsstufen, Berechtigungsmaske, Taint |
| `tg-vault` | verschlüsselter lokaler Speicher (W_PI, Verlauf) |
| `tg-fold` | Clipping, Kompression, Gauß-Rauschen, Privacy Accountant |
| `tg-sync` | Update-Request, Delta-Pakete, TUF-Prüfung, Rollback |
| `tg-p2p` | libp2p-Mesh |
| `tg-ffi` | Bindings für C#/Uno |
| `relay` | P2P-Relay-Dienst |

## Zielplattformen

Windows, macOS, Linux, Android, iOS (WebAssembly nutzt das Cloud-Backend).

## Dokumentation

Siehe [docs_restatify-tensegrity-ai](https://github.com/carryman1979/docs_restatify-tensegrity-ai).

## Lizenz

Apache License 2.0 – siehe [LICENSE](LICENSE).
