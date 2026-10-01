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

## ADR-005: Async `Transport`-Trait mit `tokio`/`async-trait`
- Datum / Status: 2026-10-01 · angenommen
- Kontext: `MASTER_PROMPT.md` skizziert den `Transport`-Trait bereits async (Bluetooth-/USB-I/O ist
  nicht-blockierend sinnvoller, Tauri-Backend läuft ohnehin async).
- Entscheidung: `ll-transport` hängt von `tokio` (nur `rt-multi-thread`, `macros`, `time`,
  `io-util`) und `async-trait` ab. `ll-core`/`ll-cli` übernehmen das transitiv.
- Konsequenzen: Einheitliches async-Modell über alle Transport-Implementierungen (Serial, BT,
  USB, Mock). Etwas größere Abhängigkeitsfläche; MIT/Apache-2.0-kompatibel.

## ADR-006: `tokio-serial` für den Serial-Transport
- Datum / Status: 2026-10-01 · angenommen
- Kontext: M2 braucht einen Serial-Transport (BT-SPP-Fallback über COM-Port), der zum async
  `Transport`-Trait (ADR-005) passt.
- Entscheidung: `tokio-serial` (wrapt `serialport`, liefert direkt eine async `SerialStream`
  mit `AsyncRead`/`AsyncWrite`) statt `serialport` + manuellem `spawn_blocking`.
- Konsequenzen: Weniger eigener Glue-Code. Auf Linux braucht `serialport` zur Port-Erkennung
  `libudev-dev` zur Build-Zeit → CI-Workflow installiert das für `ubuntu-latest`. MIT-lizenziert.

## ADR-007: Natives WinRT-RFCOMM (M4) vor M3 vorgezogen, kein eigener Treiber
- Datum / Status: 2026-10-01 · angenommen
- Kontext: Der serielle BT-SPP-Fallback aus M2 (ADR-006) scheitert auf der Zielhardware
  reproduzierbar mit `ERROR_SEM_TIMEOUT` – sowohl mit .NET `SerialPort` als auch mit
  `tokio-serial`, also kein Fehler in unserem Code, sondern eine strukturelle Schwäche der
  virtuellen-COM-Port-Kompatibilitätsschicht von Windows für Bluetooth SPP (genau das, was
  ADR-002 schon als Risiko benannt hatte). Diskutiert wurde auch ein eigener (Klon-)
  Bluetooth-Treiber – verworfen: widerspricht der nicht verhandelbaren Anforderung „kein
  Treiber nötig“ (`AGENTS.md`), bräuchte Kernel-Signing und Installation.
- Entscheidung: `ll-transport::bluetooth` (Windows: `windows`-Crate, `Devices.Bluetooth.Rfcomm`
  + `Networking.Sockets.StreamSocket`) implementiert, noch ohne programmatisches Pairing
  (Gerät muss in Windows bereits gekoppelt sein). Neue Abhängigkeit `windows` 0.58, nur für
  `cfg(windows)`. Linux/BlueZ und USB (`nusb`) bleiben als M4-Rest offen.
- Konsequenzen: Umgeht die kaputte virtuelle-COM-Schicht komplett, kein Treiber, keine
  Installation, keine Admin-Rechte. `windows` 0.58 hat kein `.await` für WinRT-Async-Operationen
  → `.get()` (blockierend) verwendet; vor Einsatz im Tauri-GUI (M6) auf `spawn_blocking`
  umstellen. `read_exact_timeout`s Timeout wird für BT aktuell nicht erzwungen (TODO im Code).
  MIT/Apache-2.0-kompatibel.

## ADR-008: `fontdue` + Systemschriften statt `cosmic-text` für M5-Text
- Datum / Status: 2026-10-01 · angenommen
- Kontext: M3s Platzhalter-Pixelfont (5x5-Stencil) sollte durch echte Typografie ersetzt werden.
  `MASTER_PROMPT.md` nennt `cosmic-text` oder `fontdue`+`ttf-parser` als Optionen.
- Entscheidung: `fontdue` (reiner Rasterizer, schnell, einfache API) statt `cosmic-text`
  (komplexer, für Mehrzeilen-/Bidi-/Shaping-Layout gedacht, das M5 noch nicht braucht –
  aktuell nur einzeilig). Schriftdatei kommt von `ll_render::fontsrc`: sucht eine kurze Liste
  bekannter Systemschrift-Pfade pro OS (Windows: Segoe UI/Arial/Calibri/Tahoma; Linux:
  DejaVu/Liberation/Noto), keine Bündelung einer eigenen Schriftdatei (keine Lizenzfragen,
  kein Download nötig). `image`-Crate (nur `png`-Feature) für PNG-Export (`ll_render::png`,
  CLI `render`).
- Konsequenzen: Kein Font-Familien-/Gewicht-/Fallback-System bisher (nur ein Default-Font),
  kein Mehrzeilen-/Bidi-Layout (kommt erst mit dem M6-Editor, dann ggf. Wechsel zu
  `cosmic-text`). CI installiert `fonts-dejavu-core` auf `ubuntu-latest`, damit der
  Linux-Pfad deterministisch eine Schrift findet (sonst würden Font-Tests dort lautlos
  übersprungen, siehe `crates/ll-render/src/text.rs`). MIT-lizenziert (`fontdue`, `image`).

## ADR-009: `qrcode`-Crate für QR-Codes (M5)
- Datum / Status: 2026-10-01 · angenommen
- Kontext: M5 braucht QR-Codes (siehe `MASTER_PROMPT.md` Feature 4 „Barcodes & QR“). Lineare
  Barcodes (Code128/EAN/...) bleiben vorerst offen.
- Entscheidung: `qrcode`-Crate (reine Rust-Implementierung, MIT, liefert die Modul-Matrix direkt
  als `Color::Dark`/`Light`-Indizierung) statt `rxing` (deutlich größer, für
  Encode+Decode/mehrere Symbologien gedacht, hier wird nur Encodieren gebraucht).
  `ll_render::barcode::render_qr()` rastert die Matrix direkt ins `Bitmap`, gleiche
  Pin-/Rasterzeilen-Orientierung wie `text`. `ll-core::print::print_qr()` teilt sich die
  Protokoll-Sequenz mit `print_text()` (gemeinsame `send_bitmap()`-Hilfsfunktion).
- Konsequenzen: Ruhezone (Quiet Zone) ist 2 Module statt der spec-üblichen 4 (spart Band).
  Hardware-Test 2026-10-01: trotzdem mit einem Handy scannbar (ein Gerät/eine App getestet,
  nicht erschöpfend geprüft). Fehlerkorrekturstufe aktuell fest auf `Medium` in der CLI, noch
  keine `--ec-level`-Option. MIT-lizenziert.
