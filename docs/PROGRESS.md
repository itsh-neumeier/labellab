# Fortschritt

> Gemeinsamer Fortschrittsspeicher für Menschen und KI-Agenten. Wird **am Ende jeder Session**
> aktualisiert (siehe `AGENTS.md`). Neueste Einträge im Session-Log oben.

## Aktueller Stand
- **Kurzfassung (2026-10-01):** CLI und Windows-GUI drucken per nativem Bluetooth auf echtem
  PT-P710BT (hardware-bestätigt: Text, QR, Code128-Optik, GUI-Druck, Serie mit 2 Labels,
  Bluetooth-Gerätename „PT-P710BT5265“). Auslieferung als **portable** `LabelLab.exe` +
  `labellab.exe` (GitHub Actions „Windows build“, ADR-015).
- **GUI (M6, weit fortgeschritten):** freies Layout mit Boxen + Einrasten, Text mehrzeilig mit
  pt-Größe/Ausrichtung/Systemschrift/fett/kursiv, QR/Barcode/Bild, Bandfarben-Vorschau
  (automatisch aus Status), glatte 4×-Vorschau oder Druckraster, Mehrband 1×–4×, CSV-Serien,
  Kettendruck („Fortlaufend“), Fortschritt beim Drucken. Details: ADR-014/016/017/018.
- **M7 teilweise vorgezogen:** CSV-Serien und Kettendruck fertig (Kettendruck hardware-offen);
  Kabelfahne/-wickel, Patchpanel, Nummernfolgen `{n:03}` fehlen.
- **M5 code-vollständig:** Symbolbibliothek (10 Material-Symbols-Icons, Apache-2.0, ADR-019)
  parallel auf `main` entstanden und beim Merge übernommen, auch als Label-Element im Editor
  nutzbar (`type: symbol`). **Offen:** Abgleich mit der Nutzerwahl in dieser Session — Tabler
  Icons (MIT) + selbst gezeichnete Warnzeichen im Stil DIN EN ISO 7010 (ergänzen oder ersetzen?).
- **Wichtigster Hardware-Befund zuletzt:** Vorschnitt per Leerseite ergab 3 Schnitte → entfernt;
  der Drucker schneidet bei Auto-Cut den Vorlauf vermutlich selbst (Bestätigung offen).
- **Offen/ungetestet am Gerät:** Kettendruck, Mehrband-Streifen, Farberkennung anderer Bänder,
  USB, Bildimport/Rahmen/EAN usw. — siehe „Hardware-Tests offen“.
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
  App, BlueZ (Linux) fehlen noch. **USB-Transport (`nusb`, ADR-013) fertig** (Code + Unit-Tests
  grün, CLI `--usb`), noch nicht gegen echten Drucker getestet.
- [x] **M5 – Renderer komplett:** Schriften, Rahmen, Barcodes/QR, Bilder, Symbole, `render` → PNG
  Echte Systemschriften und QR-Codes **hardware-verifiziert** (gedruckt + gescannt). 6
  Barcode-Symbologien (Code128, EAN-13/8, UPC-A, Code39, ITF, ADR-010, `ll_render::Symbology`,
  CLI `--barcode-type`) — Code128 gedruckt (Scan-Lesbarkeit offen), Rest nur PNG-Vorschau.
  Bildimport inkl. SVG (PNG/JPEG/BMP/SVG, Floyd-Steinberg-Dithering, `--invert`, ADR-011/012)
  und Rahmen (`ll_render::frame::draw_border`, `--frame`) fertig, noch nicht auf Band gedruckt.
  Symbolbibliothek (10 Material-Symbols-Icons, Apache-2.0, `ll_render::symbols`, ADR-019, CLI
  `--symbol <name>` + `labellab symbols`-Listenbefehl) neu, ebenfalls nur PNG-Vorschau. Alle
  fünf Inhaltsarten (`text`/`--qr`/`--barcode`/`--image`/`--symbol`) teilen sich dieselbe
  `ContentArgs`-CLI-Struktur und denselben Druckpfad (`ll-core::print::print_labels`;
  `print_text`/`print_qr`/`print_barcode`/`print_image`/`print_symbol`). **M5 ist damit code-seitig vollständig** — offene
  Hardware-Tests siehe unten, kein offener Code-Teil mehr.
  Seit dem Merge (2026-10-01) außerdem: Box-Rendering (`ll_render::boxed`, inkl.
  `symbol_in_box`), Systemschriften (`ll_render::fonts`), Symbole als `.llabel`-Element.
- [ ] **M6 – Tauri-GUI:** Geräteleiste mit Bandstatus, Editor, Live-Vorschau, Vorlagen
  **Teilstand (2026-10-01):** `ll_core::label` (Elemente nacheinander entlang des Bandes,
  Abstand/Rand/Mindestlänge/Rahmen, `.llabel`-JSON v1, `render_label` = einziger Renderpfad für
  Vorschau, Druck, CLI `--template`). App `app/` (Tauri 2, Vite + TypeScript, eigener Workspace,
  CI-Job `app`): Geräteliste (USB/BT/seriell), „Status lesen“ setzt die Bandbreite, Modell-/
  Bandwahl, Elementkarten (Text/QR/Barcode/Bild) mit Verschieben/Entfernen, Live-Vorschau mit
  Zoom (automatisch an die Bandhöhe angepasst) und Fehleranzeige, Rückgängig/Wiederholen
  (Strg+Z/Y), Öffnen/Speichern (Strg+O/S), Drucken mit Kopien/Schnitt/Nachlauf (Strg+P),
  i18n de/en. **Neu (ADR-016):** freies Layout mit Boxen (Ziehen, Skalieren, Einrasten mit
  Hilfslinien, Pfeiltasten, X/Y/B/H-Felder, Duplizieren), Text mehrzeilig mit Größe in pt oder
  auto und Ausrichtung, `.llabel` v2. Nutzer hat aus der Windows-GUI gedruckt („Gedruckt.“),
  aber die Vorschau war dort leer → behoben (Base64-Vorschau), **noch nicht erneut unter
  Windows bestätigt** (inzwischen bestätigt). Seitdem: Schriftwahl + fett/kursiv, CSV-Serien,
  Bandfarben, glatte Vorschau, Mehrband (ADR-017/018). **Fehlt:** Symbol-Auswahl in der GUI,
  Rotation, Warnung bei überlaufendem Text, zuletzt verwendete Labels, verständliche
  Fehlertexte für Statusbits.
- [ ] **M7 – Kabel/Serien/CSV + Kettendruck**
  **Teilstand:** CSV-Serien (`ll_core::series`, `{{Spalte}}`/`{{#}}`, Bereich, GUI + CLI
  `--csv/--rows`, ADR-017) und Kettendruck (`PrintOptions::chain`, `--chain`, ADR-018) fertig.
  Mehrband-Labels (ADR-018) zusätzlich. Fehlen: Kabelfahne, Kabelwickel, Patchpanel/Port-Raster,
  Nummernfolgen mit Format (`{n:03}`, Buchstabenfolgen), Vorschau aller Datensätze.
- [ ] **M8 – Release v1.0.0:** Installer, Doku, Screenshots

## In Arbeit
| Aufgabe | Wer (Werkzeug/Person) | Branch | Seit |
|---|---|---|---|
| – | – | – | – |

## Nächste Schritte
0. **Hardware-Test Nachlauf/Schnitt:** `labellab print --barcode "..." --bt` erneut testen —
   schneidet der Cutter jetzt mit sichtbarem Nachlauf statt direkt am Inhalt (Default-Margin
   28 statt 0 Druckpunkte)? `--margin <n>` zum Nachjustieren verfügbar.
1. **Hardware-Test Bildimport (auf dem Gerät mit dem Drucker):** erster Versuch scheiterte an
   einem falschen/nicht gefundenen Dateipfad (`labellab print --image ...` →
   „Datei nicht gefunden“, vermutlich OneDrive-Pictures-Redirect) — Pfad mit `Test-Path`
   prüfen und erneut versuchen, Dithering-Qualität auf echtem Band beurteilen.
2. **Hardware-Test Rahmen:** `labellab print "Text" --frame --bt --device <ID>` — druckt der
   Rahmen sauber (2 Druckpunkte dick, bisher nur PNG-Vorschau verifiziert)?
3. **Hardware-Test weitere Barcode-Symbologien:** z. B. `labellab print --barcode
   "012345678905" --barcode-type ean13 --bt` — EAN/UPC/Code39/ITF bisher nur PNG-Vorschau,
   keine auf echtem Band gedruckt.
3a. **Hardware-Test SVG:** `labellab print --image icon.svg --bt` — bisher nur PNG-Vorschau
    (ein Uhr-Symbol testweise gerendert, sah korrekt aus).
4. Symbole: Material Symbols (ADR-019) ist im Code; mit Nutzer klären, ob Tabler Icons (MIT) +
   eigene Warnzeichen im Stil DIN EN ISO 7010 ergänzt werden. GUI-Auswahl für das Element
   `{"type": "symbol", "name": ...}` fehlt noch (Backend/Renderer fertig). Hardware-Test:
   `labellab print --symbol warning --bt` (Liste: `labellab symbols`).
4a. **GUI unter Windows testen:** `LabelLab.exe` aus dem Artefakt „LabelLab-windows-x64-portable“ des
    Workflows „Windows build“ (oder der vom Agenten geschickten ZIP) starten, mit echtem
    Drucker „Status lesen“ und „Drucken“ ausprobieren. Workflow-Lauf auf GitHub prüfen.
4b. GUI-Ausbau: verständliche (deutsche) Fehlertexte für Druckerfehler/Timeouts, zuletzt
    verwendete Labels, Rotation, Warnung bei überlaufendem Text, Kettendruck für Serien (M7).
5. **Hardware-Test USB:** Drucker per USB anschließen, `labellab devices` → erscheint er?
   `labellab status --usb`, dann `labellab print "TEST" --usb`. Unter Windows wird das Öffnen
   vermutlich scheitern, solange `usbprint.sys`/Brother-Treiber gebunden ist (WinUSB per Zadig
   nötig, ADR-013); unter Linux udev-Regel aus `docs/PROTOCOL.md`.
5a. M4-Rest (programmatisches Pairing, BlueZ/Linux) — wann immer eingeschoben.
6. `--cut` (Auto-Cut) und `--copies N` (Mehrfachdruck) hardware-testen — bisher nur der
   Einzeldruck ohne Schnitt verifiziert.
7. Medientyp-/Farbcode-Bedeutung (Byte 11/24/25) gegen Brothers Farbcode-Tabelle prüfen
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
- [x] ~~M5: `labellab print --barcode "..." --bt` gegen echten Drucker testen~~ – gedruckt,
  2026-10-01: sauberes, optisch korrekt aussehendes Balkenmuster (Foto bestätigt).
- [ ] M5: Code128-Scan-Lesbarkeit mit einem echten Scanner/einer Scanner-App verifizieren
  (Nutzer hatte keinen Code128-fähigen Scanner zur Hand). Balkenbreite 3 Druckpunkte, ADR-010.
- [ ] M5: `labellab print --image <datei> --bt` gegen echten Drucker testen — Dithering-Qualität
  auf echtem Band, nicht nur PNG-Vorschau. Erster Versuch scheiterte an falschem Dateipfad
  (Datei nicht gefunden, kein Code-Befund), noch nicht erfolgreich wiederholt.
- [ ] M5: `labellab print "Text" --frame --bt` gegen echten Drucker testen — Rahmen (2
  Druckpunkte) nur in PNG-Vorschau verifiziert.
- [ ] M5: EAN-13/EAN-8/UPC-A/Code39/ITF gegen echten Drucker/Scanner testen — bisher nur
  PNG-Vorschau, nur Code128 wurde tatsächlich gedruckt.
- [ ] Margin/Nachlauf-Fix erneut hardware-testen: Nutzer meldete, Cutter schneidet direkt am
  Inhaltsende (keine Lücke zum Schnitt). Ursache gefunden: `margin(0)` war fest einprogrammiert.
  Jetzt `PrintOptions::margin_dots` konfigurierbar (CLI `--margin`, Default 28 Druckpunkte statt
  0, TODO(verify) ob 28 ausreicht). Noch nicht erneut gegen echten Drucker getestet.
- [ ] M4: USB-Transport gegen echten Drucker testen (`labellab status --usb`, `print --usb`):
  Endpunkt-Erkennung (Druckerklasse `0x07`, Bulk IN/OUT), Windows mit WinUSB vs.
  `usbprint.sys`, Linux mit udev-Regel und `usblp`-Detach.
- [x] ~~Vorschnitt per Leerseite~~ – Hardware-Test 2026-10-01: 3 Schnitte statt einem →
  entfernt. Offen: Bestätigen, dass mit nur „Abschneiden“ genau ein Schnitt vorn (Vorlauf,
  vom Drucker selbst) und einer hinten entsteht.
- [x] ~~Serien über dieselbe BT-Verbindung (je eigener Auftrag)~~ – Hardware 2026-10-01:
  „2 Label gedruckt“ aus der GUI (Nutzer-Screenshot).
- [x] ~~Bluetooth-Liste zeigt Gerätenamen~~ – Hardware 2026-10-01: „PT-P710BT5265 (Bluetooth) –
  PT-P710BT“ (Nutzer-Screenshot).
- [ ] Kettendruck („Fortlaufend“ / `--chain`): mehrseitiger Auftrag (`0C`/`1A`) — druckt der
  P710BT alle Labels ohne Schnitt, und schneidet „Abschneiden“ nur am Ende?
- [ ] Mehrband (2×/3×): Streifen in richtiger Reihenfolge/Lage, Überlappung ≈ 2,1 mm (12 mm)
  passt beim Aufkleben?
- [ ] Bandfarben-Erkennung jenseits von Weiß/Schwarz (z. B. Gelb `06`, Transparent `03`).
- [ ] M5: SVG-Import (`labellab print --image icon.svg --bt`) gegen echten Drucker testen —
  bisher nur PNG-Vorschau.
- [ ] M5: Symbolbibliothek (`labellab print --symbol <name> --bt`, Namen via `labellab
  symbols`) gegen echten Drucker testen — bisher nur PNG-Vorschau.

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
- 2026-10-01: Code128-Barcode (M5) gedruckt: `labellab print --barcode "ABC-123" --bt`
  erzeugte ein sauberes, optisch korrektes Balkenmuster auf Band. Scan-Lesbarkeit nicht
  verifiziert (Nutzer hatte keinen Code128-Scanner zur Hand) — bleibt offen.

## Session-Log
### 2026-10-01 – Claude Code, Merge nach `main`
- Branch `claude/modest-euler-hx5zk9` mit `main` zusammengeführt; dort war parallel die
  Symbolbibliothek (Material Symbols) entstanden. Konflikte gelöst: deren ADR-013 → ADR-019
  (013 = `nusb`), `print_symbol` und CLI `--symbol` auf den neuen Label-/Druckpfad umgestellt,
  `Element::Symbol` + `boxed::symbol_in_box` ergänzt (Symbole in Vorlagen/Serien nutzbar).
  106 Tests grün.

### 2026-10-01 – Claude Code, Mehrband, Kettendruck, Bandfarben, glatte Vorschau (ADR-018)
- Nutzer-Screenshot: BT-Name „PT-P710BT5265“ wird angezeigt, Serie mit 2 Labels gedruckt.
  Hardware-Befund: Vorschnitt per Leerseite → 3 Schnitte → entfernt.
- Raster Command Reference (PT-E550W/P750W/P710BT v1.02) gelesen (nur lokal, nicht im Repo):
  Farbtabellen Statusbyte 24/25, Mehrseiten-Aufbau, `ESC i A` beim P710BT nicht unterstützt,
  Half-Cut nicht verwendet → `docs/PROTOCOL.md`.
- `ll-protocol::media` (Farbcodes), `command::print_page()` (`0C`). `ll-render`:
  `render_barcode_with_module`, `png::to_png_mask`. `ll-core::label`: `Canvas`, `strips`,
  `render_label_pages`, `render_label_preview(scale)`. `ll-core::print`: `print_labels` mit
  `chain` + Fortschritt, `send_page` (erste/letzte Seite), `pre_cut` entfernt.
- CLI `--chain` statt `--pre-cut`. App: Bandfarben-Auswahl (32 Kombinationen, Auto aus Status),
  Vorschau „Glatt“ (4×) / „Druckraster“, Mehrband 1×–4× mit Streifenlinien, Einrasten an
  Streifengrenzen, Überlappungs-Hinweis, Checkboxen „Abschneiden“ + „Fortlaufend“.
- Geprüft: 102 Tests grün, fmt/clippy grün (auch Windows-Cross-Check), App unter Xvfb: glatte
  Vorschau, Weiß auf Schwarz, 2× Mehrband, Box über beide Streifen gezogen.

### 2026-10-01 – Claude Code, Schriften, CSV-Serien, Schnittoptionen, Gerätenamen (ADR-017)
- Nutzer-Feedback (Screenshot): Vorschau unter Windows jetzt ok, Druck klappt. Neue Wünsche
  umgesetzt: Systemschriften + fett/kursiv (`ll_render::fonts`, fontdb, synthetischer
  Ersatz), Bluetooth-Gerätename statt „SPP SERVER“ + Drucker-Erkennung + Auto-Status nach
  „Suchen“, Checkboxen Vor-/Nachschnitt (`PrintOptions::pre_cut`), CSV-Serien
  (`ll_core::series`, `{{Spalte}}`/`{{#}}`, Bereich), Druckknopf „Wird gedruckt … n/m“ gesperrt.
- CLI: `print --csv --rows 1-10 --pre-cut`, `render --csv --row N`, `devices` zeigt Modell und
  Namen, `--bt --device <Name>`.
- Geprüft: 98 Workspace-Tests grün, fmt/clippy (Linux + Windows-Cross-Check) grün, App unter
  Xvfb: Schriftzeile, Fett, CSV laden, Spalten-Chips einfügen, Vorschau mit Datensatz,
  Druckknopf-Zustand (Fehlerfall). Windows-`.exe` neu gebaut. **Nicht** hardware-getestet:
  Vorschnitt, Serien über BT, Gerätenamen.
- **Stolperstein:** Fokus nach Chip-Klick scrollte das Seitenpanel → `preventScroll`.

### 2026-10-01 – Claude Code, M6 (Teil) – freies Layout, Schriftgrößen, mehrzeiliger Text
- Nutzer-Feedback (Screenshot Windows-GUI): Vorschau leer, Druck klappte; Wunsch nach frei
  positionierbaren Boxen, Schriftgrößen, mehrzeiligem Text und bündigem Aneinanderlegen.
- `ll-render::boxed` (neu): `text_in_box`/`text_natural_width`/`qr_in_box`/`barcode_in_box`/
  `image_in_box`, `TextAlign`; `Bitmap::blit`; `picture::load_gray` und
  `linear_barcode::encode_modules` zur Wiederverwendung herausgelöst. `ll-protocol`:
  `dots_to_mm`, `pt_to_dots`.
- `ll-core::label`: `Item`/`Rect`, `Element::Text { size_pt, align }`, `Element::text()`,
  `resolved_rects()`, Format v2 (v1 lesbar). Fluss-Layout unverändert für Elemente ohne Box.
- App: `resolve_rects`-Befehl, `models()` liefert `tapes[{width_mm, printable_mm}]`, Vorschau
  als Base64 (Windows-Fix). Editor neu (`main.ts`, `snap.ts`), siehe ADR-016.
- Geprüft: 86 Workspace-Tests grün (u. a. Box-Position, Abschneiden außerhalb des Bands,
  Fluss→Box ohne optische Änderung, Auto-Größe ohne Umbruch/Abschneiden), fmt/clippy grün,
  `snap.ts` per Node geprüft, App unter Xvfb: Hinzufügen, Ziehen, Einrasten (Hilfslinien),
  mehrzeiliger 9-pt-Text links. CLI rendert v2-Vorlage mit Boxen korrekt.
- Nutzerwunsch: nur portable `.exe`, kein Installer → NSIS/MSI entfernt (ADR-015-Nachtrag),
  CLI-CRT statisch über `static_vcruntime` statt `.cargo/config.toml` (die brach den
  Tauri-Link). Portable ZIP neu gebaut und geschickt; Windows-CLI unter Wine geprüft.
- **Stolpersteine:** (1) fontdue bricht anders um als eine reine Glyphen-Messung → Auto-Größe
  prüft jetzt mit identischen Layout-Einstellungen, dass kein Zusatzumbruch entsteht.
  (2) Box-DOM während des Ziehens neu aufzubauen verliert die Pointer-Capture → bei
  Vorschau-Updates nur noch neu positionieren.

### 2026-10-01 – Claude Code, Windows-`.exe`
- Nutzerwunsch: vollständig lauffähige `.exe` für Windows. Workflow
  `.github/workflows/windows-build.yml` (ADR-015) baut GUI-`.exe`, NSIS-Setup, MSI und CLI-`.exe`
  als Artefakt; Tags `v*` → Release. README-Abschnitt „Windows: herunterladen und starten“.
- Lokal per `cargo-xwin` cross-gebaut und dem Nutzer geschickt (portable ZIP mit `LabelLab.exe`
  + `labellab.exe` + `LIESMICH.txt`, dazu `LabelLab_0.1.0_x64-setup.exe`).
- Geprüft: Importtabellen (`llvm-objdump -p`) — beide `.exe` brauchen nur Windows-System-DLLs
  (UCRT/WinRT/WinUSB), kein `VCRUNTIME140.dll` (CLI erst nach `+crt-static`). CLI unter Wine
  ausgeführt: `--help`, QR- und Vorlagen-Render (`.llabel` mit QR+Text+Rahmen) korrekt. GUI-`.exe`
  **nicht** unter Wine/Windows gestartet (WebView2 unter Wine nicht praktikabel) → Nutzer-Test
  offen.
- Erster Lauf des Workflows auf GitHub noch nicht beobachtet.

### 2026-10-01 – Claude Code, M6 (Teil) – Layoutmodell, `.llabel`, Tauri-GUI
- `ll-core::label` (neu): `Label`/`Element` (serde, `"type"`-Tag), `render_label()`,
  `render_label_png()`, `geometry_for()`, `Label::load/save` (relative Bildpfade relativ zur
  Datei). `print::print_label()`; `print_text/qr/barcode/image` sind jetzt Hüllen darum (gleiche
  Ausgabe, Golden-Tests unverändert grün; `print_qr` ohne EC-Parameter, immer „Medium“).
- `ll-protocol::model`: `DOTS_PER_INCH` (180) + `mm_to_dots()`. `ll-render::Bitmap`:
  `pixel/extend_blank/prepend_blank/append`; `Symbology` serde-fähig (`"ean13"`, `"upc_a"` …).
- `ll-core::device::Connection` + `connect()`/`query_status_on()` ersetzen die
  `query_status_over_*`-Funktionen; CLI nutzt das (`ConnectOpts::into_connection`).
  Windows-Build per `cargo clippy --target x86_64-pc-windows-gnu` geprüft.
- CLI: `--template <datei.llabel>` auf `print` und `render` (vorher „noch nicht implementiert“).
- `app/`: Tauri-2-App (siehe Meilenstein M6), eigenes Icon (`app/app-icon.svg`, Bandstreifen,
  keine Marken), CI-Job `app` (npm build + fmt/clippy im eigenen Workspace).
- Geprüft: 72 Workspace-Tests grün, fmt/clippy grün (auch App); App unter Xvfb gestartet,
  Screenshots: Vorschau, Elemente hinzufügen/verschieben, Barcode-Fehleranzeige, deutsche UI.
- **Stolpersteine:** WebKitGTK meldet `navigator.language` = Englisch → Default jetzt fest
  Deutsch (Nutzerwahl wird gespeichert). `pkill -f` mit Muster aus der eigenen Kommandozeile
  beendet die eigene Shell — `pkill -x` verwenden.

### 2026-10-01 – Claude Code, M4 (Teil) – USB-Transport
- `ll-transport::usb` (neu, `nusb` 0.2 mit `tokio`-Feature, ADR-013): `list_devices()`,
  `find_printer_endpoints()` (Druckerklasse `0x07`, Bulk OUT + IN aus der aktiven
  Konfiguration), `UsbTransport::open(vid, pid, serial)` mit `detach_and_claim_interface`,
  `read_exact_timeout` über `tokio::time::timeout`. 3 Unit-Tests mit handgebauten
  Konfigurationsdeskriptoren. `TransportError::DeviceNotFound`-Text generalisiert (nicht mehr
  nur Bluetooth).
- `ll-core::device`: `UsbPrinter`, `filter_usb_printers()` (VID/PID gegen
  `ll_protocol::model::MODELS`), `select_usb_printer()` (Modellname / `VVVV:PPPP` /
  Seriennummer / erster), `open_usb()`, `query_status_over_usb()`. 2 Unit-Tests.
- `ll-cli`: `--usb` auf `status` und `print` (`--device` dann optional, schließt `--bt` aus),
  `devices` listet USB-Drucker (auch in `--json`). USB-Aufzählungsfehler (z. B. kein
  `/sys/bus/usb` im Container) brechen `devices` nicht mehr ab, nur Warnung.
- `cargo fmt`/`clippy -D warnings`/`test --workspace` grün (64 Unit-Tests, vorher 59).
- **Stolperstein:** Unter Windows kann `nusb` nur WinUSB-gebundene Interfaces öffnen —
  mit dem Standard-Druckertreiber wird `--usb` vermutlich scheitern (Hardware-Test offen).
- Nutzerentscheidung Symbolbibliothek: **Tabler Icons (MIT)** für Elektro/IT, dazu eigene,
  selbst gezeichnete Warnzeichen-Rahmen im Stil DIN EN ISO 7010 (offizielle ISO-Grafiken nicht
  übernommen, Urheberrecht).
### 2026-10-01 – Claude Code (Sonnet 5), M5 fertig – Symbolbibliothek
- Nutzer wollte Material Symbols **und** eigene SVGs nutzen können — eigene SVGs liefen schon
  über `--image icon.svg` (SVG-Import, siehe vorheriger Eintrag). Für die Bibliothek: keine
  geratenen Download-URLs (Projektregel) — stattdessen npm-Paket `@material-symbols/svg-400`
  (Apache-2.0, deps: keine) als verifizierte Quelle genutzt (`npm view`/`npm pack`,
  Tarball-Inhalt lokal geprüft, kein Rohpfad-Raten nötig).
- 10 Icons kuratiert und nach `crates/ll-render/assets/symbols/*.svg` kopiert (Attribution in
  `assets/symbols/NOTICE.md`): `network`, `wifi`, `power`, `warning`, `arrow-up/-down/-left/
  -right`, `fire`, `fire-extinguisher`. Kein elektrisches Erdungssymbol — Material Symbols ist
  ein allgemeines UI-Set, kein Satz elektrotechnischer Schaltzeichen (ADR-019).
- `ll-render::symbols`: Makro bettet die SVGs per `include_bytes!` ein (`SYMBOL_NAMES`,
  `symbol_svg()`, `render_symbol()`). `ll-render::picture` dafür refaktoriert:
  `render_svg_to_gray()` nimmt jetzt Bytes statt eines Pfads, neue öffentliche
  `render_svg_bytes()` als gemeinsamer Einstieg für Datei-SVGs und eingebettete Symbole.
- `ll-core::print`: neues `print_symbol()`, teilt sich die Protokoll-Sequenz mit den anderen
  vier `print_*`-Funktionen.
- `ll-cli`: `--symbol <name>` auf `print`/`render` (schließt sich mit
  `text`/`--qr`/`--barcode`/`--image` aus), neuer `labellab symbols`-Befehl listet die
  verfügbaren Namen. `Content`-Enum um `Symbol`-Variante erweitert.
- Visuell per Wegwerf-Beispiel geprüft: network/power/warning/fire sehen alle sauber und
  korrekt erkennbar aus, danach entfernt.
- 2 neue Tests in `ll-render` (alle 10 Symbole rendern Tinte, unbekannter Name gibt Fehler
  statt Panic) + 1 in `ll-core` (`print_symbol` sendet Raster-Modus + Feed). 62 Unit-Tests
  insgesamt (vorher 59).
- **M5 ist damit code-seitig vollständig** (Schriften, Rahmen, QR, 6 Barcode-Symbologien,
  Bilder inkl. SVG, Symbolbibliothek). Offene Hardware-Tests für mehrere Teile bleiben (siehe
  „Hardware-Tests offen“) — nur Schriften und QR sind bisher tatsächlich gedruckt/verifiziert.
- Session endet hier auf Nutzerwunsch (Handoff an Cloud-Session nach Nutzungslimit).
  Release-Build erneuert, gepusht.

### 2026-10-01 – Claude Code (Sonnet 5), M5 (Teil) – SVG-Import
- `ll-render::picture`: `render_image()` erkennt `.svg` an der Dateiendung (case-insensitiv)
  und rastert über neue `render_svg_to_gray()`-Funktion (`usvg::Tree::from_data` zum Parsen,
  `resvg::render()` auf einen `tiny_skia::Pixmap` mit weißem, opakem Hintergrund vorgefüllt —
  dadurch kein manuelles Alpha-Compositing nötig, direkte RGB→Luma-Umrechnung reicht). Skaliert
  auf `printable_pins` Höhe, danach derselbe Floyd-Steinberg-Pfad wie PNG/JPEG/BMP (ADR-012).
  Neue Abhängigkeiten `resvg`+`usvg`+`tiny-skia` (selbes Projekt, MIT/Apache-2.0).
  `ll-core`/`ll-cli` brauchten **keine** Änderung — `--image` funktioniert automatisch auch für
  `.svg`-Dateien über denselben Code-Pfad.
- Visuell per Wegwerf-Beispiel geprüft: ein per Hand geschriebenes Uhr-Symbol-SVG (Kreis +
  Zeiger) wurde korrekt und sauber gerendert, danach entfernt.
- 2 neue Tests: `renders_svg_file` (voller Pfad über `render_image()` mit echter Temp-Datei,
  schwarzes Rechteck), `empty_svg_does_not_panic` (0×0-SVG darf nicht crashen).
- `cargo fmt`/`clippy -D warnings`/`test --workspace` grün (59 Unit-Tests, vorher 57).
- **Noch nicht gemacht:** SVG-Druck auf echtes Band getestet (nur PNG-Vorschau verifiziert).
- **M5 jetzt nur noch Symbolbibliothek offen** (braucht Lizenz-/Icon-Set-Entscheidung vom
  Nutzer, dann reine Asset-Arbeit — SVG-Rendering-Pipeline ist technisch bereits fertig).

### 2026-10-01 – Claude Code (Sonnet 5), Bugfix – konfigurierbarer Schnitt-Nachlauf
- Nutzer-Meldung: Cutter schneidet direkt am Ende des gedruckten Inhalts, kein Nachlauf/Lücke.
  Ursache: `send_bitmap()` schickte immer `margin(0)` fest einprogrammiert (TODO(verify) stand
  schon länger im Code, aber keine Priorität bis zum konkreten Nutzer-Feedback).
- `ll-core::print`: `frame: bool` + `auto_cut: bool`-Parameter in allen vier `print_*`-Funktionen
  zu einem gemeinsamen `PrintOptions`-Struct zusammengefasst (`frame`, `auto_cut`,
  `margin_dots`) — Parameterlisten wurden sonst bei jedem neuen Flag länger
  (`clippy::too_many_arguments`-Risiko). `PrintOptions::default()` setzt `margin_dots` auf
  neue Konstante `DEFAULT_MARGIN_DOTS = 28` statt der alten festen `0` (TODO(verify): 28 ist
  eine konservative Schätzung, kein Hersteller-Wert). Neuer Test `custom_margin_is_sent`.
- `ll-cli`: `--margin <dots>` auf `print` (Default 28, `0` = altes Verhalten). `render` bekommt
  kein `--margin` (betrifft nur den Protokollbefehl beim Drucken, nicht die PNG-Vorschau).
- `cargo fmt`/`clippy -D warnings`/`test --workspace` grün (57 Unit-Tests, vorher 56).
- **Noch nicht gemacht:** Fix nicht erneut gegen echten Drucker getestet — Nutzer hatte das
  Problem nur gemeldet, noch keine Bestätigung ob 28 Druckpunkte das eigentliche Problem löst.

### 2026-10-01 – Claude Code (Sonnet 5), M5 (Teil) – weitere Barcode-Symbologien
- `ll-render::linear_barcode`: `Symbology`-Enum (`Code128`/`Ean13`/`Ean8`/`UpcA`/`Code39`/`Itf`),
  `render_barcode(symbology, data, ...)` generalisiert die bisherige Code128-only-Funktion
  (`render_code128()` bleibt als dünner Wrapper für Abwärtskompatibilität). Alle fünf neuen
  Symbologien kommen direkt aus `barcoders` (bereits Abhängigkeit seit Code128, ADR-010) —
  `UPCA` ist dort sogar nur ein Typalias auf `EAN13` (12/13-stelliger Code mit führender `0`),
  keine neue Abhängigkeit nötig. Gemeinsame `render_modules()`-Hilfsfunktion extrahiert
  (vorher in `render_code128()` dupliziert). Visuell per Wegwerf-Beispiel geprüft: EAN-13 sieht
  wie ein echter EAN-13-Barcode aus (Guard-Pattern erkennbar), danach entfernt.
- `ll-core::print`: `print_code128()` → generalisiertes `print_barcode(symbology, data, ...)`.
- `ll-cli`: `--barcode-type <code128|ean13|ean8|upca|code39|itf>` auf `print`/`render` (Default
  `code128`, nur mit `--barcode` relevant). Lokales `BarcodeType`-Enum mit `clap::ValueEnum`
  (Waisenregel: kann `ValueEnum` nicht direkt für das fremde `ll_render::Symbology`
  implementieren), `From<BarcodeType> for Symbology`. Smoke-getestet:
  `render --barcode "012345678905" --barcode-type ean13 -o out.png` erzeugt gültig
  aussehenden EAN-13-Code.
- `cargo fmt`/`clippy -D warnings`/`test --workspace` grün (56 Unit-Tests, vorher 50).
- **Noch nicht gemacht:** keine der fünf neuen Symbologien auf echtes Band gedruckt oder
  gescannt (nur PNG-Vorschau). Nur Code128 wurde bisher tatsächlich gedruckt.
- **Noch offen in M5:** SVG-Import, Symbolbibliothek. Das war die letzten zwei fehlenden Stücke.

### 2026-10-01 – Claude Code (Sonnet 5), M5 (Teil) – Rahmen
- `ll-render`: neues `frame`-Modul (`draw_border()` zeichnet ein Rechteck um das ganze Label:
  kurze „Kappen“ an Anfang/Ende entlang der Pin-Achse, lange „Seiten“ entlang der
  Bandbreiten-Grenzen über die volle Länge). Bewusst kein Element-Kompositionsmodell
  (mehrere positionierbare Objekte pro Label) — das ist M6-Editor-Scope; deckt den
  Standardfall „ein umrandetes Label“ ab (z. B. Warnschild). Visuell per Wegwerf-Beispiel
  geprüft: Rahmen um „WARNUNG“-Text sauber gezeichnet, danach entfernt.
- `ll-core::print`: jede `print_*`-Funktion bekommt einen neuen `frame: bool`-Parameter,
  zeichnet bei `true` den Rahmen (fest 2 Druckpunkte dick) auf das bereits gerenderte Bitmap,
  bevor es gesendet wird (gemeinsamer `maybe_draw_border()`-Helfer). Neuer Test
  `frame_adds_more_ink_than_without` (vergleicht Anzahl leerer Rasterzeilen mit/ohne Rahmen).
- `ll-cli`: `--frame` auf `print` und `render`, gilt für alle vier Inhaltsarten
  (Text/QR/Barcode/Bild). Smoke-getestet: `render "WARNUNG" --frame -o out.png` zeigt
  sauberen Rahmen um den Text.
- `cargo fmt`/`clippy -D warnings`/`test --workspace` grün (50 Unit-Tests, vorher 45).
- **Noch nicht gemacht:** Rahmen auf echtes Band gedruckt (nur PNG-Vorschau verifiziert).
  Bildimport-Hardware-Test vom Nutzer versucht, scheiterte an falschem Dateipfad (Datei nicht
  gefunden, OneDrive-Pictures-Verdacht) — noch nicht erfolgreich wiederholt.
- **M5 fast vollständig:** nur noch weitere Barcode-Symbologien (EAN/UPC/Code39/ITF),
  SVG-Import und Symbolbibliothek offen.

### 2026-10-01 – Claude Code (Sonnet 5), M5 (Teil) – Bildimport + Dithering
- `ll-render`: neues `picture`-Modul (`render_image()` lädt PNG/JPEG/BMP, `render_gray()` ist
  die reine, dateisystemfreie Kernlogik — testbar mit synthetischen `GrayImage`s statt echten
  Dateien). Skaliert auf `printable_pins` Höhe (Seitenverhältnis erhalten), klassisches
  Floyd-Steinberg-Error-Diffusion selbst implementiert (kein eigenes Dithering-Crate, siehe
  ADR-011), `--invert`-Option. `image`-Crate um `jpeg`/`bmp`-Features erweitert (neben
  vorhandenem `png`). Visuell per Wegwerf-Beispiel geprüft: Radialverlauf dithert korrekt
  (klassisches FS-Streumuster erkennbar), danach entfernt.
- `ll-core::print`: neues `print_image()`, teilt sich `read_status_and_geometry()`/
  `send_bitmap()` mit den anderen `print_*`-Funktionen. Test mit echter Temp-PNG-Datei
  (`image`-Crate als Dev-Dependency für `ll-core`).
- `ll-cli`: `--image <datei>` + `--invert` auf `print` und `render` (schließt sich mit
  `text`/`--qr`/`--barcode` aus). `Content`-Enum um `Image`-Variante erweitert, `ContentArgs`
  bündelt die vier sich gegenseitig ausschließenden Inhaltsquellen plus `invert`
  (sonst `clippy::too_many_arguments`). Smoke-getestet: synthetisches Radialverlauf-PNG über
  `render --image ... -o out.png` korrekt gedithert.
- `cargo fmt`/`clippy -D warnings`/`test --workspace` grün (45 Unit-Tests, vorher 38).
- **Noch nicht gemacht:** Bilddruck auf echtes Band getestet (nur PNG-Vorschau verifiziert).
- **Noch offen in M5:** Rahmen/Linien, weitere Barcode-Symbologien, SVG-Import,
  Symbolbibliothek.

### 2026-10-01 – Claude Code (Sonnet 5), M5 (Teil) – Code128-Barcode
- `ll-render`: neues `linear_barcode`-Modul (`render_code128()` via `barcoders`-Crate, siehe
  ADR-010). Anders als Text/QR: ein Balken-Modul füllt die **gesamte** bedruckbare Bandbreite
  (keine vertikale Struktur bei 1D-Barcodes). `barcoders` braucht ein
  Zeichensatz-Präfix (`À`/`Ɓ`/`Ć`); Wrapper setzt automatisch Zeichensatz B, wenn keins
  angegeben. Visuell per Wegwerf-Beispiel geprüft: sieht wie ein echter Code128-Barcode aus
  (saubere Balkenmuster), danach entfernt.
- `ll-core::print`: neues `print_code128()`, nutzt dieselben
  `read_status_and_geometry()`/`send_bitmap()`-Hilfsfunktionen wie `print_text`/`print_qr`.
- `ll-cli`: `--barcode <daten>` auf `print` und `render` (schließt sich mit `text`/`--qr`
  gegenseitig aus, `clap conflicts_with_all`), `Content`-Enum um `Code128`-Variante erweitert.
  Smoke-getestet: `render --barcode "LABELLAB-123" -o out.png` erzeugt gültig aussehenden
  Barcode.
- `cargo fmt`/`clippy -D warnings`/`test --workspace` grün (38 Unit-Tests, vorher 34).
- **Noch nicht gemacht:** Code128-Druck auf echtes Band getestet (nur PNG-Vorschau verifiziert).
- **Noch offen in M5:** Rahmen/Linien, weitere Barcode-Symbologien (EAN/UPC/Code39/ITF),
  Bilder, Symbolbibliothek.

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
