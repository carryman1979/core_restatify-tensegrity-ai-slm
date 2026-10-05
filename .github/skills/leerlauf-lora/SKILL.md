---
name: leerlauf-lora
description: "Use when implementing or tuning Tensegrity AI idle-time LoRA training on devices: idle conditions (charging, battery, temperature, unmetered network, consent), building training examples from feedback/corrections/verified facts, masked gradients per confidentiality level, LAN desktop fallback. Keywords: leerlauf, idle training, lora training, on-device training, akku, temperatur, feedback training, korrektur, maskierte gradienten."
---
# Leerlauf-LoRA-Training

## Startbedingungen (alle erfüllt)
- Nutzerzustimmung aktiv; Gerät lädt; Akku ≥ 80 % (Richtlinie); Temperatur unter Plattformgrenze; Bildschirm aus bzw. App im Hintergrund erlaubt.
- Abbruch sofort bei Wegfall einer Bedingung; Checkpoint speichern.

## Trainingsdaten
| Quelle | Beispiel | Voraussetzung |
|---|---|---|
| Daumen hoch / positive Rückmeldung | (Prompt, Antwort) als Positivbeispiel | – |
| Daumen runter | Präferenzpaar, wenn Alternative vorliegt | – |
| Korrektur mit Beleg | (Prompt, korrigierte Antwort) | Beleg vorhanden |
| Neue Information | Fakt als Frage/Antwort | per `tg-verify` gegen Experten bestätigt |

Jedes Beispiel trägt seine Stufe (Taint aus dem Gesprächsverlauf).

## Training
1. Beispiele nach Stufe trennen: Streng geheim/Nur lokal → W_PI, sonst W_logik.
2. Rang 8–16, kleine Batches, Gradient Checkpointing; Laufzeitbudget pro Sitzung.
3. Nach dem Lauf lokale Mini-Eval (Halteset); bei Verschlechterung verwerfen.
4. W_logik-Delta an `tg-fold` übergeben; W_PI bleibt im Vault.

## Fallback
Ist On-Device-Training zu langsam/heiß (Spike S2), wird ein Desktop desselben Nutzers im LAN verwendet – Transfer
Ende-zu-Ende-verschlüsselt, gleiche Stufenregeln.

## Messen
Dauer, Energie, Temperaturspitze, Verlust vorher/nachher – mit Gerät/OS/Build protokollieren.
