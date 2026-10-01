# AGENTS.md – Anweisungen für KI-Assistenten

Diese Datei ist die **einzige Quelle der Wahrheit** für alle KI-Werkzeuge in diesem Repo
(Claude Code, OpenAI Codex, GitHub Copilot, Gemini CLI, Cursor, Aider …).
`CLAUDE.md`, `GEMINI.md`, `.github/copilot-instructions.md` und `.cursor/rules/` verweisen nur hierher.
Änderungen an Regeln **nur hier** vornehmen.

## Projekt in einem Satz
LabelLab: Rust-Label-Software für Brother PT-P710BT (und verwandte Modelle), druckt direkt per
Bluetooth/USB ohne Brother-Treiber, mit Kommandozeile und Tauri-2-Oberfläche, für Windows und Linux.

## Pflicht beim Sessionstart (in dieser Reihenfolge lesen)
1. `docs/PROGRESS.md`: aktueller Stand, laufender Meilenstein, offene Aufgaben, letzte Session
2. `docs/DECISIONS.md`: getroffene Architekturentscheidungen (nicht stillschweigend umwerfen)
3. `docs/MASTER_PROMPT.md`: Ziel, Architektur, Features, Meilensteine
4. `docs/PROTOCOL.md`: Druckerprotokoll (verifizierte vs. unverifizierte Werte)
5. Bei Wartungsarbeiten: `docs/MAINTENANCE.md`

## Pflicht beim Sessionende
- `docs/PROGRESS.md` aktualisieren: Meilenstein-Checkboxen, „Nächste Schritte“, neuen Eintrag im
  Session-Log (Datum, Werkzeug/Modell, was erledigt, was offen, Stolpersteine).
- Neue Architekturentscheidungen als ADR in `docs/DECISIONS.md` eintragen.
- Neue Protokoll-Erkenntnisse in `docs/PROTOCOL.md` (mit Quelle: Doku, Hardware-Test oder Annahme).
- Nutzerrelevante Änderungen in `CHANGELOG.md` unter `[Unreleased]`.
- Arbeit committen. Keine halbfertigen, nicht kompilierenden Stände auf `main`.

## Übergabe zwischen Agenten
- Arbeite so, dass ein anderer Agent **ohne Chatverlauf** weitermachen kann: Der Stand steht in
  den Dateien, nicht in deinem Kontext.
- Eine Aufgabe in Arbeit wird in `PROGRESS.md` unter „In Arbeit“ mit Werkzeugname markiert,
  damit parallel arbeitende Agenten sich nicht überschneiden.
- Größere Aufgaben auf eigenem Branch `feat/<thema>` oder `fix/<thema>`, Merge per PR.

## Build & Test
```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test --workspace -F hw-tests -- --device COM9   # nur mit echtem Drucker
cd app && npm install && npm run tauri dev            # GUI
cd app && npm run build                               # GUI-Frontend: Typecheck + Build
cd app/src-tauri && cargo clippy --all-targets -- -D warnings   # GUI-Backend (eigener Workspace)
```
Linux braucht `libdbus-1-dev` (BlueZ/`bluer`) und `libudev-dev`, für die GUI zusätzlich
WebKitGTK: `libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev libayatana-appindicator3-dev
libxdo-dev` (siehe CI-Jobs `test` und `app`).
Ein Task gilt erst als fertig, wenn fmt, clippy und Tests grün sind.

## Code-Regeln
- Bezeichner, Kommentare und Commit-Messages auf Englisch. UI-Texte und Doku auf Deutsch (UI über i18n-Schlüssel).
- Bibliotheks-Crates (`ll-*`): kein `unwrap()`/`expect()` außer in Tests, Fehler mit `thiserror`.
  `anyhow` nur in Binaries.
- Protokoll- und Geometriewerte nur in `ll-protocol` (Modelltabelle), keine Magic Numbers anderswo.
- **Unverifizierte Protokolldetails nicht raten**: `// TODO(verify): …` setzen, in `PROTOCOL.md`
  als unverifiziert führen und einen Hardware-Test in `PROGRESS.md` unter „Hardware-Tests offen“ eintragen.
- Vorschau und Druck nutzen **denselben** Renderpfad.
- Neue Abhängigkeiten nur mit Begründung im Commit oder ADR, lizenzkompatibel mit MIT.
- Commits im Stil Conventional Commits: `feat(protocol): …`, `fix(transport): …`, `docs: …`.

## Grenzen
- Keine Marken („Brother“, „P-touch“, „Cube“) in Produktname, Logo oder Icons.
- Keine Brother-Binärdateien, -Treiber oder -Dokumente ins Repo einchecken.
- Keine Secrets, Gerätenamen/MAC-Adressen nur als Beispiele.
