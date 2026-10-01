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

## ADR-010: `barcoders`-Crate für lineare Barcodes (M5)
- Datum / Status: 2026-10-01 · angenommen
- Kontext: M5 braucht lineare Barcodes (`MASTER_PROMPT.md` Feature 4). Code128 zuerst, weil es
  beliebige ASCII-Daten kodieren kann (im Gegensatz zu EAN/UPC, die feste Ziffernformate mit
  Prüfziffer verlangen) und damit die meisten CLI-Anwendungsfälle abdeckt.
- Entscheidung: `barcoders`-Crate (MIT, reines Rust, `encode()` liefert direkt eine
  Modulbreiten-Sequenz als `Vec<u8>`), nur `std`-Feature (kein `image`/`svg`/`json`, wir bauen
  unser eigenes `Bitmap`). `ll_render::linear_barcode::render_barcode()` füllt pro Balken-Modul
  die **gesamte** bedruckbare Bandbreite (anders als QR/Text: 1D-Barcodes haben keine vertikale
  Struktur). `barcoders` verlangt ein Zeichensatz-Präfix (`À`/`Ɓ`/`Ć`) am Code128-Dateneingang;
  unser Wrapper setzt automatisch Zeichensatz B (allgemein alphanumerisch), wenn der Aufrufer
  keins angibt. `ll-core::print::print_barcode()` teilt sich die Protokoll-Sequenz mit
  `print_text()`/`print_qr()`.
- Konsequenzen: Balkenbreite fest auf 3 Druckpunkte (TODO(verify) gegen echten Scanner).
  MIT-lizenziert. Hardware-Test 2026-10-01 (Code128): druckt sauberes, optisch korrektes
  Balkenmuster; Scan-Lesbarkeit nicht verifiziert (kein Code128-Scanner beim Nutzer verfügbar).
- **Update 2026-10-01:** `barcoders` deckt auch EAN-13, EAN-8, UPC-A (= EAN-13 mit führender
  `0`), Code39 und ITF (interleaved 2-of-5) ab, ohne weitere Abhängigkeit — alle fünf zusätzlich
  implementiert (`Symbology`-Enum, CLI `--barcode-type`). Nur noch über PNG-Vorschau geprüft,
  nicht auf Band gedruckt.

## ADR-011: Eigene Floyd-Steinberg-Dithering-Implementierung für Bilder (M5)
- Datum / Status: 2026-10-01 · angenommen
- Kontext: M5 braucht Bildimport mit Dithering (`MASTER_PROMPT.md` Feature 6). `image`-Crate
  (bereits Abhängigkeit für PNG-Export, ADR-008) deckt Dekodierung (PNG/JPEG/BMP) und Skalierung
  ab, aber kein 1-Bit-Dithering.
- Entscheidung: Features `jpeg`/`bmp` zu `image` hinzugefügt (neben vorhandenem `png`). Klassisches
  Floyd-Steinberg-Error-Diffusion selbst implementiert (~30 Zeilen, kein eigenes Crate nötig,
  Algorithmus ist Standard/gut dokumentiert). `ll_render::picture::render_gray()` ist die reine,
  dateisystemfreie Kernlogik (testbar mit synthetischen `GrayImage`s), `render_image()` nur ein
  dünner Dateilade-Wrapper darum. Bild wird auf `printable_pins` Höhe skaliert (Seitenverhältnis
  erhalten), `--invert`-Flag für Bilder mit hellem Motiv auf dunklem Grund. SVG bleibt offen
  (braucht eigenen Rasterizer wie `resvg`, `image`-Crate kann das nicht).
- Konsequenzen: Keine Symbolbibliothek (separates M5-Los). MIT-lizenziert.

## ADR-012: `resvg`/`usvg`/`tiny-skia` für SVG-Import (M5)
- Datum / Status: 2026-10-01 · angenommen
- Kontext: ADR-011 hatte SVG-Import offen gelassen, weil `image` SVG nicht dekodieren kann.
  `MASTER_PROMPT.md` Feature 6 verlangt PNG/JPG/BMP/SVG-Import; eine Symbolbibliothek
  (Material Symbols o. ä.) liefert ihre Icons typischerweise als SVG, also ist SVG-Import
  auch Voraussetzung für die noch offene Symbolbibliothek.
- Entscheidung: `resvg` + `usvg` (Parsing/Baum) + `tiny-skia` (Software-Rasterizer) — der
  Standard-Rust-Stack für SVG→Bitmap, alle drei vom selben Projekt (linebender/resvg),
  MIT/Apache-2.0. `ll_render::picture::render_image()` erkennt `.svg` an der Dateiendung
  (case-insensitiv) und rastert über `render_svg_to_gray()` auf `printable_pins` Höhe
  (Seitenverhältnis erhalten), danach derselbe Floyd-Steinberg-Dithering-Pfad wie für
  PNG/JPEG/BMP. Rendering auf weißem, opakem Hintergrund — kein manuelles
  Alpha-Compositing nötig, da `resvg::render()` mit normaler Über-Blendung auf den bereits
  gefüllten Pixmap zeichnet.
- Konsequenzen: Standard-`usvg`/`resvg`-Features aktiv (u. a. `text`, `system-fonts`,
  `raster-images`) — SVGs mit eingebettetem Text oder Rasterbildern funktionieren, erhöht aber
  Abhängigkeitsfläche/Build-Zeit spürbar (viele Font-/Shaping-Crates). Noch nicht auf echtes
  Band gedruckt, nur PNG-Vorschau (ein Uhr-Symbol testweise gerendert, sah korrekt aus).
  Symbolbibliothek selbst (echte Icon-Dateien bündeln) ist jetzt technisch möglich, aber noch
  nicht umgesetzt — braucht eine explizite Entscheidung zu Lizenz/Icon-Set.

## ADR-013: `nusb` für den USB-Transport (M4)
- Datum / Status: 2026-10-01 · angenommen
- Kontext: M4 verlangt USB-Direktdruck auf Windows und Linux. `MASTER_PROMPT.md` nennt `nusb`
  (pure Rust) bzw. `rusb` (libusb-Bindings).
- Entscheidung: `nusb` 0.2 (MIT/Apache-2.0) mit `tokio`-Feature. Kein C-Toolchain-/libusb-Bedarf,
  `EndpointRead`/`EndpointWrite` implementieren `AsyncRead`/`AsyncWrite`, passt also direkt zum
  async `Transport`-Trait (ADR-005). `ll_transport::usb::UsbTransport` sucht in der aktiven
  Konfiguration das erste Interface der Druckerklasse (`0x07`) mit Bulk-OUT und Bulk-IN,
  übernimmt es (`detach_and_claim_interface`, löst unter Linux `usblp`) und liest den
  Statusblock mit `tokio::time::timeout`. VID/PID-Filter bleibt in `ll-core` über die
  Modelltabelle (`ll-protocol`), damit `ll-transport` keine Protokollwerte kennt.
- Konsequenzen: Unter Windows kann `nusb` nur WinUSB-gebundene Interfaces öffnen; mit
  `usbprint.sys` oder dem Brother-Treiber scheitert das Öffnen. TODO(verify): ob der PT-P710BT
  mit WinUSB (z. B. per Zadig) sauber druckt. Unter Linux braucht Nicht-root-Zugriff eine
  udev-Regel (`docs/PROTOCOL.md`). Noch nicht gegen echte Hardware getestet.

## ADR-014: Label-Layoutmodell, `.llabel`-Format und Tauri-App-Struktur (M6)
- Datum / Status: 2026-10-01 · angenommen
- Kontext: Der Editor braucht mehrere Elemente pro Label und ein Speicherformat; bisher konnte
  der Renderer nur genau einen Inhalt (Text/QR/Barcode/Bild) pro Label.
- Entscheidung:
  - `ll_core::label::Label`: Elemente werden **nacheinander entlang des Bandes** angeordnet
    (Abstand, Rand, Mindestlänge mit Zentrierung, Rahmen), jedes füllt die bedruckbare Höhe.
    Das deckt typische Bandlabels ab und ist einfach zu bedienen. Frei positionierbare Elemente
    (`MASTER_PROMPT.md` Abschnitt 3) bleiben späterer Ausbau; das Format ist dafür versioniert.
  - `.llabel` = JSON über `serde` (`version`, `elements` mit `"type"`-Tag, `gap_mm`,
    `padding_mm`, `min_length_mm`, `frame`). Neuere Versionen werden abgelehnt, unbekannte
    Felder ignoriert. Relative Bildpfade gelten relativ zur Vorlagendatei.
  - `render_label()` ist der eine Renderpfad für Vorschau (`render_label_png`), Druck
    (`print_label`) und CLI (`--template`); `print_text` usw. sind Ein-Element-Abkürzungen.
  - Verbindungswahl (`ll_core::device::Connection`) zentral in `ll-core` für CLI und GUI.
  - GUI unter `app/`: Tauri 2, Frontend **Vite + reines TypeScript ohne UI-Framework**
    (kleines Bundle, schneller Kaltstart, Ziel < 1 s), i18n über flache JSON-Wörterbücher
    (`app/src/i18n/{de,en}.json`, Deutsch Standard). `app/src-tauri` ist ein **eigener
    Cargo-Workspace** (WebKitGTK-Abhängigkeit unter Linux soll den Bibliotheks-CI-Job nicht
    belasten), eigener CI-Job `app`. Vorschau-PNG geht als rohe IPC-Antwort (`ArrayBuffer`)
    ohne Base64. Einzige Tauri-Plugin-Abhängigkeit: `tauri-plugin-dialog` (Öffnen/Speichern).
- Konsequenzen: Kein freies Positionieren, keine Textformatierung (fett/Größe/mehrzeilig) in
  dieser ersten Editor-Stufe. WinRT-Bluetooth blockiert beim Verbinden einen Worker-Thread
  (TODO `spawn_blocking`).
