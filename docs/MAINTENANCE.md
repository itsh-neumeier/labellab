# Wartungsanleitung

Für Menschen und KI-Agenten. Jede Aufgabe hier hat eine Checkliste. Erst abhaken, wenn alles grün ist.

## 1. Abhängigkeiten aktualisieren (monatlich oder bei Sicherheitsmeldung)
- [ ] `cargo update` und `cargo outdated` (via `cargo install cargo-outdated`) prüfen
- [ ] `cargo audit` (via `cargo install cargo-audit`) ohne offene Warnungen
- [ ] `cargo deny check licenses` (nur MIT-kompatible Lizenzen)
- [ ] Frontend: `cd app && npm outdated && npm audit`
- [ ] Tauri-Major-Updates nur auf eigenem Branch, Migrationshinweise von Tauri lesen
- [ ] fmt, clippy, Tests grün → Commit `chore(deps): …`, Eintrag in `CHANGELOG.md`

## 2. Neues Druckermodell hinzufügen
- [ ] USB VID/PID, Series-/Model-Code (Statusbyte 3/4) und Kopfbreite ermitteln
      (Quelle: Brother Raster Command Reference oder Status-Dump vom Gerät)
- [ ] Eintrag in der Modelltabelle von `ll-protocol` (keine Sonderlogik, wenn vermeidbar)
- [ ] Bandgeometrie je Breite (bedruckbare Pins, Offset) eintragen
- [ ] Unterstützte Funktionen kennzeichnen (Auto-Cut, Half-Cut, Chain-Print, hohe Auflösung, Kompression)
- [ ] Status-Dump als Test-Fixture ablegen (`crates/ll-protocol/tests/fixtures/<modell>/`)
- [ ] Hardware-Test durchführen oder als offen in `PROGRESS.md` eintragen
- [ ] README-Kompatibilitätsliste und `PROTOCOL.md` aktualisieren

## 3. Protokollfehler / Druckbild falsch
1. Rohdaten mitschneiden: `labellab print … --dump job.bin` bzw. Debug-Option in der GUI
2. Status vor und nach dem Druck: `labellab status --json`
3. Mit Golden-Fixture vergleichen (`cargo test -p ll-protocol golden`)
4. Ursache und Fix in `PROTOCOL.md` dokumentieren, Regressionstest hinzufügen

## 4. Bluetooth-Probleme (Support-Leitfaden)
- Drucker nimmt nur **eine** BT-Verbindung an: Handy-Bluetooth aus
- Windows: `tools/bt-diagnose.ps1` ausführen (zeigt COM-Ports ein-/ausgehend und fragt Status ab)
- Neu koppeln, PIN `0000`
- Linux: `bluetoothctl` → `pair`, `trust`, dann `labellab devices`

## 5. Release erstellen
- [ ] `PROGRESS.md` und `CHANGELOG.md` aktuell, `[Unreleased]` → `[x.y.z] – Datum`
- [ ] Version in `Cargo.toml` (Workspace), `app/src-tauri/tauri.conf.json`, `app/package.json` gleich setzen
- [ ] Tag `vX.Y.Z` pushen → CI baut MSI/NSIS (Windows) und AppImage/.deb (Linux)
- [ ] Release-Artefakte auf beiden Plattformen kurz testen (Start, Status, ein Druck)
- [ ] Release-Notes aus `CHANGELOG.md`

## 6. Versionierung
- SemVer. Vor 1.0 dürfen Minor-Versionen inkompatibel sein.
- Das Vorlagenformat `.llabel` hat ein eigenes Feld `format_version`. Ältere Versionen immer lesbar halten
  (Migration im Code + Test-Fixture je Formatversion).

## 7. Doku-Pflege
- `AGENTS.md`: Regeln (einzige Quelle für alle KI-Werkzeuge)
- `docs/PROGRESS.md`: Stand und Session-Log, nach jeder Session
- `docs/DECISIONS.md`: Architekturentscheidungen
- `docs/PROTOCOL.md`: Protokoll, jeder Wert mit Quelle und Status (verifiziert/unverifiziert)
