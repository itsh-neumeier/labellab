# Fortschritt

> Gemeinsamer Fortschrittsspeicher für Menschen und KI-Agenten. Wird **am Ende jeder Session**
> aktualisiert (siehe `AGENTS.md`). Neueste Einträge im Session-Log oben.

## Aktueller Stand
- **Kurzfassung (2026-10-01):** CLI und Windows-GUI drucken per nativem Bluetooth auf echtem
  PT-P710BT (hardware-bestätigt: Text, QR, Code128-Optik, GUI-Druck, Serie mit 2 Labels,
  Bluetooth-Gerätename „PT-P710BT5265“). Auslieferung als **portable** `LabelLab.exe` +
  `labellab-cli.exe` (GitHub Actions „Windows build“, ADR-015).
- **GUI (M6, weit fortgeschritten):** freies Layout mit Boxen + Einrasten, Text mehrzeilig mit
  pt-Größe/Ausrichtung/Systemschrift/fett/kursiv, QR/Barcode/Bild, Bandfarben-Vorschau
  (automatisch aus Status), glatte 4×-Vorschau oder Druckraster, Mehrband 1×–4×, CSV-Serien,
  Kettendruck („Fortlaufend“), Fortschritt beim Drucken. Details: ADR-014/016/017/018.
- **M7 code-vollständig (ADR-020):** CSV-Serien, Kettendruck, Nummernfolgen (`{{n:03}}`,
  `{{A}}`), Kabelfahne, Kabelwickel, Patchpanel (CLI `generate`, GUI „Assistent …“), dazu
  Drehung und Linie/Fläche. Hardware-Tests offen (Kettendruck, Längen-Genauigkeit).
- **Stand auf `main`:** PR itsh-neumeier/labellab#1 am 2026-10-01 gemergt (Squash); M7-Arbeit
  liegt danach auf `claude/modest-euler-hx5zk9`.
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
  **Code-vollständig (ADR-021), Hardware-Tests teilweise offen.** WinRT-RFCOMM-Connect
  **hardware-verifiziert** (2026-10-01, PT-P710BT). Neu: Kopplung per Programm unter Windows
  (WinRT Custom Pairing, PIN `0000`) und Linux (BlueZ-Agent), BlueZ-RFCOMM-Transport (`bluer`,
  Kanal 1), CLI `labellab pair [Gerät]`, GUI „Koppeln …“. USB-Transport (`nusb`, ADR-013) fertig.
  Offen am Gerät: Kopplung (Windows/Linux), BlueZ-Druck, Kanal 1, USB.
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
- [x] **M7 – Kabel/Serien/CSV + Kettendruck** (code-seitig; Hardware-Tests offen)
  CSV-Serien (ADR-017), Kettendruck + Mehrband (ADR-018), Nummernfolgen, Kabelfahne,
  Kabelwickel, Patchpanel, Drehung, Linie/Fläche (ADR-020). CLI `generate`, `--count/--start/
  --step`; GUI „Assistent …“, Bereich „Nummerierung“. Noch nicht: Vorschau aller Datensätze
  als Liste, Half-Cut (vom P710BT nicht unterstützt).
- [ ] **M8 – Release v1.0.0:** Installer, Doku, Screenshots
- [ ] **M9 – Editor-Ausbau nach Funktionsvergleich** (Nutzerwunsch 2026-10-02: Screenshots des
  Hersteller-Editors als Anregung; nur Funktionsideen übernommen, keine Grafiken/Dateien des
  Herstellers ins Repo). Paket 1 – Editor-Grundlagen:
  - [x] Formen: Linie, Rechteck, abgerundetes Rechteck, Oval (Kontur/gefüllt, Linienstärke)
  - [x] Feste Länge (exakt, Inhalt wird abgeschnitten) zusätzlich zur Mindestlänge
  - [x] Datum/Uhrzeit als Platzhalter (`{{datum}}`, `{{zeit}}`, beim Druck aktuell)
  - [x] Bild: Helligkeit/Kontrast
  - [x] Bild-Editor „Bild bearbeiten …“ (Nutzerwunsch 2026-10-02): Zuschneiden, Drehen 90°,
    Spiegeln, Hintergrund entfernen (Randfarbe automatisch oder Farbe im Bild wählen,
    Toleranz, nur zusammenhängend), Raster oder Schwelle; nicht-destruktiv (ADR-026)
  - [x] Strg+V fügt Bilder und Texte aus der Zwischenablage ein, Strg+C/X/V für Elemente;
    Oberflächentexte nicht markierbar (Nutzerwunsch 2026-10-02, ADR-027)
  - [x] Vorschau in voller Bandbreite mit grauem, nicht bedruckbarem Rand; Zeilenabstand für
    Text; schnellere Vorschau bei viel Inhalt (Nutzerwunsch 2026-10-02, ADR-028)
  - [x] Einzelne Textteile fett/kursiv (`**fett**`, `__kursiv__`; F/K bzw. Strg+B/I auf die
    Markierung) (Nutzerwunsch 2026-10-02, ADR-029)
  - [x] Rand links/rechts getrennt einstellbar, in der Vorschau als Zonen, Ausrichten/
    Einrasten daran, Hinweis bei Elementen im Rand (Nutzerwunsch 2026-10-02)
  - [x] Boxen an allen vier Seiten und Ecken ziehbar (Nutzerwunsch 2026-10-02)
  - [x] Rahmen innerhalb der Label-Ränder, Abstand je Seite; Inhalt in der Box horizontal/
    vertikal ausrichten; senkrechtes Lineal links (Nutzerwunsch 2026-10-02, ADR-030)
  - [x] Dialog „Druckerinfo …“ (Modell, Band, Bandtyp, Farben, Fehler im Klartext,
    Rohdaten zum Kopieren); Startbildschirm/„Über …“ mit Entwickler, Copyright, Lizenz
  - [x] Linke Leiste: Bereiche auf-/zuklappbar (Elemente/Label offen, Rest zu; Zustand wird
    gemerkt) (Nutzerwunsch 2026-10-02)
  - [x] Dekorative Rahmen wie im Hersteller-Editor: Rahmen aus Start-, Wiederhol- und
    Endsegment (SVG), mitgelieferte Bibliothek + Editor/Creator (Nutzerwunsch 2026-10-02,
    ADR-032)
  - [x] Band-Ausrichtung im Editor: waagerecht oder senkrecht bearbeiten (Nutzerwunsch
    2026-10-02, = Hochformat aus Paket 3b, ADR-031)
  - [x] Eigene Namen für Elemente (Doppelklick in der Liste, Feld „Name“)
  - [x] USB unter Windows: Fehler „incompatible driver is installed“ (Nutzer 2026-10-02) —
    neuer Transport `ll_transport::usbprint` über die Geräteschnittstelle des
    Windows-Druckertreibers, `nusb` nur noch Rückfall (Hardware-Test offen)
  - [x] Schnittoptionen wie im Hersteller-Editor (Nutzerwunsch 2026-10-02, Hilfe-Seite
    „Schnittoptionen“ gelesen): Auswahl statt Häkchen – jedes Etikett schneiden / Schnitt am
    Ende / Kettendruck (kein Vorschub nach dem letzten) / kein Schnitt (Spezialband) /
    Schnittmarken drucken / alle N Etiketten schneiden (per Seiten-Schnittflag im Kettenauftrag,
    TODO(verify)) / Spiegeldruck. Halbschnitt am PT-P710BT unverifiziert → erst Hardware-Test.
  - [x] Daten (CSV): Beispieldatei erzeugen und speichern lassen (Nutzerwunsch 2026-10-02)
  - [x] Automatisches Speichern (wie Office): Schalter, nach dem ersten Speichern standardmäßig
    an; Rückgängig/Wiederholen als Symbol-Knöpfe (Nutzerwunsch 2026-10-02)
  - [x] Editor-Aufbau wie Ebenen in Bildbearbeitung: links nur Elementliste (Name, Sperren,
    Ein-/Ausblenden, Duplizieren, Löschen), rechts Eigenschaften-Leiste für das gewählte
    Element (Nutzerwunsch 2026-10-02; Ausblenden braucht Feld `hidden` am Item)
  - [ ] Import von Dateien des Hersteller-Editors (`.lbx`, ZIP mit `label.xml`) in `.llabel`
    (Nutzerwunsch 2026-10-02; Beispieldateien des Nutzers nicht ins Repo). Analyse und Plan:
    `docs/IMPORT-LBX.md`
  - [ ] Akkustand: Statusbyte per Rohdaten-Vergleich (Ladekabel/Akku) ermitteln, dann anzeigen
  - [x] Knopf „Wach halten“ (Keep-alive per Statusabfrage alle 2 min, beim Start aus)
  - [x] Ausrichten am Label (links/Mitte/rechts, oben/Mitte/unten), Seitenverhältnis
    beim Skalieren fixieren, Element sperren (nicht verschiebbar)
  - [x] Lineal (mm) über der Vorschau
  Paket 2 – Vorlagen-Startseite:
  - [x] Vorlagen-Galerie („Vorlagen …“, früher „Assistent“) mit Kategorien, Kacheln und
    Live-Vorschau: Kabelfahne, Einzelfähnchen, Kabelwickel, Patchpanel/Ports, Klemmblock/LSA
    (1–2 Reihen), Sicherungskasten/Verteiler (senkrecht, Hauptschalter). Offen daraus:
    Selbstlaminierend und Schrumpfschlauch (brauchen eigene Bandgeometrie, s. u.)
  - [ ] Selbstlaminierende Bänder (bedruckbarer Teil + transparente Wickelzone) und
    Schrumpfschlauch-Bänder (HS 5,8–23,6 mm) — Geometrie/Medientypen `TODO(verify)`
  - [x] „Erstellte Labels“: Druckverlauf (`ll_core::history`, Datenordner `history/`, max. 50,
    Vorschaubild, Datum, Band, Anzahl; „Verlauf …“ in der Werkzeugleiste, Öffnen stellt die
    Bandbreite wieder ein). „Zuletzt verwendet“ (Pfade) gab es schon.
  Paket 3 – Dokument/Layout:
  - [x] Mehrere Arbeitsblätter in einer Datei (Reiter über der Vorschau: +, Doppelklick =
    umbenennen, × = löschen; Bandbreite je Blatt), Warnung bei ungespeicherten Änderungen
    (Neu, Öffnen, Zuletzt verwendet, Verlauf, Fenster schließen; „•“ im Titel)
  - [ ] Hochformat (ganzes Label gedreht)
  - [ ] Tabellen (Zellen mit Text), dekorative Rahmen-Bibliothek (als Icon-Set-artige
    Sammlung), Z-Reihenfolge, Objektnamen

## In Arbeit
| Aufgabe | Wer (Werkzeug/Person) | Branch | Seit |
|---|---|---|---|

## Nächste Schritte
- Offene Nutzerwünsche (Stand 2026-10-02 abends): `.lbx`-Import (Plan `docs/IMPORT-LBX.md`),
  Akkuanzeige (wartet auf zweiten Rohdaten-Block), Hardware-Tests (USB/usbprint, Schnitt,
  Länge). Erledigt: Auto-Speichern, Ebenen, Hochformat, Deko-Rahmen, Schnittoptionen, CSV-Beispiel,
  Code-Assistent, Sicherungskasten-Felder verbinden + „Vorlage bearbeiten“, Vorlagen-Ziel.
- Offene Nutzerwünsche (2026-10-02): dekorative Segment-Rahmen + Editor, senkrechte
  Bandausrichtung im Editor, Akkuanzeige nach Rohdaten-Test, macOS-Build (Tauri kann es,
  braucht macOS-Runner in CI + Bluetooth über CoreBluetooth/IOBluetooth — neuer Transport),
  Code-Signatur für Windows (braucht gekauftes Zertifikat oder Azure Trusted Signing).
0. **Hardware-Test Länge/Schnitt:** Label mit fester Länge 100 mm und Rand links/rechts 4 mm
   drucken → 100 mm lang, 4 mm bis zum Schnitt? (Nachlauf jetzt Standard 0, wird sonst von
   den leeren Enden abgezogen, siehe Session 2026-10-02 „Fix Labellänge“.)
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
4b. GUI-Ausbau erledigt (übersetzte Fehlertexte, Überlauf-Warnung, „Zuletzt verwendet“,
    „Serie ansehen …“). Weitere Wünsche nach Nutzer-Feedback.
4c. M8 (Release v1.0.0): Versionsnummern, Release-Workflow per Tag `v*` existiert (portable
    `.exe`), Screenshots/Doku für Endnutzer, ggf. Code-Signierung.
5. **Hardware-Test USB:** Drucker per USB anschließen, `labellab devices` → erscheint er?
   `labellab status --usb`, dann `labellab print "TEST" --usb`. Unter Windows wird das Öffnen
   vermutlich scheitern, solange `usbprint.sys`/Brother-Treiber gebunden ist (WinUSB per Zadig
   nötig, ADR-013); unter Linux udev-Regel aus `docs/PROTOCOL.md`.
5a. **Hardware-Test M4-Kopplung/BlueZ:** siehe „Hardware-Tests offen“ (ADR-021).
6. `--cut` (Auto-Cut) und `--copies N` (Mehrfachdruck) hardware-testen — bisher nur der
   Einzeldruck ohne Schnitt verifiziert.
7. Medientyp-/Farbcode-Bedeutung (Byte 11/24/25) gegen Brothers Farbcode-Tabelle prüfen
   (aktuelle Werte: `0x01`/`0x01`/`0x08`, siehe `docs/PROTOCOL.md`).

## Hardware-Tests offen
- **Kein Schnitt (2026-10-02):** Schnitt „Kein Schnitt“ und „Kettendruck ohne Schnitt“
  drucken → darf am Ende **nicht** schneiden (neu: `ESC i K` Bit 3 = 0). Gegenprobe „Jedes
  Etikett“ → schneidet weiter wie bisher. Bleibt das Band nach „Kein Schnitt“ im Drucker,
  ist das erwartet (kein Vorschub); nächstes Label oder Vorschubtaste schiebt es heraus.
- Schnittoptionen: „Alle N Etiketten“ (Schnitt-Flag je Seite im Kettenauftrag), Schnittmarken,
  Spiegeln, „Kettendruck (kein Schnitt)“ am Gerät prüfen.
- USB unter Windows ohne Treibertausch: Status lesen und Drucken über `usbprint.sys`
  (`ll_transport::usbprint`). Klappt das Lesen des Status nicht, Fehlermeldung notieren.
- Länge: Label mit fester Länge 100 mm (Rand links/rechts ≥ 4 mm) drucken → genau 100 mm?
  (Vorschub `1B 69 64` wird jetzt von den leeren Label-Enden abgezogen, PROTOCOL.md)
- „Wach halten“: einschalten, Drucker länger als seine Abschaltzeit liegen lassen — bleibt
  er an? (Bluetooth und USB; Statusabfrage als Keep-alive ist unverifiziert, PROTOCOL.md)
- Windows: Bild (Screenshot, Bild aus dem Browser) und Text mit Strg+V einfügen,
  Element mit Strg+C/Strg+V kopieren.
- Bild-Editor: bearbeitetes Bild (Hintergrund entfernt, Schwelle) auf Band drucken und mit
  der Vorschau vergleichen.
> Tests, die nur mit echtem Drucker beantwortet werden können. Ergebnis in `PROTOCOL.md` übertragen.

- [ ] Sicherheitszeichen auf Band: `labellab print --symbol iso7010:W012 --bt` (und z. B.
  `iso7010:M001`, `iec60417:5017`) — sind Piktogramme auf 9/12 mm noch erkennbar?
- [ ] Fenster schließen mit ungespeicherten Änderungen → Abfrage erscheint, „Abbrechen“ lässt
  das Fenster offen (Windows)?
- [ ] Druckverlauf: nach einem echten Druck erscheint der Eintrag unter „Verlauf …“?
- [ ] Rahmenstile auf Band: `labellab print "TEST" --border striped --border-sides ou
  --border-width 1 --bt` (und dashed/dotted/double) — Muster sauber, Streifen nicht verwaschen?
- [ ] M4: Kopplung unter Windows: Drucker in den Windows-Einstellungen entfernen, dann
  `labellab pair` (bzw. GUI „Koppeln …“) — klappt Custom Pairing mit PIN `0000` oder ohne PIN?
- [ ] M4: Linux/BlueZ: `labellab pair`, `labellab devices`, `labellab status --bt --device <MAC>`
  und `print` — ist der SPP-Dienst auf RFCOMM-Kanal 1 (`TODO(verify)` in `ll-protocol`)?
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
- [ ] M7: Patchpanel-Label (z. B. 24 × 12,7 mm) drucken und gegen das echte Panel halten —
  stimmt die Länge/das Raster (Vorschub-Genauigkeit)?
- [ ] M7: Kabelfahne mit echtem Kabel: passt der Wickelbereich (π × Durchmesser)?
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
### 2026-10-02 – Claude Code, Sicherungskasten als Element (ADR-034)
- Nutzer-Screenshots aus dem Hersteller-Editor: Sicherungskasten ist ein Element mit
  Eigenschaften. Neu: `Element::FuseBox` (`ll-core/src/fusebox.rs` Modell + Trennlinien,
  `label::render_fuse_box` Text), `boxed::text_fit_px` für eine gemeinsame Größe; CSV-
  Platzhalter in Feldtexten. GUI: Knopf „Sicherungskasten“, Eigenschaften mit Feldliste
  (Text, Faktor, Richtung, ⇔). Generator `layouts::fuse_box` liefert ein Element.
- Geprüft: Tests (Trennstile, Spannen, Rendern, Generator), fmt/clippy, Build; GUI unter Xvfb.
- Offen/Ideen: Feldtext direkt im Label anklicken (wie im Hersteller-Editor), Hochformat.

### 2026-10-02 – Claude Code, Fix „Kein Schnitt“ schneidet trotzdem
- Nutzer-Bug: „Kein Schnitt (Spezialband)“ schnitt am Ende. Ursache: `ESC i K` (Advanced
  Mode) wurde nie gesendet; Bit 3 „no chain printing“ steht im Drucker offenbar auf 1
  (vorschieben + schneiden nach dem letzten Label). Jetzt je Seite `advanced_mode(auto_cut)`.
  Tests (Befehlsbytes, Kettenauftrag). Hardware-Test eingetragen.

### 2026-10-02 – Claude Code, Fix Symbol-Vorschau („missing field `name`“)
- Nutzer-Bug: Symbole ließen sich nicht rendern. Ursache: `Item::name` (eigener Elementname,
  ADR-031) wird ins Element-JSON geflacht und schluckte den `name` des Symbols. Elementname
  heißt jetzt `title` (Rust + TS), Regressionstest. Vor heute gespeicherte eigene Namen
  (Schlüssel `name`) bei Nicht-Symbol-Elementen gehen verloren (nur kurz veröffentlicht).

### 2026-10-02 – Claude Code, Knopf „Code“
- Nutzerwunsch: Code-Assistent als normaler Hinzufügen-Knopf wie Text/Bild → Beschriftung
  „Code“ (Tooltip erklärt den Assistenten). Geprüft: Build, GUI unter Xvfb.

### 2026-10-02 – Claude Code, Code-Assistent, Verteiler-Felder verbinden, Vorlagen-Ziel (ADR-033)
- „Code-Assistent …“ statt der Knöpfe QR-Code/Barcode: Dialog mit Code-Typ, für QR Inhaltsarten
  (Text, Link, WLAN, vCard, E-Mail, Telefon; `app/src/codes.ts` baut/erkennt den Inhalt),
  Live-Vorschau über `render_preview` (Fehler sperren „Einfügen“), Hinweise je Symbologie.
  In den Eigenschaften von QR/Barcode „Assistent …“ zum Bearbeiten (QR ⇄ Barcode möglich).
- Sicherungskasten: `FuseBox::spans`/`texts` (ll-core, CLI `--spans 1,3,2 --texts ";FI"`),
  Feldleiste im Assistenten (⇔ verbinden, ✂ trennen, Text je Feld). `Label::source` speichert
  die Vorlage; „Vorlage bearbeiten …“ unter Label öffnet sie wieder. Ziel: aktuelles Blatt /
  neues Blatt / neue Datei.
- Fix: UPC-A wurde als EAN-13 ohne führende 0 kodiert.
- Stolperstein: horizontal scrollende Feldleiste – die GTK-Overlay-Scrollbar fing Klicks auf
  ⇔ ab; Leiste bricht jetzt um statt zu scrollen. Lückenhaftes `texts`-Array → `null` im
  JSON → Fehler; wird jetzt aufgefüllt.
- Geprüft: fmt/clippy/Tests (UPC-A-Test neu), Build; GUI unter Xvfb (WLAN-QR einfügen und
  wieder öffnen, EAN-13-Fehler, QR → Code 128, Verteiler als neues Blatt, Vorlage bearbeiten).
  Nicht in der GUI geprüft: Ziel „Neue Datei“ (nativer Rückfrage-Dialog).
- Offen: `.lbx`-Import, Akkuanzeige, Hardware-Tests.

### 2026-10-02 – Claude Code, Beispiel-CSV
- „Beispiel-CSV …“ unter Daten (CSV): Spalten = vom Label genutzte `{{Platzhalter}}` (ohne
  n/a/A/datum/zeit), sonst „Name;Raum;Nummer“; 3 Beispielzeilen, `;`-getrennt; Speicherort
  per Dialog, danach direkt geladen. App-Befehl `save_text_file`.
- Geprüft: Build; GUI unter Xvfb (Datei erzeugt und geladen).

### 2026-10-02 – Claude Code, Schnittoptionen
- Druckleiste: Auswahl „Schnitt“ (jedes Etikett / Schnitt am Ende / alle N Etiketten /
  Kettendruck ohne Schnitt / kein Schnitt) statt zwei Häkchen, dazu „Schnittmarken“ und
  „Spiegeln“; Auswahl wird gemerkt. `PrintOptions::cut_every` (Schnitt-Flag je Seite im
  Kettenauftrag, TODO(verify)), `cut_marks` (gepunktete Linie am Ende jedes Etiketts),
  `mirror` (`Bitmap::mirrored`). Halbschnitt nicht eingebaut (am PT-P710BT unverifiziert).
- Geprüft: Tests (2 neue), fmt/clippy, Build; Druckleiste unter Xvfb.

### 2026-10-02 – Claude Code, Deko-Rahmen aus Segmenten (ADR-032)
- `ll_render::decor`: Rahmen aus drei SVG-Segmenten (Anfang, Mitte wiederholt und leicht
  gestreckt, Ende) in voller druckbarer Höhe; Sets als `.llabel-frames` mit Registry.
  Mitgeliefertes Set „basis“ (8 eigene Rahmen, MIT): Geschweifte Klammer, Pfeilband, Welle,
  Punktreihe, Doppellinie mit Raute, Zickzack, Banner, Abgerundet.
- `Label::decor` (`set:frame`) zwischen den Label-Rändern; Fluss-Inhalt hält Abstand zu
  Anfang/Ende. `ll_core::frames`: Sets aus `<Datenordner>/frames/` laden/importieren/
  entfernen, eigene Rahmen im Set „eigene“.
- App: „Deko-Rahmen“ unter Rahmen mit Galerie (Vorschaubilder), Rahmen-Editor (drei
  SVG-Felder oder Dateien, Live-Vorschau, Speichern), Set-Import; beim Wählen rücken Boxen
  hinter das Anfangsstück.
- Geprüft: Tests (5 neue), fmt/clippy, Build; CLI-Render aller 8 Rahmen; GUI unter Xvfb
  (Galerie, Auswahl, Editor speichern).

### 2026-10-02 – Claude Code, USB unter Windows über usbprint.sys
- Nutzerfehler „incompatible driver is installed for this device“: `nusb` braucht WinUSB.
  Neu `ll_transport::usbprint` (Windows): SetupAPI-Suche nach `GUID_DEVINTERFACE_USBPRINT`,
  Gerät per `std::fs` öffnen, Lesen/Schreiben im Blocking-Pool; `open_usb` nimmt unter
  Windows zuerst diesen Weg. Fehlertext „Systemfehler“ nennt jetzt auch USB-Ursachen.
- Geprüft: `cargo check`/`clippy` für `x86_64-pc-windows-msvc` (ll-transport, ll-core),
  Linux-Tests. **Nicht am Gerät geprüft.**

### 2026-10-02 – Claude Code, Hochformat, Elementnamen (ADR-031)
- `Label::orientation` (`landscape`/`portrait`); Hochformat-Boxen in Hochformat-Koordinaten,
  `Label::to_landscape` (Box gedreht, Inhalt +270°) vor dem Rendern/Drucken; Vorschau-PNG im
  Hochformat gedreht (nur Anzeige); `resolved_rects` liefert Hochformat-Boxen zurück.
- App: „Ausrichtung“ unter Label; Umschalten dreht Boxen und Inhalt mit (Ausdruck bleibt
  gleich); Editor, Lineale, Ränder, Einrasten, Ausrichten, neue Elemente für Hochformat.
- `Item::name`: eigener Name in der Ebenenliste (Doppelklick) und Feld „Name“.
- Geprüft: Tests (1 neuer: Hochformat druckt wie Querformat-Gegenstück), fmt/clippy, Build;
  GUI unter Xvfb (Umschalten, Umbenennen).

### 2026-10-02 – Claude Code, Ebenen links, Eigenschaften rechts
- Links nur noch die Elementliste (Ebenen): Typ, Inhalt, Ein-/Ausblenden, Sperren,
  Duplizieren, Löschen; rechts neue Leiste „Eigenschaften“ mit allen Einstellungen des
  gewählten Elements (bisherige Elementkarte, Drehen im Kopf).
- `Item::hidden` (ll-core): ausgeblendete Elemente werden nicht gerendert/gedruckt, zählen
  aber für die Labellänge mit (Länge springt nicht beim Ausblenden). Box blass/gepunktet.
- Geprüft: Tests (1 neuer), Frontend-Build; GUI unter Xvfb (Auswahl, Ausblenden).

### 2026-10-02 – Claude Code, Auto-Speichern, Speichern/Speichern unter, Undo/Redo-Symbole
- „Speichern“ schreibt direkt in die geöffnete Datei (Strg+S), „Speichern unter …“
  (Strg+Umschalt+S) fragt nach. Schalter „Auto-Speichern“: aktiv, sobald die Datei einen
  Pfad hat; speichert 1,5 s nach der letzten Änderung (Vorgabe pro Nutzer in localStorage,
  Standard an). Rückgängig/Wiederholen als SVG-Symbolknöpfe; Werkzeugleiste bricht bei
  wenig Platz um.
- Rohdaten des Nutzers in PROTOCOL.md ausgewertet (Modellbytes verifiziert, Medientyp
  `0x14` unbekannt). Akkutest braucht noch einen zweiten Block im anderen Stromzustand.
- Geprüft: Frontend-Build, Tests; GUI unter Xvfb (Speichern mit Dialog, Änderung nach 1,5 s
  in der Datei).

### 2026-10-02 – Claude Code, Druckerinfo und Startbildschirm
- `ll_protocol::status`: `series_byte`/`model_byte`/`model()`, `notification`, `ERROR_BITS` +
  `error_ids()` (laut Raster Command Reference, am Gerät nur „kein Fehler“ bestätigt);
  `media::MEDIA_TYPES`/`media_type_id`. App-Befehl `printer_info`, Dialog „Druckerinfo …“
  mit Klartext, Kennzeichnung „(unbestätigt)“ und Rohdaten (kopierbar) für den Akkutest.
- Startbildschirm (min. 1,8 s, Klick/Esc schließt) und „Über …“: Entwickler ITSH Neumeier –
  Timo Neumeier, © 2026, MIT, Drittkomponenten, Markenhinweis. `tauri.conf.json`:
  `bundle.publisher`/`copyright` (Datei-Eigenschaften der exe).
- Geprüft: Tests (1 neuer), fmt/clippy, Frontend-Build; GUI unter Xvfb (Dialog ohne Drucker
  → Fehlermeldung, Startbildschirm/Über).

### 2026-10-02 – Claude Code, Rahmenabstände, Inhaltsausrichtung, senkrechtes Lineal (ADR-030)
- `LabelBorder::insets_mm` (oben/unten/links/rechts, sonst `inset_mm`); links/rechts zählen
  ab den Label-Rändern (`ll_render::Insets`, `border_reserve` je Seite).
- `Item::halign`/`valign`; Text: `valign` im fontdue-Layout (`boxed::TextLayout`, ersetzt
  die lange Parameterliste), QR/Barcode/Bild/Symbol: `boxed::align_content` verschiebt die
  Tinte nach dem Drehen an die gewünschte Seite.
- App: Zeile „Inhalt:“ (⇤ ↔ ⇥ ⤒ ↕ ⤓) je Element, die Text-„Ausrichtung“-Auswahl ist darin
  aufgegangen; vier Abstandsfelder beim Rahmen; senkrechtes mm-Lineal links über die volle
  Bandbreite.
- Geprüft: Tests (4 neue), fmt/clippy, Frontend-Build; GUI unter Xvfb (Text rechts/oben,
  Lineal), CLI-Render (QR oben links/unten rechts, Rahmen 3 mm Rand + 2 mm Abstand rechts).

### 2026-10-02 – Claude Code, Fix Labellänge (100 mm fest → 107 mm gedruckt)
- Ursache (Messung Nutzer): Der Drucker-Rand `1B 69 64` (Nachlauf, Standard 28 Punkte
  ≈ 3,95 mm) kommt vor *und* nach dem Label dazu: 100 + 2 × 3,95 ≈ 108 mm.
- Fix `ll_core::print::trim_for_margin`: je Seite höchstens den Nachlauf von beiden leeren
  Enden des gerenderten Labels abschneiden und genau diesen Wert als Rand senden; nie Tinte
  abschneiden (Rand = kleineres leeres Ende). Gedruckte Länge = Vorschau-Länge; der Abstand
  zum Schnitt kommt jetzt aus „Rand links/rechts“. Standard-Nachlauf jetzt 0 (Nutzerwunsch).
- Geprüft: Tests (1 neuer, 2 angepasst). Hardware-Test eingetragen.

### 2026-10-02 – Claude Code, Größe an allen Seiten ändern
- Acht Griffe je Box (n/s/e/w und Ecken); `snap.ts`: `handleEdges`, `resizeRect` (Gegenkante
  fest, Mindestgröße, Umschalt an Ecken = Seitenverhältnis, linke Kante nicht vor 0) und
  `snapResize` für die bewegte Kante (auch links/oben). Nummernschild der Box nach rechts
  versetzt, damit es den Eckgriff nicht verdeckt.
- Geprüft: Frontend-Build, Tests; GUI unter Xvfb (linke und obere Kante gezogen).

### 2026-10-02 – Claude Code, Rand links/rechts
- `Label::padding_start_mm` (fehlt = `padding_mm`); Fluss-Inhalt beginnt nach dem linken
  Rand, `padding_mm` bleibt rechter Rand. Boxen behalten ihre Position (Format unverändert).
- App: Felder „Rand links“/„Rand rechts“; schraffierte Zonen an beiden Label-Enden (Ende =
  gerenderte Länge); Ausrichten links/rechts und Einrasten an den Rändern (rechts nur bei
  fester Länge); Hinweis „Im Rand: Element …“; Ändern des linken Rands verschiebt alle
  Boxen mit.
- Geprüft: Tests (1 neuer), fmt/clippy, Frontend-Build; GUI unter Xvfb (4/3 mm, feste
  Länge 5 → Hinweis, 50 → kein Hinweis).

### 2026-10-02 – Claude Code, Wach halten (Keep-alive)
- Knopf „Wach halten“ in der Geräteleiste (Umschalter, beim Start immer aus): fragt alle
  2 min still den Status ab (`api.queryStatus`), nicht während eines Drucks; Fehler in der
  Statuszeile, letzte Abfrage im Tooltip. Ob das die Auto-Abschaltung verhindert, ist
  unverifiziert (`TODO(verify)` in PROTOCOL.md, Hardware-Test eingetragen).
- Geprüft: Frontend-Build, Tests; GUI unter Xvfb (Umschalter, Fehlermeldung ohne Drucker).

### 2026-10-02 – Claude Code, Fett/kursiv für Textteile (ADR-029)
- `ll_render::richtext` (Parser `**…**`/`__…__`, ungepaarte Marker bleiben Text),
  `FaceSet` (regulär/fett/kursiv/fett-kursiv); Textfunktionen in `boxed` legen Läufe mit
  eigenem Font-Index an (fontdue), Synthese (Schräg/Fett) je Glyphe nach deren Schnitt.
  `ll-core` lädt die drei Zusatzschnitte nur, wenn der Text Marker enthält; der
  Fluss-Schnellpfad gilt nur für Text ohne Marker.
- App: F/K wirken bei Markierung im Textfeld auf den markierten Teil (Marker setzen bzw.
  entfernen), sonst auf das ganze Element; Strg+B/Strg+I im Textfeld; Beschriftungen und
  Verlaufsnamen ohne Marker (`app/src/richtext.ts`).
- Geprüft: Tests (5 neue), fmt/clippy, Frontend-Build; GUI unter Xvfb („Server **42**“,
  „Rack __kalt__“, erneutes Strg+B entfernt die Marker).

### 2026-10-02 – Claude Code, Bandrand, Zeilenabstand, Vorschau-Performance (ADR-028)
- Fix: Abdunklung des Zuschnitt-Rahmens im Bild-Editor lag über dem ganzen Dialog
  (`box-shadow` nicht beschnitten) → `.ie-stage { overflow: hidden }`, Griffe innen.
- Vorschau: `#tape-frame` um die Bühne, Polsterung oben/unten = (Bandbreite − bedruckbar) / 2
  aus der Modelltabelle (`Tape.printable_mm`), grau schraffiert über der Bandfarbe.
  Koordinaten der Bühne unverändert (bedruckbarer Bereich). Bei Mehrband nur außen.
- Text: `line_spacing` (Vielfaches, 0,5–3, fehlt = 1) → fontdue `LayoutSettings::line_height`,
  auch in der Auto-Größe berücksichtigt; Feld „Zeilenabstand“ in der Textkarte.
- Performance (Beispiel `cargo run --release -p ll-core --example bench_preview -- <datei>`):
  schweres Testlabel (12 Texte, 4 Fotos 2400×1800, QR, 24 mm, glatt) ~700 ms → ~75 ms je
  Aktualisierung. Cache gerenderter Bildboxen (Schlüssel: Pfad, Dateigröße/-zeit, Box,
  Korrektur, Bearbeitung; max. 64), Schriften einmal pro Prozess geparst,
  `render_preview` als async-Befehl auf Blocking-Worker (Fenster friert nicht mehr ein),
  Frontend hält höchstens einen Render gleichzeitig. Schnellere PNG-Kompression getestet:
  kaum Gewinn, größere Daten → verworfen.
- Geprüft: Tests (1 neuer), fmt/clippy, Frontend-Build; GUI unter Xvfb (grauer Rand auf
  12 mm, Zeilenabstand 1,8, Bild-Editor-Abdunklung nur im Bild).

### 2026-10-02 – Claude Code, Einfügen per Strg+V, nicht markierbare Oberfläche (ADR-027)
- `ll_core::pasted`: eingefügte Bilder landen in `<Datenordner>/pasted/<FNV-Hash>.<ext>`
  (gleiches Bild = gleiche Datei). App-Befehle `save_pasted_image` (Bilddaten aus dem
  Paste-Ereignis, WebView2) und `paste_clipboard_image` (Rückfall über
  `tauri-plugin-clipboard-manager`, weil WebKitGTK Bilder nicht an die Seite gibt).
- Frontend: Strg+V fügt Bild (Seitenverhältnis übernommen), kopierte Elemente oder Text
  (neues Textelement) ein; Strg+C/X kopiert/schneidet das gewählte Element
  (`application/x-labellab+json`, Text-Rückfall mit Marker `labellab-elements`). Nicht in
  Eingabefeldern und Dialogen. Verstecktes `#paste-catcher` (contenteditable) bekommt bei
  Strg+C/X/V kurz den Fokus, sonst feuert WebKitGTK keine Clipboard-Ereignisse.
- CSS: `body { user-select: none }`, Eingabefelder bleiben markierbar.
- Geprüft: Tests (1 neuer), fmt/clippy, Frontend-Build; GUI unter Xvfb mit `xclip`: PNG und
  Text einfügen, Element kopieren/einfügen, Dreifachklick auf Hinweistext markiert nichts.
  **Nicht geprüft:** Windows (WebView2) — Screenshot mit Win+Umschalt+S und Strg+V testen.

### 2026-10-02 – Claude Code, Bild-Editor (ADR-026)
- `ll_render::image_edit` (`ImageEdit`: Drehung, Spiegeln, Zuschnitt in Bruchteilen,
  `BackgroundRemoval` mit Farbe/Toleranz/zusammenhängend, Halbton Raster/Schwelle) im
  gemeinsamen Renderpfad (`render_image_edited`, `image_in_box_edited`), Feld `edit` am
  Bild-Element in `ll-core`. Reihenfolge: drehen/spiegeln → zuschneiden → Hintergrund →
  auf Weiß → Grau → Helligkeit/Kontrast → Raster/Schwelle.
- App: Befehl `image_editor_source` (Vorschaubild max. 640 px, einmal maskiert, einmal ohne
  Maske für die Farbwahl), Dialog in `app/src/imageEditor.ts`, Knopf „Bild bearbeiten …“ in
  der Bildkarte; Druckvorschau im Dialog über `render_preview` (nur dieses Element).
  Neues Bild wählen setzt die Bearbeitung zurück.
- Geprüft: Tests (4 neue), fmt/clippy (Workspace + App), Frontend-Build; GUI unter Xvfb:
  Hintergrund automatisch entfernt (Schachbrett sichtbar), Zuschnitt per Ecken, Farbe im
  Loch gewählt + „nur zusammenhängend“ aus → Loch transparent, Übernehmen → Vorschau.
- Offen: Druck eines bearbeiteten Bildes auf echtem Band.

### 2026-10-02 – Claude Code, M9 Paket 3a – Arbeitsblätter, Warnung bei ungespeicherten Änderungen (ADR-025)
- `ll_core::document` (`Document`/`Sheet`, Format v3 nur bei >1 Blatt, sonst weiter v2-Label),
  CLI `--template … --sheet <Nr|Name>`, App-Befehle `load_document`/`save_document` (ersetzen
  `load_label`/`save_label`).
- GUI: Reiterleiste, Blatt hinzufügen/umbenennen/löschen, Bandbreite je Blatt; Undo-Verlauf
  wird beim Blattwechsel zurückgesetzt. Ungespeichert = Dokument-JSON ≠ Stand beim letzten
  Öffnen/Speichern/Neu; Abfrage per `ask` (Dialog-Plugin; `window.confirm` ersetzt, auch beim
  Icon-Set-Entfernen). Fenster schließen: `onCloseRequested` (Berechtigungen
  `dialog:allow-ask`, `core:window:allow-destroy`).
- Geprüft: Tests (3 neue Dokument-Tests), fmt/clippy, Frontend-Build; GUI unter Xvfb (Blatt
  „Ports“ anlegen/umbenennen, Wechsel behält Inhalte, „Neu“ fragt bei Änderungen).
  **Nicht geprüft:** Warnung beim Fenster-Schließen (Xvfb ohne Fenstermanager) — am Gerät testen.
- Nächster Schritt: Paket 3b Hochformat, Tabellen, Deko-Rahmen.

### 2026-10-02 – Claude Code, M9 Paket 2b – Druckverlauf
- `ll_core::history` (`record`/`list`/`load`/`preview`; Tests mit eigenem Ordner statt
  Umgebungsvariable, damit parallele Tests sich nicht stören), App-Befehle `history`,
  `record_history` (nach erfolgreichem Druck, Fehler dabei werden verschluckt),
  `load_history`; Dialog „Verlauf …“ mit Vorschau, Datum, Band, Anzahl, „Öffnen“.
- Geprüft: Tests, fmt/clippy, Frontend-Build; GUI unter Xvfb mit eingetragenem Verlaufseintrag
  (Klemmblock 24 mm → Öffnen stellt 24 mm ein). Echter Druck → Eintrag: nur am Gerät prüfbar.
- Nächster Schritt: 2c SL-/Schrumpfschlauch-Bänder (Bandgeometrie `TODO(verify)`), sonst
  Paket 3 (mehrere Labels pro Datei, Hochformat, Tabellen, Deko-Rahmen).

### 2026-10-02 – Claude Code, M9 Paket 2a (Teil 1) – neue Generatoren
- `ll_core::layouts`: `SingleFlag` (Einzelfähnchen: Wickelbereich + ein Fähnchen),
  `TerminalBlock` (Klemmblock/LSA, 1–2 Reihen, zweireihig unten 2i, oben 2i+1 wie im Vorbild),
  `FuseBox` (Sicherungskasten/Verteiler: Modulfelder, Text senkrecht 270° = von unten nach oben,
  optional Hauptschalter-Feld links/rechts). CLI `generate single-flag|terminal-block|fuse-box`.
- Geprüft: Tests (je ein Test pro Generator), CLI-Renders (Klemmblock 6×2 auf 24 mm, Verteiler
  mit HAUPT + F1–F6 auf 12 mm).
- 2a Teil 2 erledigt: „Vorlagen …“-Dialog als Galerie (Kategorien Kabel / Verteiler & Netzwerk,
  Kacheln mit Mini-Zeichnung, Live-Vorschau über `generate_layout` + `render_preview`,
  typische Raster je Vorlage: 12,7 / 15 / 17,5 mm). Unter Xvfb geprüft.
- Stolperstein: Container-Platte voll (`app/src-tauri/target` 14 GB) → alte Build-Ordner
  gelöscht; bei „No space left on device“ zuerst `target/*windows*`, `*/release`, App-`target`.
- Nächster Schritt: Paket 2b „Erstellte Labels“ (zuletzt verwendet mit Vorschaubild,
  Druckverlauf), danach 2c SL-/Schrumpfschlauch-Bänder (Protokoll `TODO(verify)`).

### 2026-10-02 – Nutzer-Test Windows (Stand `main` nach PR itsh-neumeier/labellab#7)
- Nutzer meldet: „alle Funktionen funktionieren perfekt“ (Icon-Sets/Symbolauswahl, Formen,
  Ausrichten/Sperren, `{{datum}}`/`{{zeit}}`, feste Länge, Bildregler, Lineal).
- Arbeitsweise ab jetzt (Nutzerwunsch): kleine Schritte, nach jedem Schritt Commit + Push und
  `PROGRESS.md` aktualisieren, damit eine spätere (kostenlose) Session nahtlos weitermacht.

### 2026-10-02 – Claude Code, M9 Paket 1 – Editor-Grundlagen (ADR-024)
- Anlass: Nutzer schickte Screenshots des Hersteller-Editors (ODT, nicht im Repo; enthält
  persönliche Daten). Daraus Roadmap M9 (3 Pakete) in „Meilensteine“.
- Umgesetzt: Element „Form“ (Linie/Rechteck/abgerundet/Oval, Linienstärke, gefüllt), „Länge
  fest“, `{{datum}}`/`{{zeit}}` (chrono), Bild-Helligkeit/-Kontrast, Sperren, Ausrichten-Knöpfe,
  Seitenverhältnis mit Umschalt, mm-Lineal.
- Geprüft: 141 Tests, fmt/clippy (Linux, Windows-Cross-Check, App), Frontend-Build; GUI unter
  Xvfb (Form hinzufügen → Oval gefüllt, waagerecht mittig = (37,8 − 14,8)/2, Sperren blendet
  Griffe/Ausrichten aus, Lineal deckungsgleich); CLI `render "Geprüft {{datum}} {{zeit}}"`.
- Stolperstein: Mittig-Ausrichten muss die Labellänge **ohne** das Element selbst messen, sonst
  zählt seine alte Position mit.
- Offen: Paket 2 (Vorlagen-Startseite, SL-/Schrumpfschlauch-Bänder, Druckverlauf), Paket 3.

### 2026-10-02 – Claude Code, Icon-Sets, ISO 7010 und IEC 60417 (ADR-023)
- `ll-render::iconset`: Format `.llabel-iconset` (JSON, Kategorien, SVG je Icon, Lizenz/Urheber
  je Icon), Registry (mitgeliefert + importiert), Namen `set:icon`, `normalize_svg` (usvg),
  Halbton „threshold“ für Icon-Sets. Bisherige Symbole = Set `material` mit Kategorien.
- `ll-core::iconsets`: Import/Entfernen im Datenordner, Laden beim Start, `from_dir`
  (SVG-Ordner → Set, Unterordner = Kategorien). CLI `symbols --set/--search`,
  `iconset list|import|remove|create`. App: Symbolauswahl-Dialog (Sets/Kategorien, Suche,
  Kacheln, Import, Entfernen), Symbol-Knopf mit Vorschau.
- ISO 7010: Liste aus de.wikipedia, Grafiken von Commons (nur gemeinfrei/CC0, 1× CC BY-SA
  ausgelassen). Stolperstein: Wikimedia drosselt die geteilte IP der Cloud-Umgebung (HTTP 429
  auf upload.wikimedia.org) → Dateien über einen per SHA-1 gegen Commons geprüften Mirror
  (npm `@iso-safety-signs/assets`), Rest über den neuen Workflow „Icon sets“ auf GitHub.
- IEC 60417 (Nutzerwunsch, Commons-Kategorie, 736 Dateien + 25 nicht einsortierte „Ref-No“-
  Dateien wie 5007 „Ein“): 754 Symbole, 11 Themen per Stichwort (~19 % „Sonstige“), englische
  Namen. ISO-OBP-Link des Nutzers nicht als Quelle genutzt (nicht frei lizenziert).
- Beide Sets gebaut vom neuen Workflow „Icon sets“ auf GitHub (dort keine Drosselung): ISO 7010
  335 Zeichen (M002 CC BY-SA ausgelassen), IEC 60417 754. Programmgröße +~4 MB.
- Geprüft: Tests, fmt/clippy (Linux, Windows-Cross-Check, App), Frontend-Build; CLI-Renders
  (W012, P001, M001, E003, F001, W004 sauber schwarz/weiß); GUI unter Xvfb (Kategorien, Suche
  „elektr“ → W012, Auswahl → Vorschau).
- Hardware offen: Sicherheitszeichen auf Band drucken (Lesbarkeit kleiner Piktogramme auf 12 mm).

### 2026-10-02 – Claude Code, Fix Windows-Paket
- Befund: Im Artefakt „LabelLab-windows-x64-portable“ war `LabelLab.exe` in Wahrheit die CLI —
  `Copy-Item … labellab.exe` überschrieb auf NTFS (ohne Groß-/Kleinschreibung) die GUI.
  Seit ADR-015 betroffen. Fix: CLI wird als `labellab-cli.exe` gepackt (Workflow, LIESMICH,
  README). Stolperstein für künftige Pakete: Dateinamen nie nur in Groß-/Kleinschreibung
  unterscheiden.

### 2026-10-01 – Claude Code, Rahmenstile und Schriftauswahl (ADR-022)
- `ll-render`: `draw_border_styled` mit `BorderStyle` (durchgezogen, gestrichelt, gepunktet,
  doppelt, gestreift), Stärke, Muster, Abstand, frei wählbaren Seiten. `ll-core`:
  `Label::border` (`LabelBorder`, mm), `effective_border()` (altes `frame` = Standardrahmen),
  Fluss-Inhalt und Labelende halten Abstand zum Rahmen. CLI `--border*`.
- App: Bereich „Rahmen“ (ersetzt Checkbox), Boxen werden beim Einstellen nach innen gerückt;
  Schriftauswahl zeigt jede Schrift in sich selbst, mit Suche und Tastatur.
- Geprüft: 126 Tests, fmt/clippy (Linux, Windows-Cross-Check, App), Frontend-Build; CLI-PNGs
  aller Stile, „nur oben“, „oben+unten gestreift“; GUI unter Xvfb (Warnband oben/unten,
  Schriftsuche „serif“ → FreeSerif in Liste und Vorschau). Seitenleiste: WebKitGTK zeigte
  einen waagerechten Scrollbalken (Chromium nicht) → `overflow-x: hidden`.
- Hardware offen: Rahmenmuster auf echtem Band.

### 2026-10-01 – Claude Code, GUI: zuletzt verwendete Labels, Serienübersicht
- PR itsh-neumeier/labellab#3 (M4 + Fehlertexte/Überlauf) nach CI-Grün per Squash gemergt.
- Werkzeugleiste: Auswahl „Zuletzt verwendet …“ (bis 8 Pfade, `localStorage`
  `labellab.recent`, beim Öffnen und Speichern aktualisiert; nur sichtbar, wenn nicht leer).
- „Serie ansehen …“ (nur bei CSV oder Nummerierung): Dialog mit allen Labels der Serie (max.
  100) in Bandfarbe, gerendert über `render_preview` (gleicher Renderpfad), ⚠ bei Überlauf.
- Geprüft: Frontend-Build; unter Xvfb Nummernfolge `Nr {{n:03}}` × 5 → Nr 001…005, mit 60 pt
  orange markierte Box + Hinweis und ⚠ je Label. Nicht automatisiert geprüft: Liste
  „Zuletzt verwendet“ (nativer Dateidialog unter Xvfb nicht bedienbar).

### 2026-10-01 – Claude Code, GUI: übersetzte Fehlertexte, Warnung bei abgeschnittenem Text
- PR itsh-neumeier/labellab#2 (M7) nach CI-Grün per Squash nach `main` gemergt.
- `ll-core`: `CoreError::code()` liefert stabile Fehlercodes (`timeout`, `no_device`,
  `printer_error`, `too_tall`, `file_not_found` …). App-Befehle liefern `{ code, detail }`;
  Frontend zeigt `error.<code>` aus den i18n-Dateien plus Rohtext in Klammern (`errorText`).
- `ll-render`: `text_in_box_checked` meldet abgeschnittene Tinte (Basisglyphe, 1 Punkt Toleranz,
  synthetisches Fett/Kursiv zählt nicht). `render_label_preview` liefert `Preview { png,
  overflowing }`; der Editor markiert betroffene Boxen orange gestrichelt mit Hinweistext.
- Geprüft: Tests, fmt/clippy (Workspace, App), Frontend-Build grün.

### 2026-10-01 – Claude Code, M4 – BlueZ (Linux) und Kopplung aus CLI/GUI (ADR-021)
- `ll-transport`: neues `bluetooth/linux.rs` (`bluer`): `list_devices`, `discover` (8 s Scan),
  `pair` (Agent beantwortet PIN/Bestätigung), `BluetoothTransport` über RFCOMM. Windows:
  `list_unpaired_devices`, `pair` (Custom Pairing, PIN aus `ll-protocol`).
- `ll-protocol`: `BT_SPP_RFCOMM_CHANNEL = 1`, `BT_DEFAULT_PIN = "0000"` (beide `TODO(verify)`).
- `ll-core::device`: plattformübergreifend `list/discover_bluetooth_devices`, `pair_bluetooth`.
  CLI `labellab pair [Gerät]` (ohne Argument: Suche + Liste). App: `discover_bluetooth`,
  `pair_bluetooth`, Dialog „Koppeln …“.
- CI/AGENTS.md: Linux braucht jetzt `libdbus-1-dev` + `pkg-config`.
- Geprüft: 118 Tests, fmt/clippy (Linux, Windows-Cross-Check, App), Frontend-Build grün; Dialog
  unter Xvfb (zeigt im Container erwartungsgemäß D-Bus-Fehler, kein BlueZ). Nicht am Gerät.
- Außerdem PR itsh-neumeier/labellab#2 (M7) CI repariert: rust-cache stellte veraltete
  Pfad-Crates in `app/` wieder her → `cargo clean -p …` vor clippy/Build.

### 2026-10-01 – Claude Code, M7 – Generatoren, Nummernfolgen, Drehung, Fläche (ADR-020)
- PR itsh-neumeier/labellab#1 nach CI-Grün per Squash gemergt (Repo erlaubt keine
  Merge-Commits). Vorher Fix: `--symbol` stand beim Merge in seiner eigenen Konfliktliste →
  clap-Panik beim Start; neuer Test `cli_definition_is_consistent` (`Cli::command().
  debug_assert()`), damit so etwas in CI auffällt.
- `ll-render::Bitmap`: `rotated()`, `fill()`. `ll-core`: `Item::rotation`, `Element::Fill`,
  `series::Numbering`/`Record`/`fill` (`{{n}}`, `{{n:03}}`, `{{a}}`, `{{A}}`), `layouts`
  (Kabelfahne, Kabelwickel, Patchpanel). CLI `generate …`, `--count/--start/--step`.
- App: Elemente „Symbol“ (Liste aus `ll_render::SYMBOL_NAMES`) und „Linie/Fläche“, ⟳ drehen,
  Bereich „Nummerierung“ mit Platzhalter-Chips, Dialog „Assistent …“ (Kabelfahne/-wickel/
  Patchpanel mit Live-Info zu Wickelbereich/Länge).
- Geprüft: 117 Tests grün, fmt/clippy (inkl. Windows-Cross-Check, App) grün; CLI-Renders
  (Patchpanel 8×12 mm = 98 mm, Kabelfahne, Kabelwickel gedreht, `SW-115 DK`); App unter Xvfb:
  Assistent → 24-Port-Patchpanel 305 mm.

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
