# Architekturentscheidungen (ADR-Log)

> Kurzform-ADRs. Neue Einträge unten anhängen, fortlaufend nummerieren. Eine Entscheidung wird nicht
> gelöscht, sondern mit Status „ersetzt durch ADR-xxx“ markiert.

Vorlage:
```
## ADR-000: Titel
- Datum / Status: JJJJ-MM-TT · vorgeschlagen | angenommen | ersetzt durch ADR-xxx
- Kontext: Warum musste entschieden werden?
- Entscheidung: Was wurde entschieden?
- Konsequenzen: Was folgt daraus (positiv/negativ)?
```

## ADR-001: Direktdruck ohne Brother-Treiber
- Datum / Status: 2026-10-01 · angenommen
- Kontext: Brother unterstützt unter Windows für den PT-P710BT nur USB. Bluetooth geht nur über manuelle
  COM-Port-Umstellung des Treibers. Der P-touch Editor ist langsam.
- Entscheidung: LabelLab erzeugt die Rasterdaten selbst und sendet sie über das Brother-Raster-Protokoll
  direkt an den Drucker. Kein Windows-Spooler, keine Treiberinstallation.
- Konsequenzen: Volle Kontrolle und Geschwindigkeit, plattformunabhängig. Protokoll muss selbst gepflegt
  und für neue Modelle verifiziert werden. Drucken aus Fremdprogrammen (Word usw.) ist kein Ziel.

## ADR-002: Native Bluetooth-Verbindung statt virtueller COM-Ports
- Datum / Status: 2026-10-01 · angenommen
- Kontext: Windows legt pro Kopplung mehrere SPP-COM-Ports an (ein- und ausgehend), deren Nummern sich ändern.
- Entscheidung: Windows über WinRT RFCOMM (`StreamSocket`) inkl. Kopplung aus der App. Linux über BlueZ.
  COM-Ports nur als Fallback.
- Konsequenzen: Robuste Verbindung ohne Nutzereingriff. Zwei plattformspezifische Implementierungen hinter
  einem gemeinsamen `Transport`-Trait.

## ADR-003: Tauri 2 für die Oberfläche
- Datum / Status: 2026-10-01 · angenommen
- Kontext: Schnelle, kleine Desktop-App für Windows und Linux, Rust-Backend gewünscht.
- Entscheidung: Tauri 2 mit Svelte 5 + TypeScript. Kernlogik in Rust-Crates, Frontend nur Darstellung.
- Konsequenzen: Kleine Binaries, Web-UI-Komfort. Vorschau-Rendering erfolgt in Rust (gleicher Pfad wie Druck)
  und wird als Bild an das Frontend gegeben.

## ADR-004: Multi-Agent-Arbeitsweise mit dateibasiertem Gedächtnis
- Datum / Status: 2026-10-01 · angenommen
- Kontext: Entwicklung mit wechselnden KI-Werkzeugen (Claude, Codex, Copilot, Gemini, Cursor).
- Entscheidung: `AGENTS.md` als einzige Regelquelle, dünne Verweisdateien je Werkzeug. Fortschritt in
  `docs/PROGRESS.md`, Entscheidungen hier, Wartung in `docs/MAINTENANCE.md`.
- Konsequenzen: Jeder Agent kann ohne Chatverlauf übernehmen. Disziplin nötig: Dateien am Sessionende pflegen.
