# Fortschritt

> Gemeinsamer Fortschrittsspeicher für Menschen und KI-Agenten. Wird **am Ende jeder Session**
> aktualisiert (siehe `AGENTS.md`). Neueste Einträge im Session-Log oben.

## Aktueller Stand
- **Phase:** M1/M2 abgeschlossen. M4-Teilstand (natives Windows-BT-RFCOMM) hardware-verifiziert.
  **M3 hardware-verifiziert:** `labellab print "TEST" --bt` hat echten, lesbaren Text auf
  9-mm-Band gedruckt (Foto vom Nutzer bestätigt, 2026-10-01). Komplette Pipeline (Status lesen →
  Band erkennen → Platzhalter-Font rendern → PackBits → natives BT → Drucker) funktioniert
  end-to-end auf echter Hardware.
- **Aktueller Meilenstein:** M5 läuft — echte Schriften und QR-Codes beide hardware-verifiziert
  (QR gedruckt und mit Handy erfolgreich gescannt, Link öffnete). Rahmen/lineare
  Barcodes/Bilder/Symbole offen.
- **Letzte Aktualisierung:** 2026-10-01

## Meilensteine
- [x] **M1 – Grundgerüst:** Workspace, Crates, CI, `docs/PROTOCOL.md`, `ARCHITECTURE.md`
- [x] **M2 – Protokoll + Status:** Status-Parser, Befehle, PackBits, Serial-Transport, CLI `devices`/`status`
  (Code + Unit-Tests grün; serieller BT-SPP-Pfad auf Zielhardware unzuverlässig, siehe ADR-007 –
  native BT, s. M4-Teilstand, ist der verifizierte Weg)
- [x] **M3 – Erster Druck:** Textlabel per CLI, Mock-Transport, Golden-Tests.
  `ll_core::print::print_text` (Status lesen → Bandbreite prüfen → Platzhalter-Bitmapfont
  rendern → PackBits → Protokollbefehle senden), CLI `print "Text" --device <...> [--bt]
  [--cut] [--copies N]`, 3 Golden-/Strukturtests mit `MockTransport`.
  **Hardware-verifiziert 2026-10-01:** echter, lesbarer Druck auf 9-mm-Band bestätigt (Foto).
- [ ] **M4 – Native Bluetooth/USB:** WinRT-RFCOMM inkl. Kopplung, BlueZ, USB (`nusb`)
  **Teilstand:** WinRT-RFCOMM-Connect (ohne programmatisches Pairing) fertig, **hardware-verifiziert
  gegen echten PT-P710BT** (2026-10-01, Gerät „SPP SERVER“/`b4:22:00:eb:96:6f`). Kopplung aus der
  App, BlueZ (Linux), USB fehlen noch.
- [ ] **M5 – Renderer komplett:** Schriften, Rahmen, Barcodes/QR, Bilder, Symbole, `render` → PNG
  **Teilstand:** echte Systemschriften (`fontdue`, `ll_render::fontsrc`/`text`, **hardware-
  verifiziert**) und QR-Codes (`qrcode`-Crate, `ll_render::barcode::render_qr`, ADR-009,
  **noch nicht auf Band gedruckt**) fertig. `ll_render::png` + CLI `render ["Text"|--qr <daten>]
  -o x.png --width <mm>`, `print` ebenso mit `--qr`. `print_text`/`print_qr` teilen sich jetzt
  die Protokoll-Sequenz (`ll-core::print::send_bitmap()`). Noch offen: Rahmen/Linien, lineare
  Barcodes (Code128/EAN/...), Bilder (PNG/JPG/BMP/SVG, Dithering), Symbolbibliothek.
- [ ] **M6 – Tauri-GUI:** Geräteleiste mit Bandstatus, Editor, Live-Vorschau, Vorlagen
- [ ] **M7 – Kabel/Serien/CSV + Kettendruck**
- [ ] **M8 – Release v1.0.0:** Installer, Doku, Screenshots

## In Arbeit
| Aufgabe | Wer (Werkzeug/Person) | Branch | Seit |
|---|---|---|---|
| – | – | – | – |

## Nächste Schritte
1. **Hardware-Test QR (auf dem Gerät mit dem Drucker):** `labellab print --qr "https://..."
   --bt --device <ID>` — druckt, scannt der Code mit einem Handy? Ruhezone ist mit 2 statt der
   üblichen 4 Modulen knapp bemessen (ADR-009), ggf. nachjustieren falls Scan unzuverlässig.
2. M5 weiter: Rahmen/Linien, lineare Barcodes (Code128/EAN/Code39 via `barcoders`), Bilder
   (Import + Floyd-Steinberg-Dithering via `image`), Symbolbibliothek.
3. M4-Rest (programmatisches Pairing, BlueZ/Linux, USB `nusb`) — wann immer eingeschoben.
4. `--cut` (Auto-Cut) und `--copies N` (Mehrfachdruck) hardware-testen — bisher nur der
   Einzeldruck ohne Schnitt verifiziert.
5. Medientyp-/Farbcode-Bedeutung (Byte 11/24/25) gegen Brothers Farbcode-Tabelle prüfen
   (aktuelle Werte: `0x01`/`0x01`/`0x08`, siehe `docs/PROTOCOL.md`).

## Hardware-Tests offen
> Tests, die nur mit echtem Drucker beantwortet werden können. Ergebnis in `PROTOCOL.md` übertragen.

- [ ] Pin-Offsets/bedruckbare Pins je Bandbreite (3,5 / 6 / 9 / 12 / 18 / 24 mm) bestätigen
- [ ] Status-Byte für Akkustand vorhanden?
- [ ] Half-Cut am PT-P710BT unterstützt? (vermutlich nein)
- [ ] Maximale Bluetooth-Durchsatzrate / sinnvolle Blockgröße beim Senden der Rasterdaten
- [ ] Medientyp-Code `0x01` (Byte 11), Bandfarbe `0x01` (Byte 24), Schriftfarbe `0x08` (Byte 25)
  gegen Brothers Farbcode-Tabelle decodieren (vermutlich laminiert/schwarz auf weiß)
- [x] ~~M2: seriellen BT-SPP-Fallback testen~~ – auf Zielhardware reproduzierbar defekt
  (`ERROR_SEM_TIMEOUT`), siehe ADR-007. Durch natives BT-RFCOMM ersetzt (nächster Punkt).
- [x] ~~M4: `labellab status --bt` gegen echten Drucker testen~~ – erfolgreich, 2026-10-01
  (Gerät „SPP SERVER“, PT-P710BT), siehe `docs/PROTOCOL.md`.
- [x] ~~M3: `labellab print "Text" --bt --device <ID>` gegen echten Drucker testen~~ –
  erfolgreich, 2026-10-01: lesbarer "TEST"-Druck auf 9-mm-Band (Foto bestätigt).
- [ ] `--cut` (Auto-Cut) hardware-testen — bisher nur ohne Schnitt gedruckt.
- [ ] `PrintInformation`-Validitätsflags (n1) jenseits des gesendeten Bits (Medientyp/-länge
  gültig, Qualität/Recovery) gegen echtes Verhalten prüfen.
- [x] ~~M5: `labellab print "Text" --bt` mit der neuen Systemschrift (`fontdue`) gegen echten
  Drucker testen~~ – erfolgreich, 2026-10-01: sauberer "TEST"-Druck in echter Schrift (Foto im
  Vergleich zum alten M3-Pixelfont bestätigt deutliche Verbesserung).
- [x] ~~M5: `labellab print --qr "..." --bt` gegen echten Drucker testen~~ – erfolgreich,
  2026-10-01: gedruckt und mit Handy gescannt, Link öffnete trotz schmaler Ruhezone (2 statt
  der spec-üblichen 4 Module, siehe ADR-009).

## Bekannte Fakten aus der Hardware
- 2026-10-01: Statusabfrage (`00×100, 1B 40, 1B 69 53`) über Windows-Bluetooth-COM-Port (ausgehend) beantwortet,
  32 Byte, Byte0 `0x80`, Bandbreite korrekt (9 mm). Eingehende BT-COM-Ports sind unbrauchbar.
- 2026-10-01: Drucker akzeptiert nur **eine** Bluetooth-Verbindung gleichzeitig (Handy blockiert PC).
- 2026-10-01: Auf der konkreten Testmaschine (Nutzer-PC mit gekoppeltem PT-P710BT) scheitert der
  serielle BT-SPP-Fallback zuverlässig: `bt-diagnose.ps1` (.NET `SerialPort`) **und** `labellab`
  (`tokio-serial`) bekommen auf allen drei ausgehenden virtuellen COM-Ports (COM6/COM7/COM9)
  `ERROR_SEM_TIMEOUT` bzw. (COM9, vom Druckertreiber selbst belegt) `ERROR_INVALID_FUNCTION`/
  `ERROR_FILE_NOT_FOUND`. Zwei unabhängige Stacks scheitern identisch → kein Code-Fehler, die
  virtuelle-COM-Port-Kompatibilitätsschicht selbst baut die RFCOMM-Verbindung nicht auf. Deshalb
  M4 (natives WinRT-RFCOMM) vorgezogen, siehe ADR-007.
- 2026-10-01: Natives WinRT-RFCOMM erfolgreich gegen echten PT-P710BT getestet (`labellab status
  --bt`). Der Drucker ist in Windows als **zwei** Geräte sichtbar: „Brother PT-P710BT“
  (Druckerklasse/Treiber, für uns irrelevant) und „PT-P710BT5265“ (das eigentliche
  Bluetooth-Gerät). Die `RfcommDeviceService`-Enumeration listet ihn aber unter dem vom Drucker
  selbst gemeldeten SPP-Dienstnamen **„SPP SERVER“**, nicht unter dem Gerätenamen — Identifikation
  nur über die MAC in der Geräte-ID möglich (`b4:22:00:eb:96:6f`, passt zur Hardware-ID
  `BTHENUM\Dev_B42200EB966F` aus dem Geräte-Manager). Status: `media_width_mm=9, error1=0,
  error2=0, media_type=1, status_type=Antwort, phase_type=0, tape_color=1, text_color=8`.
- 2026-10-01: Erster echter Druck erfolgreich. `labellab print "TEST" --device "SPP SERVER"
  --bt` druckte lesbaren Text auf 9-mm-Band (Foto vom Nutzer bestätigt). Vollständige Pipeline
  (Invalidate/Initialize/Status → Raster-Modus → Various Mode → Rand(0) → PrintInformation →
  PackBits-Kompression → Rasterzeilen → Druck mit Vorschub) funktioniert mit den aktuellen,
  bis dahin nur dokumentierten (nicht hardware-verifizierten) Befehls-Bytes. Getestet: ohne
  Auto-Cut, ohne mehrere Kopien, mit dem M3-Platzhalter-Bitmapfont (kein echter Renderer).
- 2026-10-01: Echte Systemschrift (`fontdue`, M5) gegen echten Drucker getestet:
  `labellab print "TEST" --bt` druckte sauberen, proportionalen Text — Foto-Vergleich mit dem
  alten M3-Pixelfont-Druck bestätigt deutlich bessere Lesbarkeit/Optik.
- 2026-10-01: QR-Code (M5) gegen echten Drucker getestet: `labellab print --qr "https://..."
  --bt` druckte einen sauberen QR-Code auf 9-mm-Band, mit Handy gescannt — Link öffnete. Die
  knappe Ruhezone (2 statt der spec-üblichen 4 Module, ADR-009) war in diesem Test kein Problem.

## Session-Log
### 2026-10-01 – Claude Code (Sonnet 5), M5 (Teil) – QR-Codes
- `ll-render`: neues `barcode`-Modul (`render_qr()` via `qrcode`-Crate, siehe ADR-009). Rastert
  die Modul-Matrix direkt ins `Bitmap`, gleiche Pin-/Raster-Zeilen-Orientierung wie `text`.
  Modulgröße automatisch an `printable_pins` angepasst (Ruhezone 2 Module statt der
  spec-üblichen 4, TODO(verify) Scanbarkeit). Visuell per Wegwerf-Beispiel geprüft: korrekte
  QR-Struktur (drei Finder-Pattern erkennbar), danach entfernt.
- `ll-core::print`: auf gemeinsame `read_status_and_geometry()`/`send_bitmap()`-Hilfsfunktionen
  refaktoriert, `print_text()` und neues `print_qr()` teilen sich jetzt die Protokoll-Sequenz
  statt sie zu duplizieren.
- `ll-cli`: `--qr <daten>` auf `print` und `render` (schließt sich mit Text-Argument aus,
  `clap conflicts_with`), `Content`-Enum für die Text-oder-QR-Unterscheidung, `ConnectOpts`
  bündelt `--device`/`--bt`/`--baud` (sonst `clippy::too_many_arguments`). Smoke-getestet:
  `render --qr "https://..." -o out.png` erzeugt gültig aussehenden QR-Code.
- `cargo fmt`/`clippy -D warnings`/`test --workspace` grün (34 Unit-Tests, vorher 30).
- **Hardware-Test erfolgreich:** Nutzer hat `labellab print --qr "..." --bt` gedruckt und mit
  Handy gescannt — Link öffnete trotz schmaler Ruhezone (2 statt 4 Module).
- **Noch offen in M5:** Rahmen/Linien, lineare Barcodes (Code128/EAN/...), Bilder,
  Symbolbibliothek.

### 2026-10-01 – Claude Code (Sonnet 5), M5 (Teil) – echte Schriften + PNG-Vorschau
- `ll-render`: M3-Platzhalter-Font (`font.rs`) entfernt. Neu: `fontsrc` (sucht eine kurze Liste
  bekannter Systemschrift-Pfade pro OS — Windows: Segoe UI/Arial/Calibri/Tahoma; Linux:
  DejaVu/Liberation/Noto — lädt die erste gefundene Datei), `text::render_text()`/
  `render_text_with_font()` (echtes Rasterizing via `fontdue`, Schriftgröße automatisch an
  `printable_pins` angepasst, gleiche Pin-/Raster-Zeilen-Orientierung wie zuvor), `png::to_png()`
  (Bitmap → PNG für Vorschau, gleicher Renderpfad wie der Druck). Neue Abhängigkeiten `fontdue`,
  `image` (nur `png`-Feature), siehe ADR-008.
- Visuell per Wegwerf-Beispiel geprüft: "HELLO LabelLab 123" sauber und lesbar in echter
  Systemschrift (Segoe UI/Arial auf dieser Maschine) gerendert, danach entfernt.
- `ll-core::print::print_text()`: `render_text()`-Aufruf an neue `Result`-Signatur angepasst
  (Fehler propagieren über `CoreError::Render`, z. B. wenn keine Systemschrift gefunden wird).
- `ll-cli`: `render "Text" -o datei.png [--width mm] [--model ...]` echt implementiert
  (provisorisch: nimmt reinen Text statt eines `.llabel`-Vorlagenformats, das kommt erst mit
  dem M6-GUI-Editor — `--width` nötig, weil ohne Drucker keine Bandbreite abfragbar ist).
  Smoke-getestet: `render "LabelLab M5" -o out.png --width 12` erzeugt lesbares PNG.
- CI: `fonts-dejavu-core` zusätzlich zu `libudev-dev` auf `ubuntu-latest` installiert, damit
  die Font-Tests dort nicht mangels Systemschrift übersprungen werden.
- `cargo fmt`/`clippy -D warnings`/`test --workspace` grün (30 Unit-Tests, unverändert in der
  Zahl — Font-Tests ersetzen die alten Platzhalter-Font-Tests 1:1).
- **Hardware-Test erfolgreich:** Nutzer hat `labellab print "TEST" --bt` mit der neuen Schrift
  gedruckt, Foto im Vergleich zum alten Pixelfont-Druck bestätigt sauberen, proportionalen Text.
- **Noch offen in M5:** Rahmen/Linien, Barcodes/QR/DataMatrix, Bilder, Symbolbibliothek.

### 2026-10-01 – Claude Code (Sonnet 5), M3 – Erster Druck (Code)
- `ll-render`: `font` (Platzhalter-5x5-Pixelstencil: Leerzeichen, 0-9, A-Z, `. , - : !`,
  Groß-/Kleinschreibung gleich; explizit **nicht** der echte M5-Renderer), `text::render_text()`
  (rendert Spalte für Spalte direkt in ein `Bitmap`, pins=Bandbreite-Achse, raster
  lines=Vorschubachse, Schriftgröße automatisch an `printable_pins` angepasst). Visuell per
  Wegwerf-Beispiel geprüft (ASCII-Dump von "HELLO 123") – Buchstabenformen (H, E) sahen korrekt
  aus, danach Beispiel wieder entfernt.
- `ll-protocol::model`: `find_by_name()` neu (Modell per Namen nachschlagen, z. B. für
  CLI `--model`).
- `ll-core::print`: `print_text()` – Ablauf wie in `docs/PROTOCOL.md` dokumentiert (Invalidate →
  Initialize → Status lesen → bei Druckerfehler abbrechen, ohne Rasterdaten zu senden → Band
  gegen Modell-Geometrietabelle prüfen (`UnsupportedTapeWidth` falls unbekannte Breite) → Text
  rendern → Raster-Modus/Various-Mode/Rand/PrintInformation/Kompression → Rasterzeilen
  (PackBits, Leerzeilen als `Z`) → Druck mit Vorschub). Neue `CoreError::PrinterError`-Variante.
- 3 Tests mit `MockTransport` (Golden/strukturell, kein hartkodierter Byte-Dump nötig):
  kompletter Befehlsablauf inkl. Anzahl Rasterzeilen, Abbruch ohne Rasterdaten bei
  Druckerfehler, Ablehnung unbekannter Bandbreite.
- `ll-cli`: `print "Text" [--device ...] [--bt] [--baud ...] [--model PT-P710BT] [--cut]
  [--copies N]` echt implementiert; `--template`/`--csv`/`--image` weiterhin Platzhalter
  (M5/M7). `open_transport()`-Helfer in der CLI (seriell vs. nativ BT) extrahiert.
- `cargo fmt`/`clippy -D warnings`/`test --workspace` grün (30 Unit-Tests, vorher 21).
- **Noch nicht gemacht:** echter Druck auf Band getestet (nur Mock-Transport-Tests bisher).
  `PrintInformation`-Validitätsflags und die meisten Various-/Advanced-Mode-Bits sind
  TODO(verify) – nur Auto-Cut-Bit ist dokumentiert/sicher.
- Release-Build erneuert; Nutzer testet `labellab print "..." --bt --device <ID>` auf dem
  Gerät mit dem Drucker.
- **Hardware-Test erfolgreich:** Nutzer hat `labellab print "TEST" --device "<SPP-SERVER-ID>"
  --bt` ausgeführt, Foto vom bedruckten Band bestätigt lesbaren Text. `docs/PROTOCOL.md`:
  alle in `print_text()` verwendeten Befehle (Raster-Modus, Various Mode, Rand,
  PrintInformation, PackBits-Kompression, Rasterzeile, Leerzeile, Druck mit Vorschub) von
  „dokumentiert“ auf „verifiziert“ gehoben; `Advanced Mode` und `Seite ohne Vorschub` bleiben
  „dokumentiert“ (werden von `print_text()` nicht gesendet). 9-mm-Geometrie als „teilweise
  verifiziert“ markiert (druckt korrekt, Pin-Zahlen nicht einzeln nachgemessen).

### 2026-10-01 – Claude Code (Sonnet 5), M4 (Windows-Teil) vorgezogen – natives BT-RFCOMM
- Grund: Hardware-Test von M2 auf der Nutzermaschine scheiterte reproduzierbar am seriellen
  BT-SPP-Fallback (`ERROR_SEM_TIMEOUT`/`ERROR_INVALID_FUNCTION`, siehe „Bekannte Fakten“). Statt
  die kaputte virtuelle-COM-Schicht zu reparieren: M4s nativer Weg vorgezogen, der sie umgeht.
- `ll-transport`: neues `bluetooth`-Modul, `windows`-only (`cfg(windows)`), `BluetoothTransport`
  (verbindet über bereits gekoppeltes Gerät via `RfcommDeviceService`/`StreamSocket`,
  implementiert `Transport`), `list_paired_devices()`. Neue Abhängigkeit `windows` 0.58
  (ADR-007), nur für `cfg(windows)`-Target.
- `windows` 0.58 hat kein `Future`/`.await` für WinRT-Async-Operationen (anders als gehofft) →
  `.get()` (blockierend) verwendet. Für M2-Timeout-Parität fehlt das bei BT noch
  (`read_exact_timeout`s `timeout`-Parameter wird für BT aktuell ignoriert, TODO im Code).
- `ll-core::device`: `query_status`-Logik auf `&mut dyn Transport` verallgemeinert (von Serial
  und Bluetooth gemeinsam genutzt), `list_bluetooth_devices()`/`query_status_over_bluetooth()`
  neu (`cfg(windows)`).
- `ll-cli`: `status --bt` (nativ statt seriell, Gerät dann die ID aus `devices`), `devices`
  listet jetzt auch gekoppelte Bluetooth-Geräte (Text + `--json`).
- `cargo fmt`/`clippy -D warnings`/`test --workspace` grün (21 Unit-Tests).
- **Hardware-Test erfolgreich:** Nutzer hat Repo via `git clone` + `cargo build --release`
  selbst gebaut, Drucker-Gerät in der Bluetooth-Liste identifiziert (meldet sich als „SPP
  SERVER“, nicht als „PT-P710BT“/„PT-P710BT5265“ — nur über die MAC zuzuordnen) und
  `labellab status --device "<ID>" --bt --json` erfolgreich gegen den echten PT-P710BT
  ausgeführt: 9-mm-Band korrekt erkannt, keine Fehler. Details siehe „Bekannte Fakten“ und
  `docs/PROTOCOL.md`.
- Noch offen für M4: programmatisches Pairing, BlueZ (Linux), USB (`nusb`).
- `docs/PROTOCOL.md` entsprechend aktualisiert (BT-Status-Felder jetzt verifiziert, serieller
  BT-SPP-Pfad als unzuverlässig markiert).

### 2026-10-01 – Claude Code (Sonnet 5), M2 – Protokoll + Status
- `ll-protocol`: `command` (Invalidate/Initialize/Status-Request/Raster-Modus/PrintInformation/
  Various-Mode/Margin/PackBits-Auswahl/Rasterzeile/Leerzeile/Print-Befehle), `status`
  (32-Byte-Parser inkl. Hardware-Fixture-Test), `packbits` (TIFF-PackBits-Encoder mit
  Decode-Roundtrip-Tests). Alle unverifizierten Bit-Layouts (`PrintInformation`-Flags) mit
  `TODO(verify)` markiert statt geraten.
- `ll-transport`: `serial` (`SerialTransport` auf `tokio-serial`, `list_ports()`), neue
  Abhängigkeit `tokio-serial` (ADR-006). CI bekommt `libudev-dev`-Installationsschritt für
  Ubuntu, da `serialport` das auf Linux zur Port-Erkennung braucht.
- `ll-core`: `device::list_serial_devices()`, `device::query_status_over_serial()`.
- `ll-cli`: `devices`/`status` echt implementiert (clap + `tokio::main` + `serde_json` für
  `--json`), `print`/`render` bleiben Platzhalter für M3/M5.
- `cargo fmt`/`clippy -D warnings`/`test --workspace` grün (16+2+3 Unit-Tests).
- Hardware-Test versucht: `labellab status --device COM9` auf der Entwicklungsmaschine dieser
  Session scheiterte mit `ERROR_INVALID_FUNCTION` – Nutzer bestätigte, der gekoppelte Drucker
  hängt an einem **anderen** Gerät, COM9 hier ist unrelated. Kein Code-Befund. Echter
  Hardware-Test steht noch aus (siehe „Hardware-Tests offen“).
- Release-Build (`cargo build --release -p ll-cli`) für den Nutzer bereitgestellt, um
  `labellab status --device COM<n>` auf dem Gerät mit dem Drucker zu testen.

### 2026-10-01 – Claude Code (Sonnet 5), M1 – Grundgerüst
- `.git/HEAD` war korrupt (enthielt Reflog-Zeilen statt `ref: refs/heads/main`) → reparieren,
  `refs/heads/main` war unversehrt, kein Datenverlust, `git fsck` danach sauber.
- Cargo-Workspace mit `ll-protocol`, `ll-transport`, `ll-render`, `ll-core`, `ll-cli` angelegt.
  `ll-protocol`: Modelltabelle (PT-P710BT/P715eBT/E720BT). `ll-transport`: `Transport`-Trait
  (async, `tokio`+`async-trait`, siehe ADR-005) + `MockTransport`. `ll-render`: `Bitmap`-Typ.
  `ll-core`: gemeinsamer `CoreError`. `ll-cli`: Subcommands-Gerüst (clap).
- `docs/PROTOCOL.md` aus `MASTER_PROMPT.md` übernommen, Werte mit Status
  verifiziert/dokumentiert/unverifiziert versehen. `docs/ARCHITECTURE.md` neu.
- CI (`.github/workflows/ci.yml`): fmt-check, clippy -D warnings, test auf windows-latest +
  ubuntu-latest.
- `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace` lokal grün.
- Offen: Repo-Remote (`itsh-neumeier/labellab`) noch nicht geprüft/gepusht in dieser Session.

### 2026-10-01 – Claude (claude.ai), Planung
- Brother-Treiber analysiert: USB-ID, Modellcodes, Invalidate-Länge, versteckte BT-Option im Installer
- Bluetooth-Druck unter Windows über COM-Port verifiziert (Diagnose-Skript `tools/bt-diagnose.ps1`)
- Master-Prompt, Name „LabelLab“, Multi-Agent-Struktur (AGENTS.md, PROGRESS, DECISIONS, MAINTENANCE)
- Offen: Repo auf GitHub anlegen und pushen, dann M1
