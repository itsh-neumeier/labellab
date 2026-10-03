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

## ADR-015: Windows-Auslieferung als eigenständige `.exe` (portabel)
- **Nachtrag 2026-10-01:** Nutzerwunsch „reine portable ohne Installer“: NSIS/MSI entfernt
  (`bundle.active = false`), Artefakt `LabelLab-windows-x64-portable` mit `LabelLab.exe`,
  `labellab.exe` und `LIESMICH.txt`. Die statische C-Laufzeit für die CLI kommt jetzt aus
  `static_vcruntime` (Build-Abhängigkeit von `ll-cli`, MIT/Apache-2.0/Zlib) statt aus einer
  Workspace-`.cargo/config.toml` mit `+crt-static`: die galt auch für `app/src-tauri` und
  kollidierte dort beim Linken mit tauri-builds eigener CRT-Einstellung.
- Datum / Status: 2026-10-01 · angenommen
- Kontext: Nutzer will LabelLab unter Windows als fertige `.exe` ohne Entwicklungsumgebung.
- Entscheidung: GitHub-Actions-Workflow `windows-build.yml` (bei jedem Push, manuell, Tags `v*`
  zusätzlich als Release) baut auf `windows-latest`: `LabelLab.exe` (GUI, Frontend eingebettet),
  NSIS-Setup (`installMode: currentUser`, Deutsch/Englisch, keine Adminrechte), MSI (de-DE) und
  `labellab.exe` (CLI) als Artefakt `LabelLab-windows-x64`. C-Laufzeit statisch gelinkt
  (`.cargo/config.toml`, `+crt-static` für MSVC; Tauri macht das für die GUI selbst), damit kein
  Visual-C++-Redistributable nötig ist. WebView2: vorinstalliert auf Windows 11/aktuellem
  Windows 10, der Installer lädt den Bootstrapper bei Bedarf (`downloadBootstrapper`).
  `mainBinaryName = "LabelLab"`.
- Konsequenzen: Binärdateien unsigniert → SmartScreen-Warnung beim ersten Start (Signierung
  ist M8-Thema). Lokaler Cross-Build von Linux geht auch (`cargo-xwin`, `clang`/`lld`, `nsis`:
  `npx tauri build --runner cargo-xwin --target x86_64-pc-windows-msvc --bundles nsis`), gilt
  bei Tauri aber als experimentell — maßgeblich ist der Windows-Runner.

## ADR-016: Freies Layout mit Boxen, Schriftgrößen, mehrzeiliger Text (M6)
- Datum / Status: 2026-10-01 · angenommen (ergänzt ADR-014)
- Kontext: Nutzer-Feedback nach dem ersten Windows-Test: Elemente müssen frei als Boxen
  positionierbar und skalierbar sein, Schriftgrößen angebbar, Text mehrzeilig, Boxen sollen
  sich bündig aneinanderlegen lassen. Außerdem blieb die Vorschau unter Windows leer.
- Entscheidung:
  - `Item { element, rect: Option<Rect> }` (`rect` in mm: `x_mm` entlang, `y_mm` quer ab
    Oberkante des bedruckbaren Bereichs). Ohne `rect` gilt weiter das Fluss-Layout
    (CLI-Abkürzungen, v1-Vorlagen unverändert, Golden-Tests unberührt). `.llabel` Version 2,
    v1 wird gelesen.
  - `ll_render::boxed`: jedes Element wird in seine Box gerendert und per `Bitmap::blit` auf
    das Label gelegt, außerhalb des bedruckbaren Bereichs abgeschnitten. Text: `size_pt`
    (1 pt = 2,5 Druckpunkte bei 180 dpi, `pt_to_dots`) oder automatisch größtmöglich ohne
    Zusatzumbruch; `\n` = neue Zeile; feste Größe bricht an Wortgrenzen um; Ausrichtung
    links/Mitte/rechts, vertikal zentriert. Messung und Rendern nutzen dieselben
    fontdue-Layout-Einstellungen. QR/Bild seitenverhältnistreu eingepasst; Barcode-Modulbreite
    = größte ganze Zahl Druckpunkte, die in die Box passt (Lesbarkeit).
  - Labellänge mit Boxen: Ende der rechtesten Box + `padding_mm`, mindestens `min_length_mm`.
  - `resolved_rects()` liefert für Fluss-Elemente die Box, die sie im Fluss bekommen; der Editor
    wandelt damit alte Vorlagen ohne optische Änderung in Boxen um.
  - Editor: Boxen als Overlay über der echten 1-Bit-Vorschau, Ziehen/Skalieren (Kante rechts,
    unten, Ecke), magnetisches Einrasten (`app/src/snap.ts`) an Labelanfang, Bandkanten/-mitte
    und Kanten/Mitten anderer Boxen mit Hilfslinien (Alt = aus), Pfeiltasten, X/Y/B/H-Felder,
    Duplizieren, Entfernen-Taste. X wird auf ≥ 0 begrenzt.
  - Vorschau als Base64-String statt roher IPC-Bytes (`base64`-Crate, MIT/Apache-2.0): der
    Binärweg kam im Windows-Build nicht als Bild an.
- Konsequenzen: Kein Fett/Kursiv (nur die eine Systemschrift aus `fontsrc`), keine Rotation,
  keine Warnung, wenn Text mit fester Größe nicht in seine Box passt (wird abgeschnitten).

## ADR-017: Systemschriften, CSV-Serien, Vor-/Nachschnitt, BT-Gerätenamen
- Datum / Status: 2026-10-01 · angenommen
- Kontext: Nutzerwünsche nach dem ersten erfolgreichen GUI-Druck unter Windows: Schriftarten
  inkl. fett/kursiv aus den installierten Systemschriften; Drucker heißt in der Liste immer
  „SPP SERVER“; Checkboxen Vorschnitt/Nachschnitt; Serien mit Variablen aus CSV inkl.
  Bereichsauswahl; sichtbarer „Wird gedruckt“-Zustand.
- Entscheidung:
  - Schriften: `ll_render::fonts` mit `fontdb` (MIT, war über `resvg` schon im Baum):
    Systemschriften einmal pro Prozess scannen (`OnceLock`), Abfrage nach Familie/fett/kursiv.
    Fehlt die Familie → Standardschrift; fehlt der Schnitt → synthetisch (Glyphen um 1 Punkt
    je 24 px verbreitert bzw. ~12° geschert), damit fett/kursiv auf dem Band immer sichtbar ist.
    `Element::Text` bekommt `font`, `bold`, `italic` (nur geschrieben, wenn gesetzt).
  - CSV-Serien: `ll_core::series` (`csv`-Crate, MIT/Unlicense). Platzhalter `{{Spalte}}`
    (Groß/Klein egal) und `{{#}}` (Datensatznummer) in Text, QR-/Barcode-Daten und Bildpfad;
    unbekannte Platzhalter bleiben sichtbar stehen. Trennzeichen `;`/`,`/Tab aus der Kopfzeile
    erkannt, UTF-8 (mit/ohne BOM) oder Windows-1252 (Excel „ANSI“). Bereich 1-basiert,
    inklusive, auf die Daten begrenzt. Vorschau und Druck füllen denselben `Label` aus
    (gleicher Renderpfad). GUI hält die CSV im Backend (`SeriesState`), Vorschau zeigt einen
    wählbaren Datensatz; Druck aller oder Von–Bis, Fortschritt per Event `print-progress`.
    Jedes Label ist ein eigener Druckauftrag über dieselbe Verbindung (Kettendruck = M7-Rest).
  - Vorschnitt: zunächst als leere Ein-Zeilen-Seite mit Auto-Cut umgesetzt — auf Hardware
    3 Schnitte statt einem, daher **wieder entfernt** (ADR-018).
  - Bluetooth-Liste zeigt den Gerätenamen hinter dem SPP-Dienst (`RfcommDeviceService.Device()
    .Name()`, Fallback Dienstname, TODO(verify)); `model_for_device_name()` erkennt das Modell am
    Namenspräfix; erkannte Drucker stehen oben, werden nach „Suchen“ vorausgewählt, das Modell
    wird übernommen und der Bandstatus automatisch gelesen. `--bt --device` akzeptiert Namen.
- Konsequenzen: Erster Schriftscan kann unter Windows spürbar dauern (läuft im Hintergrund,
  Schriftliste erscheint verzögert). Vorschnitt und Mehrfach-Aufträge pro Verbindung sind
  hardware-unbestätigt.

## ADR-018: Mehrband-Labels, Kettendruck, Bandfarben-Vorschau, glatte Vorschau
- Datum / Status: 2026-10-01 · angenommen
- Kontext: Nutzerwünsche: Labels über 2×/3× Band (überlappend aufkleben), Serien fortlaufend
  ohne Schnitt, weniger pixelige Vorschau, Vorschau in allen gängigen Bandfarben. Hardware-
  Befund: Vorschnitt per Leerseite schneidet dreimal.
- Entscheidung:
  - `Label::strips` (Standard 1): Entwurf ist `strips` × bedruckbare Höhe hoch, auf einer
    virtuellen Zeichenfläche (`Canvas`, Kopfbreite = gestapelte Höhe, Offset 0) gerendert;
    `render_label_pages` schneidet ihn in Streifen und legt jeden an den Pin-Offset des echten
    Kopfes. Oberster Streifen zuerst. Rahmen umschließt das Gesamtlabel. Überlappung beim
    Aufkleben ≈ Bandbreite − bedruckbare Höhe (Hinweis in der GUI).
  - `print_labels`: ohne `chain` jeder Streifen/jede Kopie/jeder Datensatz als eigener Auftrag
    (bisheriger, hardware-erprobter Weg, Status je Auftrag); mit `chain` ein mehrseitiger
    Auftrag nach Raster Command Reference (`0C` zwischen Seiten, `1A` am Ende, `n9`), Auto-Cut
    nur in den Steuercodes der letzten Seite, da `ESC i A` (Schnitt nach n Labels) laut Referenz
    beim PT-P710BT nicht unterstützt ist. Fortschritt per Callback. Vorschnitt-Leerseite
    entfernt (`PrintOptions::pre_cut` weg, CLI `--pre-cut` → `--chain`).
  - Statusbyte 24/25 als Farbtabellen in `ll_protocol::media` (Quelle Raster Command Reference
    v1.02); GUI übernimmt die Bandfarbe nach „Status lesen“ automatisch.
  - Vorschau: `render_label_preview(.., scale)` rendert dasselbe Layout über denselben Code mit
    `scale`-facher Auflösung (Standard 4 = 720 dpi, „Glatt“) oder exakt (1, „Druckraster“) als
    transparente PNG-Maske; die GUI legt sie per CSS-Maske in Schriftfarbe auf die Bandfarbe
    (transparentes Band als Schachbrett). Skaliert werden mm-Umrechnung, Schriftgrößen,
    Rahmenstärke und die Fluss-Barcode-Modulbreite; Dithering von Bildern ist bei „Glatt“
    feiner als im Druck — maßgeblich bleibt „Druckraster“.
- Konsequenzen: Kettendruck und Mehrseiten-Aufträge sind hardware-unbestätigt; ob Auto-Cut nur
  auf der letzten Seite genau einen Schnitt am Ende ergibt, ist offen.

## ADR-019: Material Symbols (kuratierte Auswahl) als Symbolbibliothek (M5)
- Datum / Status: 2026-10-01 · angenommen (parallel auf `main` entstanden, beim Zusammenführen
  von ADR-013 auf ADR-019 umnummeriert, da ADR-013 bereits `nusb` belegt)
- **Offener Abgleich:** In der parallelen Session hatte der Nutzer **Tabler Icons (MIT)** für
  Elektro/IT plus **selbst gezeichnete Warnzeichen im Stil DIN EN ISO 7010** gewählt (offizielle
  ISO-Grafiken wegen Urheberrecht nicht übernehmen). Material Symbols bleibt vorerst; ob ergänzt
  oder ersetzt wird, mit dem Nutzer klären. Symbole sind seit dem Merge auch als Label-Element
  (`{"type": "symbol", "name": ...}`, `ll_render::boxed::symbol_in_box`) nutzbar.
- Kontext: Letztes offenes M5-Stück. Nutzer wollte sowohl eine mitgelieferte Bibliothek als
  auch eigene SVGs nutzen können — Letzteres war mit ADR-012 (`--image icon.svg`) bereits
  fertig. Für die Bibliothek selbst: keine geratenen URLs (Projektregel), echte Icon-Dateien
  nötig.
- Entscheidung: npm-Paket `@material-symbols/svg-400` (Variante „outlined“, Apache-2.0,
  deps: keine) als verifizierte, deterministische Quelle statt vermuteter GitHub-Rohpfade.
  10 Icons kuratiert und nach `crates/ll-render/assets/symbols/*.svg` kopiert (Attribution in
  `NOTICE.md` dort): `network`, `wifi`, `power`, `warning`, `arrow-up/-down/-left/-right`,
  `fire`, `fire-extinguisher`. **Kein elektrisches Erdungssymbol** — Material Symbols ist ein
  allgemeines UI-Set, kein Satz elektrotechnischer Schaltzeichen; dafür ggf. eigenes
  `--image erdung.svg` nutzen. Neues `ll_render::symbols`-Modul bettet die SVGs per
  `include_bytes!` ein (`SYMBOL_NAMES`, `symbol_svg()`, `render_symbol()` auf demselben
  `render_svg_bytes()`-Pfad wie ADR-012). `ll-core::print::print_symbol()`,
  CLI `--symbol <name>` auf `print`/`render`, neuer `labellab symbols`-Befehl listet die Namen.
- Konsequenzen: Icons fest im Binary (kein Laufzeit-Download, keine Netzabhängigkeit zur
  Laufzeit). Erweiterung um weitere Icons = weitere Dateien kopieren + `symbols.rs`-Makro-Eintrag,
  kein neuer Build-Abhängigkeits-Code. Noch nicht auf echtes Band gedruckt, nur PNG-Vorschau
  (4 Symbole stichprobenartig geprüft, sahen korrekt aus). Apache-2.0-Lizenznotiz muss bei
  Weitergabe erhalten bleiben (`NOTICE.md`). **M5 ist damit vollständig** (Schriften, Rahmen,
  QR, 6 Barcode-Symbologien, Bilder inkl. SVG, Symbolbibliothek, `render` → PNG) — bis auf
  ausstehende Hardware-Tests für mehrere Teile.

## ADR-020: M7-Vorlagen (Kabelfahne, Kabelwickel, Patchpanel), Nummernfolgen, Drehung, Fläche
- Datum / Status: 2026-10-01 · angenommen
- Kontext: Offene M7-Punkte aus `MASTER_PROMPT.md`: Kabelfahne, Kabelwickel, Patchpanel/Port-
  Labels, Serien mit `{n}`/Start/Schritt/Stellen und Buchstabenfolgen. Der Kabelwickel braucht
  Text quer zum Band, Patchpanels brauchen Trennstriche.
- Entscheidung:
  - **Generatoren statt eigener Labeltypen:** `ll_core::layouts` (`CableFlag`, `CableWrap`,
    `PatchPanel`, `Layout` mit `"kind"`-Tag) erzeugen ein normales `Label` mit Boxen und fester
    Länge (`min_length_mm`, `padding_mm = 0`). Das Ergebnis bleibt im Editor bearbeitbar, als
    `.llabel` speicherbar und mit CSV/Nummerierung kombinierbar. Kabelfahne: Text,
    Wickelbereich π × Durchmesser, Text. Kabelwickel: Umfang π × Durchmesser, Text n-mal
    (Standard ≈ alle 15 mm), optional 90° gedreht. Patchpanel: `count` Felder à `pitch_mm`,
    Nummer = `start + i·step` mit Präfix und Nullauffüllung, optional 0,3-mm-Trennstriche.
  - **Drehung:** `Item::rotation` (0/90/180/270°, im Uhrzeigersinn), nur für Boxen; der
    Inhalt wird in die getauschte Box gerendert und per `Bitmap::rotated` gedreht.
  - **Fläche:** `Element::Fill` = gefüllte Box (Trennstriche, Balken, Blöcke).
  - **Nummernfolgen:** `series::Numbering { start, step }`; Platzhalter `{{n}}`, `{{n:03}}`
    (Nullen), `{{a}}`/`{{A}}` (a…z, aa…), zusätzlich zu `{{Spalte}}`/`{{#}}`; CSV-Spalten
    haben Vorrang vor gleichnamigen Eingebauten. Ohne CSV: `count` Labels
    (CLI `--count/--start/--step`, GUI-Bereich „Nummerierung“). Ein Einzeldruck füllt
    `{{n}}` mit dem Startwert (Vorschau = Druck).
  - CLI `labellab generate cable-flag|cable-wrap|patch-panel … -o datei.llabel`; GUI
    „Assistent …“ (Backend-Befehl `generate_layout`), außerdem Elemente „Symbol“
    (Bibliothek aus ADR-019) und „Linie/Fläche“ sowie ⟳-Knopf je Element.
- Konsequenzen: Text in Generator-Feldern ist auto-skaliert (an Feldbreite/-höhe); sehr lange
  Texte werden klein. Echte Längen-Genauigkeit hängt an der Vorschub-Genauigkeit des Druckers
  (hardware-offen, wichtig für Patchpanels).

## ADR-021: Bluetooth unter Linux über BlueZ (`bluer`), Kopplung aus CLI und GUI (M4)
- Datum / Status: 2026-10-01 · angenommen
- Kontext: M4 verlangt nativen Bluetooth-Druck auch unter Linux sowie Kopplung aus der App,
  damit Nutzer nicht in die Systemeinstellungen müssen.
- Entscheidung:
  - **Linux:** `bluer` (offizielle BlueZ-Rust-Bindung, BSD-2-Clause, MIT-kompatibel, D-Bus) mit
    Features `bluetoothd` + `rfcomm`. Geräte werden über die MAC-Adresse angesprochen. `bluer`
    hat keinen SDP-Client, daher kommt der RFCOMM-Kanal aus `ll_protocol::model::
    BT_SPP_RFCOMM_CHANNEL` (= 1, `TODO(verify)`). `tokio-stream` für den Discovery-Stream.
  - **Kopplung:** `pair(id, pin)` in `ll-transport` — Linux registriert einen BlueZ-Agenten,
    der PIN-Anfragen mit `BT_DEFAULT_PIN` (`0000`, `TODO(verify)`) beantwortet und
    Bestätigungen annimmt, und setzt das Gerät auf „trusted“. Windows nutzt WinRT
    `DeviceInformationCustomPairing` mit denselben Antworten.
  - `ll-core::device` bietet eine plattformübergreifende async-API (`list_bluetooth_devices`,
    `discover_bluetooth_devices`, `pair_bluetooth`); CLI `labellab pair [Gerät]`, GUI-Dialog
    „Koppeln …“ (Suche, Liste ungekoppelter Geräte, Koppeln).
- Konsequenzen: Linux-Builds brauchen `libdbus-1-dev`/`pkg-config` (CI, AGENTS.md). Zur
  Laufzeit muss `bluetoothd` laufen. Kanal und PIN sind bis zum Hardware-Test Annahmen.

## ADR-022: Rahmen mit Stil, Stärke und frei wählbaren Seiten; Schriftauswahl in der eigenen Schrift
- Datum / Status: 2026-10-01 · angenommen
- Kontext: Nutzerwunsch: Ränder in verschiedenen Varianten (u. a. gestreift), definierbare
  Linienstärke, frei wählbare Seiten (z. B. nur oben oder oben und unten). Außerdem sollen
  Schriften in der Auswahlliste in ihrer eigenen Schrift erscheinen.
- Entscheidung:
  - `ll_render::frame`: `BorderStyle` (`solid`, `dashed`, `dotted`, `double`, `striped`),
    `BorderSides` (`top`/`bottom`/`left`/`right`, wie in der Vorschau: oben = Bandkante am
    Pin-Offset), `Border` in Druckpunkten (Stärke, Muster, Abstand) und `draw_border_styled`.
    Streifen laufen diagonal über absolute Koordinaten, damit sie an Ecken durchgehen.
  - `.llabel`: neues optionales Feld `border` (`LabelBorder`, mm: `style`, `width_mm`,
    `sides`, `pattern_mm`, `inset_mm`). Das alte `frame: true` bleibt lesbar und entspricht dem
    Standardrahmen (durchgezogen, 2 Punkte, alle Seiten); `border` hat Vorrang. Keine
    Versionserhöhung nötig (Feld optional, alte Dateien unverändert gültig).
  - Inhalt hält Abstand zum Rahmen: Fluss-Elemente werden um Abstand + Linie + Freiraum
    (eine Linienstärke, mind. 0,3 mm) je Rahmenseite verkleinert, das Labelende wächst um
    denselben Betrag. Boxen bleiben frei positioniert; die GUI rückt sie beim Einstellen
    des Rahmens oben/unten/links nach innen (rückgängig machbar).
  - CLI: `--border <stil>`, `--border-sides ou|olur…`, `--border-width`, `--border-pattern`,
    `--border-inset` für `print` und `render`; `--frame` bleibt.
  - GUI: Bereich „Rahmen“ (Stil, Linienstärke, Strich-/Streifenlänge, Abstand, Seiten).
    Schriftauswahl als eigene Komponente (Suchfeld, Pfeiltasten/Enter/Esc), jede Familie in
    ihrer Schrift (`font-family`), weil native `<select>`-Optionen sich in WebView2/WebKitGTK
    nicht zuverlässig gestalten lassen.
- Konsequenzen: Feine Muster (< 1 mm) sind auf 180 dpi grob; wie gut Streifen/Punkte auf dem
  Band aussehen, ist hardware-offen. Systemschriften, die die Webview nicht kennt, erscheinen
  in der Liste in der Ersatzschrift (gedruckt wird trotzdem die gewählte).

## ADR-023: Icon-Sets (`.llabel-iconset`) mit Kategorien, Import und ISO-7010-Set
- Datum / Status: 2026-10-02 · angenommen
- Kontext: Nutzerwunsch: Symbole nach Kategorien ordnen, eigene Icon-Sets als **eine Datei**
  mit eigener Endung importieren, und die Sicherheitszeichen nach ISO 7010 (Liste im
  Wikipedia-Artikel) als eigenes Set mitliefern. Weitere Sets sollen folgen können.
- Entscheidung:
  - **Dateiformat `.llabel-iconset`** (JSON, `format: "llabel-iconset"`, `version: 1`): `id`,
    `name`/`description` (Text oder `{de, en}`), `license`, `source`, `halftone`
    (`threshold`|`dither`), `categories[]` (`id`, `name`), `icons[]` (`id`, `name`,
    `category`, `tags`, `svg` als SVG-Text, optional `license`/`author`/`source` je Icon).
    Eine Datei, menschenlesbar, ohne Zip; SVGs werden beim Erstellen mit usvg bereinigt.
  - **Registry in `ll-render::iconset`** (prozessweit, `RwLock`): mitgelieferte Sets
    (`material` = bisherige Symbolbibliothek, jetzt mit Kategorien; `iso7010`) plus importierte.
    Symbolnamen `set:icon` (z. B. `iso7010:W012`); ein Name ohne Set wird in allen Sets gesucht
    (mitgelieferte zuerst), daher bleiben alte `.llabel`-Dateien (`warning`) gültig.
    Mitgelieferte IDs sind reserviert.
  - **Graustufen → Punkte:** Icon-Sets nutzen standardmäßig einen harten Schwellwert (Luma
    < 150 = Tinte): Signalgelb wird weiß, Signalrot/-blau/-grün und Schwarz werden Tinte. So
    bleiben Zeichen auf 180 dpi sauber statt gerastert (Bilder bleiben bei Floyd-Steinberg).
  - **Import/Verwaltung in `ll-core::iconsets`:** Import prüft die Datei vollständig und kopiert
    sie nach `<Datenordner>/iconsets/<id>.llabel-iconset` (Windows `%APPDATA%\LabelLab`,
    Linux `~/.local/share/labellab`, überschreibbar mit `LABELLAB_DATA_DIR`); CLI und GUI laden
    beim Start alle Dateien dort. `from_dir` baut ein Set aus einem SVG-Ordner (Unterordner =
    Kategorien).
  - CLI: `labellab symbols [--set] [--search]`, `labellab iconset list|import|remove|create`.
    GUI: Symbolauswahl-Dialog (Sets/Kategorien links, Suche, Vorschau-Kacheln, „Icon-Set
    importieren …“, „Set entfernen“).
  - **ISO-7010-Set:** Grafiken von Wikimedia Commons, nur Dateien mit Commons-Lizenz
    „Public domain“ oder „CC0“ (335 Zeichen; M002 ist CC BY-SA und fehlt daher). Herkunft, Urheber
    und Lizenz je Zeichen im Set; Bauweg reproduzierbar (`tools/iconsets/iso7010/`). Weil
    Wikimedia die geteilte IP drosselte, kamen die Dateien über einen per SHA-1 gegen Commons
    geprüften Mirror (npm `@iso-safety-signs/assets`) – nur byte-identische Dateien.
  - **IEC-60417-Set:** alle SVGs der Commons-Kategorie „IEC 60417 symbols“ (alle Seiten über
    die API) plus nicht einsortierte Dateien „IEC 60417 - Ref-No …“ (z. B. 5007 „Ein“), gleiche
    Lizenzregel (754 Symbole; 5107A ist CC BY-SA und fehlt). Englische Namen aus den
    Commons-Beschreibungen (deutsche Namen gibt es dort nicht). Commons ordnet nicht thematisch; die
    Kategorien (Sicherheit, Medizin, Ein/Aus, Elektrik, Audio/Video, Daten, Temperatur, Licht,
    Bedienung, Sonstige) vergibt das Bauskript nach Stichworten der Beschreibung.
  - Die ISO/IEC-Datenbank „Online Browsing Platform“ (OBP) ist **keine** Quelle: deren Inhalte
    sind geschützt und nicht frei lizenziert.
  - Beide Sets baut der Workflow „Icon sets“ auf GitHub (dort keine Drosselung der IP); das
    Ergebnis wird nach Durchsicht nach `crates/ll-render/assets/iconsets/` übernommen.
- Konsequenzen: `.llabel`-Dateien mit Symbolen aus importierten Sets brauchen dieses Set auf dem
  Zielrechner (klare Fehlermeldung, sonst kein Druck). Das ISO-Set vergrößert das Programm um
  das JSON (~1 MB). Rechtlich: ISO beansprucht Rechte an der Norm; die Commons-Dateien sind dort
  als gemeinfrei (u. a. „zu einfach für Schutz“) bzw. CC0 eingestuft — diese Einstufung wird
  übernommen und je Zeichen dokumentiert.

## ADR-024: Editor-Ausbau Paket 1 – Formen, feste Länge, Datum/Zeit, Bildkorrektur, Ausrichten/Sperren
- Datum / Status: 2026-10-02 · angenommen
- Kontext: Funktionsvergleich mit dem Hersteller-Editor (Screenshots des Nutzers, nur als
  Anregung; keine Grafiken oder Dateien daraus im Repo). Roadmap M9 in `PROGRESS.md`.
- Entscheidung:
  - **Element `shape`** (`ll_render::shape`, tiny-skia): `line`, `rectangle`,
    `rounded_rectangle` (Radius 25 % der kürzeren Seite), `ellipse`; `stroke_mm` (Standard
    0,3 mm), `filled`. Kanten per Schwellwert (Alpha ≥ 50 %), im Fluss ein Quadrat der Bandhöhe.
  - **`Label::fixed_length`:** mit `min_length_mm` ist das Label genau so lang, Inhalt darüber
    hinaus wird abgeschnitten (`Bitmap::truncate`); Rahmen folgt der festen Länge.
  - **Platzhalter `{{datum}}`/`{{date}}`, `{{zeit}}`/`{{time}}`** mit optionalem
    strftime-Format (`{{datum:%Y-%m-%d}}`), lokale Zeit über `chrono` (MIT/Apache, war im
    App-Baum schon vorhanden). Ungültiges Format → Platzhalter bleibt sichtbar. Die CLI füllt
    Platzhalter jetzt auch beim Einzeldruck (Vorschau = Druck).
  - **Bild:** `brightness`/`contrast` (−100…100) vor dem Dithering (`ImageAdjust`).
  - **`Item::locked`:** nur Editor-Verhalten (kein Verschieben/Skalieren, keine Griffe).
  - GUI: Ausrichten am Label/Band (Start/Mitte/Ende, oben/Mitte/unten, ganze Bandhöhe; Länge
    ohne das Element selbst gemessen), Umschalt beim Ecken-Skalieren hält das
    Seitenverhältnis, mm-Lineal über der Vorschau, Chips `{{datum}}`/`{{zeit}}`.
- Konsequenzen: `.llabel` bleibt Version 2 (alle neuen Felder optional). Z-Reihenfolge ist bei
  reinem Schwarz-Druck (Tinte wird nur hinzugefügt) wirkungslos und entfällt vorerst.

## ADR-025: Mehrere Arbeitsblätter pro `.llabel`-Datei
- Datum / Status: 2026-10-02 · angenommen
- Kontext: Nutzerwunsch: eine Datei soll aus mehreren Arbeitsblättern bestehen (wie Reiter im
  Hersteller-Editor), dazu Warnung vor dem Verlust ungespeicherter Änderungen.
- Entscheidung: `ll_core::document::Document { version: 3, sheets: [Sheet { name, width_mm?,
  label }] }`. Eine Datei mit **einem** Blatt wird weiterhin als normales Label (Version 2)
  geschrieben, damit sie überall lesbar bleibt; erst ab zwei Blättern entsteht Version 3.
  Ältere Programmstände lehnen Version 3 mit „neueres Format“ ab statt ein leeres Label zu
  lesen. Endung bleibt `.llabel`. CLI wählt ein Blatt mit `--sheet` (Nummer oder Name).
  GUI-Undo gilt je Blatt (Wechsel setzt ihn zurück).
- Konsequenzen: Die App lädt/speichert nur noch Dokumente (`load_document`/`save_document`).
  Druck/Serien/CSV wirken auf das aktuelle Blatt.


## ADR-026: Nicht-destruktiver Bild-Editor
- Datum / Status: 2026-10-02 · angenommen
- Kontext: Nutzerwunsch: Bilder im Programm zuschneiden und Hintergründe entfernen
  (transparent setzen), ohne externes Grafikprogramm.
- Entscheidung: Bearbeitungen werden als `ImageEdit` am Bild-Element gespeichert
  (`edit` in `.llabel`, fehlt = unverändert), die Bilddatei wird nie verändert. Zuschnitt in
  Bruchteilen des gedrehten Bildes (unabhängig von der Auflösung), Hintergrund =
  Farbabstand zu Rand- oder gewählter Farbe (Toleranz 0–100), optional nur vom Rand aus
  zusammenhängend (Flutfüllung), entfernte Pixel werden weiß (= nicht gedruckt).
  Druckumsetzung wählbar: Raster (Floyd-Steinberg, Standard) oder Schwelle (1–254).
  Vorschau und Druck nutzen denselben Pfad in `ll-render`; der Dialog zeigt zusätzlich
  eine verkleinerte Fassung (≤ 640 px) zum Bearbeiten.
- Konsequenzen: App-Backend hängt direkt an `image` (bereits Abhängigkeit von `ll-render`,
  MIT/Apache-2.0) für die PNG-Kodierung der Editor-Vorschau. Ältere Programmstände
  ignorieren das unbekannte Feld `edit`: Labels mit Bearbeitung öffnen dort, aber
  unbearbeitet.

## ADR-027: Einfügen aus der Zwischenablage
- Datum / Status: 2026-10-02 · angenommen
- Kontext: Nutzerwunsch: Grafiken und Texte mit Strg+V einfügen; Oberflächentexte sollen
  nicht markierbar sein (Verhalten wie eine Desktop-App).
- Entscheidung: Bild-Elemente verweisen weiter auf Dateien; ein eingefügtes Bild wird als
  Datei im Datenordner (`pasted/`, Name = FNV-1a-Hash des Inhalts) gespeichert statt in die
  `.llabel`-Datei eingebettet. Bilddaten kommen bevorzugt aus dem Paste-Ereignis (WebView2),
  sonst aus der System-Zwischenablage über `tauri-plugin-clipboard-manager` (offizielles
  Tauri-Plugin, MIT/Apache-2.0), weil WebKitGTK eingefügte Bilder nicht an die Seite gibt.
  Elemente werden als JSON (`application/x-labellab+json`, Text-Rückfall mit Marker)
  kopiert. Ein verstecktes contenteditable-Element nimmt Strg+C/X/V kurz den Fokus ab,
  damit auch WebKitGTK Clipboard-Ereignisse auslöst.
- Konsequenzen: Ein Label mit eingefügtem Bild ist nur zusammen mit dem Datenordner
  vollständig (wie bisher bei Bildern aus anderen Ordnern). Einbetten von Bildern in die
  `.llabel`-Datei bleibt eine mögliche spätere Erweiterung.

## ADR-028: Bandrand in der Vorschau, Zeilenabstand, Vorschau-Caches
- Datum / Status: 2026-10-02 · angenommen
- Kontext: Nutzerwunsch: Vorschau in echter Bandbreite mit sichtbarem nicht bedruckbarem
  Rand, einstellbarer Zeilenabstand, Vorschau bei viel Inhalt zu langsam (Fenster hing).
- Entscheidung: Der Rand wird nur im Frontend gezeichnet (Rahmen um die Bühne), Werte aus
  der Modelltabelle (`width_mm`, `printable_mm`); die Bühne bleibt der bedruckbare
  Bereich, damit Koordinaten, Druck und gespeicherte Labels unverändert bleiben.
  Zeilenabstand als Faktor `line_spacing` am Text-Element (0,5–3), direkt als
  fontdue-`line_height`. Performance: prozessweite Caches in den Bibliotheken
  (gerenderte Bildboxen, geparste Schriften) statt Neuladen je Render; Cache-Schlüssel
  enthält Dateigröße und Änderungszeit, damit geänderte Dateien neu gelesen werden.
  `render_preview` läuft asynchron auf einem Blocking-Thread.
- Konsequenzen: Speicherbedarf für bis zu 64 Bildboxen (Bitmaps, je Box wenige hundert KB
  bei hoher Vorschauauflösung). Der Rand entlang der Länge (Vorlauf vor dem Schnitt) wird
  nicht gezeigt, da unverifiziert.

## ADR-029: Fett/kursiv für Textteile als Auszeichnung im Text
- Datum / Status: 2026-10-02 · angenommen
- Kontext: Nutzerwunsch: in mehrzeiligem Text einzelne Bereiche fett oder kursiv.
- Entscheidung: Auszeichnung direkt im Textstring, `**fett**` und `__kursiv__`
  (Markdown-ähnlich, doppelte Zeichen, selten in Labeltexten). Kein neues Datenfeld, das
  `.llabel`-Format bleibt gleich, CSV-Platzhalter und CLI funktionieren unverändert.
  Ein Marker ohne Partner bleibt wörtlicher Text. Der Stil des Elements (F/K ohne
  Markierung) gilt als Grundstil, Marker schalten zusätzlich ein. Gerendert mit vier
  Schnitten derselben Familie in einem fontdue-Layout.
- Konsequenzen: Ältere Programmstände drucken die Marker als Zeichen. Die Marker sind im
  Textfeld sichtbar (kein WYSIWYG-Editor); dafür gibt es die Knöpfe/Tastenkürzel.

## ADR-030: Rahmen innerhalb der Label-Ränder, Inhaltsausrichtung in Boxen
- Datum / Status: 2026-10-02 · angenommen
- Kontext: Nutzerwunsch: der Rahmen soll die Label-Ränder berücksichtigen und je Seite einen
  Abstand haben; Inhalte in Boxen horizontal und vertikal ausrichtbar.
- Entscheidung: Rahmen-Abstand je Seite (`insets_mm`, Rückfall auf `inset_mm`); links/rechts
  wird er zu `padding_start_mm`/`padding_mm` addiert, oben/unten gilt er ab dem
  bedruckbaren Bereich. Ausrichtung als `halign`/`valign` am `Item` (nicht am Element, damit
  es keine Kollision mit Text-Feldern im flachen JSON gibt); Text nutzt weiter `align` für
  die Waagerechte und `valign` im Layout, andere Elemente werden nach dem Rendern anhand
  ihrer Tinte verschoben (Renderer bleiben unverändert zentriert).
- Konsequenzen: Ältere Labels mit Rahmen und Rand > 0 zeichnen den Rahmen jetzt um den Rand
  nach innen versetzt. Bilder mit weißem Hintergrund richten sich nach ihrer Tinte, nicht
  nach dem Bildrand.

## ADR-031: Hochformat als Ansicht, Druck immer im Querformat
- Datum / Status: 2026-10-02 · angenommen
- Kontext: Nutzerwunsch: Label mit senkrechtem Band bearbeiten (wie Hochformat im
  Hersteller-Editor).
- Entscheidung: `Label::orientation = portrait` speichert Boxen in Hochformat-Koordinaten
  (x quer zum Band, y entlang). Vor dem Rendern wird daraus das Querformat-Label
  (`to_landscape`: Box gedreht, Inhalt +270°); der Druckpfad bleibt unverändert. Die
  Vorschau dreht nur das fertige Bild für die Anzeige. Beim Umschalten dreht der Editor
  Boxen und Inhalte mit, damit der Ausdruck gleich bleibt.
- Konsequenzen: Rahmen-Seiten (oben/unten/links/rechts) beziehen sich weiter auf das Band
  im Querformat. Fluss-Elemente ohne Box werden beim Umschalten in Boxen umgewandelt.
- Nachtrag (Elementnamen): Der eigene Name eines Elements heißt im JSON `title`, nicht
  `name`, weil `Item` ins Element geflacht wird und `name` die Symbol-ID ist (Bugfix
  2026-10-02).

## ADR-032: Deko-Rahmen aus drei SVG-Segmenten
- Datum / Status: 2026-10-02 · angenommen
- Kontext: Nutzerwunsch: verzierte Rahmen wie im Hersteller-Editor, mit Editor; Aufbau aus
  Anfangs-, Wiederhol- und Endsegment.
- Entscheidung: Ein Rahmen = drei SVGs in voller druckbarer Höhe. Das Mittelstück wird so oft
  wiederholt, dass eine ganze Zahl Stücke die Länge füllt (leicht gestreckt), Anfang/Ende in
  Originalbreite. Sets als JSON `.llabel-frames` (wie Icon-Sets); mitgeliefertes Set selbst
  gezeichnet (MIT), keine Hersteller-Grafiken. Eigene Rahmen im Set „eigene“ im Datenordner.
  Zusätzlich zum bisherigen Linienrahmen (`border`), nicht als Ersatz.
- Konsequenzen: Boxen werden nicht automatisch vor dem Mittelstück geschützt (nur Anfang beim
  Wählen); Rahmen-SVGs werden beim Speichern normalisiert (Text zu Pfaden).

## ADR-033: Vorlage bleibt am Label (`Label::source`), Code-Assistent im Frontend
- Datum / Status: 2026-10-02 · angenommen
- Kontext: Nutzerwunsch: Sicherungskasten-Felder verbinden und „nicht nur einmal generierbar,
  sondern dynamisch anpassbar“; QR/Barcode in einem Assistenten.
- Entscheidung: `layouts::generate` speichert die erzeugende `Layout`-Beschreibung in
  `Label::source` (in `.llabel` mitgespeichert, optional). „Vorlage bearbeiten …“ öffnet den
  Assistenten damit und erzeugt das Blatt neu; Handänderungen an den Elementen gehen dabei
  verloren (bewusst: die Vorlage ist die Quelle). Sicherungskasten: `spans` (Teileinheiten je
  Feld) und `texts` (eigener Text je Feld, leer = Nummer); leer = wie bisher ein Feld je
  Teileinheit. Der Code-Assistent baut nur den Inhalt (`WIFI:`, vCard 3.0, `mailto:`, `tel:`)
  im Frontend (`app/src/codes.ts`) und erkennt ihn beim Bearbeiten wieder; das Datenformat
  der Elemente `qr`/`barcode` bleibt unverändert.
- Konsequenzen: Alte Dateien ohne `source` bleiben gültig (kein „Vorlage bearbeiten“).
- Nachtrag (ADR-034): Der Sicherungskasten ist inzwischen ein eigenes Element.
  UPC-A wird jetzt als EAN-13 mit führender 0 kodiert (Fehlerkorrektur).

## ADR-034: Sicherungskasten als eigenes Element
- Datum / Status: 2026-10-02 · angenommen
- Kontext: Nutzer zeigte den Sicherungskasten des Hersteller-Editors: ein Objekt, dessen
  Felder (Anzahl, Raster, Verhältnis je Feld), Trennzeichen und Text nachträglich in den
  Eigenschaften geändert werden. Unsere Vorlage erzeugte viele Einzel-Elemente.
- Entscheidung: `Element::FuseBox { fields: [{text, ratio, vertical?}], pitch_mm,
  separator (marks|dashed|line|bold|frame|none), vertical, reverse, size_pt, align, font,
  bold, italic }` (`ll_core::fusebox`). Felder werden nach `ratio` über die Box verteilt,
  `pitch_mm` × Summe der Faktoren ist die natürliche Länge (Fluss-Layout; der Editor setzt
  die Boxlänge danach). Eine gemeinsame Schriftgröße für alle Felder (`None` = größte, die in
  jedes Feld passt). Senkrechter Text liest von unten nach oben. Der Generator `fuse_box`
  liefert genau dieses eine Element (Hauptschalter = Feld mit Faktor Breite/Raster, waagerecht).
- Konsequenzen: Hochformat-Ansicht: Felder laufen entlang der Box-Breite (wie gedreht). Neue
  Trennstil-Maße (0,25 mm Linie, 0,7 mm fett, 1/0,8 mm Striche) sind Konstanten in
  `fusebox.rs`, keine Protokollwerte.

## ADR-035: Rahmen-Zeichnungen als PNG im SVG-Segment
- Datum / Status: 2026-10-02 · angenommen
- Kontext: Nutzerwunsch: Rahmenteile selbst zeichnen wie in Paint, Bilder einfügen und
  nachbearbeiten, statt nur SVG-Code.
- Entscheidung: Der Editor (`app/src/framePaint.ts`) arbeitet mit 1 Bit je Pixel in der
  bedruckbaren Höhe des gewählten Bandes (1 Pixel = 1 Druckpunkt). Gespeichert wird jedes
  gezeichnete Teil als SVG mit genau einem `<image href="data:image/png;base64,…">`; das
  Dateiformat `.llabel-frames` und der Renderer bleiben unverändert (resvg zeichnet
  eingebettete PNGs, Test in `decor.rs`). Beim Öffnen werden solche Teile wieder als
  Zeichnung geladen, andere SVGs als Code; „Zeichnen“ rastert SVG-Code auf Wunsch.
  Bilder laufen über den vorhandenen Bild-Editor (Zuschnitt, Hintergrund) und werden mit
  Schwelle oder Floyd-Steinberg-Raster in Pixel umgesetzt.
- Konsequenzen: Auf anderen Bandbreiten wird die Zeichnung skaliert (nicht mehr
  pixelgenau). Keine neue Abhängigkeit.

## ADR-036: Portable Ablage, verschlüsselte Einstellungsdatei
- Datum / Status: 2026-10-02 · angenommen
- Kontext: Nutzerwunsch: Einstellungen „direkt bei der exe und verschlüsselt, damit man
  nicht einfach so herumspielen kann“; eigene Importe in einem Unterordner `llappdata` bei
  der exe. Bisher: WebView-`localStorage` (an das WebView-Profil gebunden) und
  `%APPDATA%\LabelLab`.
- Entscheidung: `ll_core::paths`: Ist der Ordner der exe beschreibbar, liegen dort
  `LabelLab.settings` und `llappdata/`; sonst der Benutzerordner. Einmalige Übernahme alter
  Daten aus dem Benutzerordner. `ll_core::settings`: Schlüssel/Wert-JSON, verschlüsselt mit
  ChaCha20-Poly1305 (neue Abhängigkeit `chacha20poly1305`, RustCrypto, Apache-2.0/MIT),
  Format `LLS1` + Nonce + Chiffrat, atomar geschrieben. Der Schlüssel steckt im Programm: Das
  schützt vor Lesen/Ändern mit dem Editor und erkennt jede Änderung (dann Standardwerte),
  ist aber kein Geheimnis gegen jemanden, der das Programm zerlegt. Die Oberfläche nutzt
  `app/src/settings.ts` statt `localStorage` (alte Werte werden einmal übernommen).
- Konsequenzen: Mehrere Benutzer derselben portablen Kopie teilen sich die Einstellungen.
  Umgebungsvariablen `LABELLAB_DATA_DIR`/`LABELLAB_SETTINGS` überschreiben die Pfade.

## ADR-037: A4-Druck über den Druckdialog der WebView
- Datum / Status: 2026-10-02 · angenommen
- Kontext: Nutzerwunsch: ausgewählte Blätter auf A4 mit normalem Drucker drucken, mit Kopf,
  Logo, Graustufen, Farbwahl und Testseite mit Korrekturwert.
- Entscheidung: `app/src/a4print.ts` rendert jedes Blatt über `render_preview` (gleicher
  Renderpfad, 720 dpi), färbt es auf einem Canvas (Band-/Schriftfarbe), verteilt die Labels
  regalweise auf A4-Seiten (zu lange Labels hochkant) und druckt per `window.print()` mit
  Druck-CSS (`@page A4, margin 0`). Korrekturfaktoren X/Y (%) skalieren den Seiteninhalt; die
  Testseite hat 100-mm-Lineale und 10/50-mm-Quadrate. Keine neue Abhängigkeit (kein PDF).
- Konsequenzen: Die Maßhaltigkeit hängt vom Druckdialog ab: WebKitGTK (Linux) skaliert auf
  ca. 94 % (eigene Seitenränder, gemessen per „In Datei drucken“), WebView2 (Windows) sollte
  100 % liefern – unverifiziert, daher Testseite und Korrekturwert. Seitenhöhe im Druck 287 mm,
  damit kein leeres Zusatzblatt entsteht. Serien/CSV werden auf A4 noch nicht ausgegeben.

## ADR-038: `.lbx`-Import mit `zip` und `quick-xml`
- Datum / Status: 2026-10-02 · angenommen
- Kontext: Nutzer wollen vorhandene Labels aus dem Hersteller-Editor weiterverwenden. `.lbx` ist
  ein ZIP-Archiv mit `label.xml` und eingebetteten Bildern (Analyse: `docs/IMPORT-LBX.md`).
- Entscheidung: `ll_core::lbx` liest das Archiv mit `zip` (MIT, Feature `deflate`; das kleinere
  `deflate-flate2` allein kompiliert in zip 2.4 nicht, weil es kein flate2-Backend aktiviert –
  `deflate` zieht zusätzlich `zopfli`, Apache-2.0) und das XML mit `quick-xml` (MIT) in einen
  kleinen eigenen Knotenbaum. `image` ist in `ll-core` jetzt normale Abhängigkeit (BMP → PNG
  der eingebetteten Bilder). Ergebnis: `Document` plus Hinweisliste (`LbxWarning{kind, detail}`,
  `kind` als stabiler i18n-Schlüssel `lbx.warn.<kind>`). Texte mit „Verkleinern“ (`shrink`)
  werden nach dem Import mit dem echten Renderer schrittweise verkleinert, bis sie passen.
- Konsequenzen: Kein Export nach `.lbx` (nur Import). Nicht Übertragbares (Zierrahmen,
  Cliparts aus Hersteller-Schriften, Aztec, senkrechter Text, Bildzuschnitt) wird angenähert und
  gemeldet. Das Format ist aus Beispieldateien abgeleitet, nicht aus einer Spezifikation.

## ADR-039: Weitere UI-Sprachen, Flaggen als SVG
- Datum / Status: 2026-10-02 · angenommen
- Kontext: Nutzerwunsch: die wichtigsten Sprachen, Auswahl mit Flaggen.
- Entscheidung: Acht Sprachen (de, en, fr, es, it, nl, pl, cs) als JSON-Wörterbücher, alle
  statisch eingebunden (klein, keine Ladezeit). Fehlende Texte: Sprache → Englisch → Deutsch.
  Auswahl als eigene Aufklappliste (`app/src/langs.ts`), weil ein `<select>` keine Bilder
  zeigt; Flaggen als kleine Inline-SVGs, da Windows keine Flaggen-Emojis darstellt (Englisch =
  britische Flagge). `app/scripts/check-i18n.mjs` prüft bei `npm run build` Schlüssel und
  Platzhalter gegen `de.json`.
- Konsequenzen: Übersetzungen fr/es/it/nl/pl/cs sind maschinell erstellt und sollten von
  Muttersprachlern gegengelesen werden. Die CLI bleibt deutsch.

## ADR-040: Tabellen-Element
- Datum / Status: 2026-10-03 · angenommen
- Kontext: Tabellen wurden bisher aus Rechteck, Linien und Einzeltexten zusammengesetzt (auch
  beim `.lbx`-Import) und ließen sich danach kaum bearbeiten.
- Entscheidung: Eigenes Element `Element::Table` (`ll_core::table`): Zellen als Zeilen von
  Texten, relative Spaltenbreiten/Zeilenhöhen, Linienstärke, Außenrahmen, fette Kopfzeile;
  Text über den normalen Textpfad (Inline-Fett/Kursiv, Platzhalter, eine gemeinsame Größe oder
  „passend“). Ohne Box im Fluss-Layout 10 mm je Spalte. Keine verbundenen Zellen (beim Import
  steht der Text in der ersten Zelle, Hinweis `table_merge`).
- Konsequenzen: `.lbx`-Tabellen werden als ein bearbeitbares Element übernommen. Verbundene
  Zellen wären eine spätere Erweiterung (`spans` je Zelle).

## ADR-041: Rohrleitungskennzeichnung (DIN 2403) als Element, GHS-Icon-Set
- Datum / Status: 2026-10-03 · angenommen (Entwurf vom Nutzer freigegeben, `docs/ENTWURF-DIN2403.md`)
- Kontext: Rohrleitungen nach DIN 2403 kennzeichnen (farbige Pfeile mit Medium, Fließrichtung,
  Zusatzfarbe, Gefahrensymbole) – mit einem Drucker, der nur eine Farbe druckt.
- Entscheidung: Neues Element `Element::PipeMarker` (`ll_core::pipe`): Bandfarbe = Stofffarbe,
  gedruckt wird alles außer der Pfeilform (Rand in Druckfarbe, wahlweise nur Kontur), Trennstriche
  zwischen Spitze und Körper, Spitzen frei/gefüllt/schraffiert (Zusatzfarbe; Rot ist einfarbig
  nicht druckbar → frei oder schraffiert), Medium + Zusatzzeile über den Textpfad, bis zu drei
  Symbole über die Icon-Sets. Vorlage `Layout::PipeMarker`, Assistenten-Kachel mit den Stoffgruppen
  0–9 (`app/src/pipes.ts`: Farben, empfohlene Kassette, typische Medien und GHS-Symbole), Vorschau
  im Assistenten in den RAL-Farben, nach dem Erzeugen Vorschau-Bandstil der Gruppe. CLI
  `generate pipe-marker`. Neues mitgeliefertes Icon-Set `ghs` (GHS01–GHS09 von Wikimedia Commons,
  gemeinfrei, `tools/iconsets/ghs`).
- Konsequenzen: Grau, Violett, Braun gibt es meist nicht als Kassette (Hinweis im Assistenten).

## ADR-042: Mitgelieferte Schrift D-DIN, eigene Farben für Rohrleitungsschilder
- Datum / Status: 2026-10-03 · angenommen
- Kontext: Rohrleitungsschilder nach DIN 2403 nutzen üblicherweise die DIN 1451. Auf den
  Zielrechnern ist sie meist nicht installiert. Der Nutzer wünscht außerdem eigene Farben für die
  Schilder (Hintergrund, Schrift, Zusatzfarbe), z. B. für Werksnormen.
- Entscheidung: D-DIN und D-DIN Condensed (je Normal und Fett; Datto Inc., SIL OFL 1.1, freie
  Nachbildung der DIN 1451) liegen unter `crates/ll-render/assets/fonts/` und werden per
  `include_bytes!` in `ll_render::fonts::database()` geladen. Damit nutzen Vorschau und Druck
  dieselbe Schrift, unabhängig von den Systemschriften. Das Frontend bindet dieselben Dateien per
  `@font-face` für die Schriftauswahl ein (`app/src/fonts/`). Die Vorlage „Rohrleitung“ setzt
  `D-DIN` (`ll_render::fonts::DIN_FAMILY`). Die OFL erlaubt die Weitergabe mit Software, auch
  unter MIT, solange die Lizenzdatei beiliegt und die Schrift nicht allein verkauft wird.
  `PipeMarker.colors` (optional: `background`, `ink`, `extra` als `#rrggbb`) gilt nur für
  farbige Ausgaben (Assistenten-Vorschau, PNG-Export, A4 mit „Farben wie Band“). Die Spitzen
  in der Zusatzfarbe werden als Differenzmaske berechnet: einmal mit, einmal ohne gefüllte
  Spitzen rendern. Der Etikettendrucker druckt weiter einfarbig in den Farben der Kassette.
- Konsequenzen: Binärgröße steigt um ca. 0,4 MB. Ohne gesetzte Farben gelten die
  RAL-Farben der Stoffgruppe.

## ADR-043: Release 1.0.0 und Release-Pakete je Plattform
- Datum / Status: 2026-10-03 · angenommen (Nutzerwunsch: Releases je Plattform, Sprung auf 1.0.0)
- Kontext: Bisher gab es nur das Windows-Artefakt je Push. Für Linux gab es keine fertigen
  Pakete, Releases hingen am Windows-Workflow.
- Entscheidung: Eigener Workflow `release.yml`, ausgelöst durch Tags `v*` (oder manuell mit
  einem Tag). Er prüft, dass Tag, alle vier Manifeste und ein CHANGELOG-Abschnitt
  übereinstimmen, und baut:
  - Windows: portables ZIP wie bisher (ADR-015, kein Installer);
  - Linux auf `ubuntu-22.04` (glibc 2.35 als Untergrenze): AppImage (portabel) und `.deb`,
    Bundling nur hier per `--config` aktiviert. Das `.deb` installiert die udev-Regel
    `packaging/linux/60-labellab.rules` nach `/usr/lib/udev/rules.d/` (VID/PID aus der
    Modelltabelle, unverifiziert) und lädt udev im postinst neu;
  - CLI für Linux als `.tar.gz` mit udev-Regel und `LIESMICH.txt`.
  Veröffentlicht wird mit `softprops/action-gh-release`, Notizen aus dem CHANGELOG,
  `SHA256SUMS.txt`. `windows-build.yml` baut weiter je Push, hängt aber nichts mehr an Releases.
  macOS bleibt außen vor (eigener Bluetooth-Transport nötig).
- Konsequenzen: Der Paketname des `.deb` ist `label-lab` (von Tauri aus dem Produktnamen
  abgeleitet). Die Binärdateien sind weiterhin unsigniert.
