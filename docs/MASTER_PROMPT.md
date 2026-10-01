# LabelLab – Master-Prompt

> Diesen Prompt komplett in Claude Code (im geklonten Repo `itsh-neumeier/labellab`) einfügen.
> Er beschreibt Ziel, Architektur, Protokoll, Features und Arbeitsweise. Arbeite die Meilensteine
> der Reihe nach ab und committe nach jedem Meilenstein.

---

## Rolle & Ziel

Du bist ein erfahrener Rust-Entwickler mit Erfahrung in Systemprogrammierung, Bluetooth/USB und
Tauri-Apps. Baue **LabelLab**: eine schnelle, schlanke Open-Source-Alternative zum P-touch Editor
für Brother-Labeldrucker der PT-P7xx-Serie – **ohne Brother-Druckertreiber**. LabelLab spricht den
Drucker direkt über das Brother-Raster-Protokoll an, per **Bluetooth (RFCOMM/SPP)** oder **USB**.

Primäres Zielgerät: **Brother PT-P710BT** („P-touch Cube Plus“).
Weitere Modelle, die über dieselbe Logik laufen sollen (Konfiguration statt Sonderlogik):
PT-P715eBT, PT-E720BT, später PT-P700/PT-P750W/PT-P910BT.

Plattformen: **Windows 10/11** und **Linux** (x86_64). Lizenz: **MIT**.

### Warum (Kontext)
Unter Windows bietet Brother für den PT-P710BT offiziell nur USB an. Bluetooth geht nur über
Umwege: Windows erzeugt beim Koppeln einen „Standardmäßige Seriell-über-Bluetooth-Verbindung“-COM-Port,
und der Treiber muss darauf umgestellt werden. Der P-touch Editor ist zudem träge. LabelLab soll
beides lösen: Bluetooth direkt und ohne Treiber-Gebastel, schnelle UI, druckt in Sekunden.

---

## Nicht verhandelbare Anforderungen

1. **Kein Brother-Treiber nötig.** Kein Windows-Spooler, keine INF-Installation. Rohdaten direkt
   an den Drucker.
2. **Performance:** Kaltstart der GUI < 1 s, Vorschau-Rendering < 16 ms pro Änderung, Druckauftrag
   bei Bluetooth sofort nach Klick, Binary klein (Ziel < 15 MB Installer).
3. **Bandstatus live:** Eingelegtes Band (Breite, Typ, Band-/Schriftfarbe) wird beim Verbinden und
   vor jedem Druck abgefragt. Der Editor passt sich automatisch an die Bandbreite an. Bei leerem Band,
   offener Abdeckung, Überhitzung usw. eine klare deutsche Fehlermeldung.
4. **Saubere Trennung:** Protokoll/Rendering als reine Rust-Bibliotheken (ohne GUI-Abhängigkeiten),
   testbar ohne Hardware.
5. **Sprache der UI:** Deutsch als Standard, i18n-fähig (Englisch als zweite Sprache).
6. **Keine Marken im Produktnamen/Logo.** „Brother“ und „P-touch“ sind Marken von Brother
   Industries. Nur beschreibend nennen („kompatibel mit Brother PT-P710BT“) und Disclaimer in die README.

---

## Architektur (Cargo-Workspace)

```
labellab/
├─ crates/
│  ├─ ll-protocol/    # Befehle, Status-Parser, Raster-Encoder (PackBits), Modelltabelle – no_std-fähig wo sinnvoll
│  ├─ ll-transport/   # Trait `Transport` + Implementierungen: BT-Windows (WinRT RFCOMM), BT-Linux (BlueZ RFCOMM),
│  │                  # Serial/COM (Fallback), USB (nusb/rusb), Mock (für Tests)
│  ├─ ll-render/      # Label-Modell → 1-Bit-Bitmap: Text, Rahmen, Barcodes, QR, Bilder, Symbole
│  ├─ ll-core/        # Gerätemanager, Jobs, Serien/CSV, Vorlagen-Speicherung (JSON), Fehlerbehandlung
│  └─ ll-cli/         # Kommandozeile `labellab` (clap)
├─ app/               # Tauri 2: src-tauri (Rust, nutzt ll-core) + Frontend
├─ docs/              # MASTER_PROMPT, PROGRESS, DECISIONS, MAINTENANCE, PROTOCOL, ARCHITECTURE
├─ tools/             # bt-diagnose.ps1 u. a. Hilfsskripte
├─ AGENTS.md          # einzige Regelquelle für alle KI-Werkzeuge
├─ CLAUDE.md, GEMINI.md, .github/copilot-instructions.md, .cursor/rules/  # verweisen auf AGENTS.md
├─ CHANGELOG.md
└─ .github/workflows/ # CI: fmt, clippy -D warnings, test, Release-Builds Win/Linux
```

### Empfohlene Crates (begründet abweichen ist ok)
- Transport: `windows` (WinRT `Windows.Devices.Bluetooth`, `Windows.Devices.Bluetooth.Rfcomm`,
  `Windows.Networking.Sockets.StreamSocket`, `Windows.Devices.Enumeration` für Discovery/Pairing),
  `bluer` (Linux/BlueZ, RFCOMM), `serialport` (COM-Fallback), `nusb` (USB, pure Rust).
- Rendering: `cosmic-text` oder `fontdue`+`ttf-parser` (Systemschriften, Unicode, Kerning),
  `tiny-skia` (Rahmen/Linien/Antialiasing vor dem Thresholding), `image` (Import PNG/JPG/BMP/SVG via `resvg`),
  `rxing` oder `barcoders`+`qrcode` (Code128, EAN-13, Code39, QR, DataMatrix).
- Allgemein: `thiserror`, `anyhow` (nur in Binaries), `tracing`, `serde`, `csv`, `clap`, `tokio`.
- Frontend: Svelte 5 + TypeScript + Vite (leichtgewichtig). Kein schweres UI-Framework.

### Transport-Trait (Skizze)
```rust
#[async_trait]
pub trait Transport: Send {
    async fn write_all(&mut self, data: &[u8]) -> Result<(), TransportError>;
    async fn read_exact_timeout(&mut self, buf: &mut [u8], timeout: Duration) -> Result<(), TransportError>;
    fn kind(&self) -> TransportKind; // BluetoothRfcomm | Serial | Usb | Mock
    async fn close(&mut self) -> Result<(), TransportError>;
}
```

---

## Druckerprotokoll (Brother P-touch Raster Command Reference)

Grundlage ist Brothers öffentliche „Raster Command Reference“ für PT-P700/P710BT/P750W. **Alle Werte
gegen das offizielle PDF und den echten Drucker verifizieren**, dann in `docs/PROTOCOL.md`
dokumentieren. Bekannte, bereits verifizierte Fakten:

| Fakt | Wert | Quelle |
|---|---|---|
| USB VID/PID PT-P710BT | `04F9:20AF` (P715eBT `20C5`, E720BT `224A`) | Brother-Treiber Setup.ini |
| Series/Model-Code im Status | Series `0x30`, Model `0x76` (P715eBT `0x77`, E720BT `0x81`) | Brother Sprachmonitor-INI |
| Invalidate-Länge | 100 × `0x00` | Brother Sprachmonitor-INI (`CmdNull1=100`) |
| Bluetooth | Classic BT 2.1+EDR, SPP-UUID `00001101-0000-1000-8000-00805F9B34FB`, PIN ggf. `0000` | getestet unter Win 11 |
| Statusabfrage | `00×100, 1B 40, 1B 69 53` → 32 Bytes, Byte0 = `0x80` | getestet: Antwort über COM9 ok |

### Befehle (zu verifizieren)
- Invalidate: `00` × 100
- Initialize: `1B 40`
- Status-Request: `1B 69 53` → 32-Byte-Statusblock:
  Byte 8 = Fehler 1, Byte 9 = Fehler 2, Byte 10 = Bandbreite (mm), Byte 11 = Medientyp,
  Byte 18 = Statustyp (0x00 Antwort, 0x01 Druck fertig, 0x02 Fehler, 0x06 Phasenwechsel …),
  Byte 19 = Phase, Byte 24 = Bandfarbe, Byte 25 = Schriftfarbe.
- Raster-Modus: `1B 69 61 01`
- Druckinformation: `1B 69 7A n1..n10` (Gültigkeitsflags, Medientyp, Breite, Länge, Rasterzeilen LE32, Seite, 0)
- Various Mode: `1B 69 4D n` (Bit 6 = Auto-Cut)
- Advanced Mode: `1B 69 4B n` (Chain-Print aus, Hohe Auflösung, Half-Cut wo unterstützt)
- Rand/Vorschub: `1B 69 64 n1 n2` (Punkte, LE16)
- Kompression: `4D 02` (TIFF/PackBits)
- Rasterzeile: `47 n1 n2 <daten>` (Länge LE16), Leerzeile: `5A`
- Drucken mit Vorschub (letzte Seite): `1A`; Seite ohne Vorschub: `0C`

### Kopf & Bandgeometrie (verifizieren!)
- 180 dpi (optional 180×360 hohe Auflösung), Druckkopf 128 Pins = 16 Bytes pro Rasterzeile.
- Bedruckbare Pins und linker Pin-Offset je Bandbreite (Startwerte, gegen Referenz prüfen):
  3,5 mm: 24 Pins / Offset 52 · 6 mm: 32 / 48 · 9 mm: 50 / 39 · 12 mm: 70 / 29 · 18 mm: 112 / 8 · 24 mm: 128 / 0
- Diese Tabelle als Daten (`models.toml` oder const-Tabelle) ablegen, nicht als verstreute Magic Numbers.

### Ablauf eines Druckjobs
1. Verbinden → Invalidate → Initialize → Status lesen → Band prüfen (Breite passt zum Layout?)
2. Raster-Modus, Various/Advanced Mode, Rand, Druckinfo, Kompression
3. Rasterzeilen senden (gestreamt, PackBits, Leerzeilen als `5A`)
4. `1A` → auf Status „Druck fertig“ bzw. „Fehler“ warten (Timeout konfigurierbar) → Ergebnis an UI

---

## Features Version 1.0

### 1. Geräteverwaltung & Verbindung
- Geräte-Suche: gekoppelte und neue Bluetooth-Geräte (Filter auf Namen `PT-P710BT*` usw.) sowie USB.
- **Windows: Kopplung direkt aus der App** (WinRT `DeviceInformation.Pairing`, PIN `0000` automatisch,
  wenn verlangt), Verbindung per RFCOMM-`StreamSocket` – **ohne virtuelle COM-Ports**.
- Fallback: vorhandene Bluetooth-COM-Ports erkennen (ausgehend vs. eingehend unterscheiden!) und nutzen.
- Linux: BlueZ (`bluer`), Kopplung über BlueZ-Agent, RFCOMM-Socket. USB mit udev-Regel-Hinweis.
- Hinweis in der UI, wenn der Drucker belegt ist (nur eine BT-Verbindung gleichzeitig, z. B. Handy).
- Letztes Gerät merken, Auto-Reconnect, Akkustand/Status anzeigen, soweit der Status ihn liefert.

### 2. Bandstatus
- Beim Verbinden und vor jedem Druck: Bandbreite, Medientyp, Band- und Schriftfarbe auslesen.
- Statusleiste: z. B. „24 mm · laminiert · schwarz auf weiß“.
- Editor stellt die Labelhöhe automatisch auf die Bandbreite um. Warnung, wenn Layout und Band nicht passen.
- Alle Fehlerbits in verständliche deutsche Meldungen übersetzen (kein Band, Band-Ende, Abdeckung offen,
  Überhitzung, falscher Medientyp, Akku schwach …).

### 3. Label-Editor
- Mehrzeiliger Text (bis zur maximalen Zeilenzahl der Bandbreite), Systemschriften, Größe automatisch
  oder fest, fett/kursiv/unterstrichen, Ausrichtung links/mitte/rechts/Blocksatz, vertikaler Text.
- Rahmen (mehrere Stile), Trennlinien, Ränder links/rechts, feste oder automatische Länge.
- Elemente frei positionierbar (Text, Barcode, Bild, Symbol) auf einer Arbeitsfläche in echter
  Bandgeometrie. Zoom, Ausrichtungshilfen, Rückgängig/Wiederholen.
- **Live-Vorschau = exakt das 1-Bit-Raster, das gedruckt wird** (gleicher Renderpfad wie der Druck).
- Vorlagen speichern/laden (eigenes JSON-Format `.llabel`), zuletzt verwendete Labels.

### 4. Barcodes & QR
- Code128, Code39, EAN-13, EAN-8, UPC-A, ITF, QR (Fehlerkorrektur L/M/Q/H), DataMatrix.
- Modulbreite in ganzen Druckpunkten (Lesbarkeit!), Klartext darunter optional, Prüfziffer automatisch.

### 5. Kabel- und Serienlabels
- **Kabelfahne:** gleicher Text zweimal mit Abstand, Wickelbereich frei (Länge nach Kabeldurchmesser).
- **Kabelwickel (Wrap):** Text wiederholt quer über die Länge.
- **Patchpanel/Port-Labels:** n Felder mit festem Raster (z. B. 24 Ports bei Portabstand x mm),
  Trennstriche, automatische Nummerierung.
- **Serien:** Platzhalter `{n}` mit Start/Schritt/Stellen (z. B. `SW-{n:03}`), Buchstabenfolgen,
  **CSV-Import** (Spalten auf Platzhalter abbilden, Vorschau aller Labels).
- Serie als **Kettendruck** (Chain-Print, ein Job, nur am Ende schneiden; Half-Cut wo unterstützt).

### 6. Bilder & Symbole
- Import PNG/JPG/BMP/SVG, Schwellwert oder Dithering (Floyd–Steinberg), Invertieren, Skalieren auf Bandhöhe.
- Mitgelieferte Symbolbibliothek (freie Lizenz, z. B. Material Symbols als SVG): Netzwerk, Strom, Warnung,
  Pfeile, Erdung, Brandschutz usw.

### 7. CLI (`labellab`)
```
labellab devices                         # gefundene Drucker (BT/USB/COM)
labellab status [--device ID]            # Bandstatus als Text oder --json
labellab print "Text" [--font ..] [--size auto] [--cut] [--copies N]
labellab print --template x.llabel --csv daten.csv
labellab print --image logo.png
labellab render x.llabel -o vorschau.png   # ohne Drucker
```
- Exit-Codes sinnvoll, `--json` für Skripte. Gleicher Renderpfad wie die GUI.

---

## Qualität & Tests
- Unit-Tests: PackBits-Encoder, Status-Parser (echte 32-Byte-Dumps als Fixtures), Befehlsaufbau.
- **Golden-Tests:** Label → Raster-Bytes. Byteweiser Vergleich mit gespeicherten Referenzdumps.
- `MockTransport` zeichnet alle gesendeten Bytes auf. Kompletter Druckjob ohne Hardware testbar.
- Hardware-Tests hinter Feature-Flag `hw-tests` (z. B. `cargo test -F hw-tests -- --device COM9`).
- CI (GitHub Actions): `cargo fmt --check`, `clippy -D warnings`, Tests auf `windows-latest` und
  `ubuntu-latest`, Tauri-Release-Builds (MSI/NSIS für Windows, AppImage/.deb für Linux) bei Tags `v*`.
- Logging mit `tracing`, Debug-Option „Rohdaten mitschneiden“ für Fehlerberichte.

---

## Meilensteine (in dieser Reihenfolge, je Meilenstein ein Commit/PR)

1. **M1 – Grundgerüst:** Workspace, Crates, CI, README, `docs/PROTOCOL.md`, Lizenz.
2. **M2 – Protokoll + Status:** `ll-protocol` mit Status-Parser, Befehlsaufbau, PackBits.
   `ll-transport` mit Serial-Transport. CLI `devices` + `status` (Akzeptanz: Bandbreite über COM-Port korrekt).
3. **M3 – Erster Druck:** Textlabel per CLI drucken (Serial + Mock). Golden-Tests.
4. **M4 – Native Bluetooth:** WinRT-RFCOMM (Discovery, Pairing, Connect) und BlueZ. USB über `nusb`.
5. **M5 – Renderer komplett:** Schriften, Rahmen, Barcodes/QR, Bilder, Symbole, `render` nach PNG.
6. **M6 – Tauri-GUI:** Geräteleiste mit Bandstatus, Editor, Live-Vorschau, Vorlagen.
7. **M7 – Kabel/Serien/CSV + Kettendruck.**
8. **M8 – Release:** Installer, Signatur-Hinweise, Doku/Screenshots, v1.0.0.

## Multi-AI-Arbeitsweise & Fortschrittsspeicher

Das Projekt wird mit wechselnden KI-Werkzeugen entwickelt (Claude Code, Codex, Copilot, Gemini, Cursor).
Der Stand lebt **in Dateien, nicht im Chatverlauf**:

| Datei | Zweck | Wann pflegen |
|---|---|---|
| `AGENTS.md` | Regeln, Build-Befehle, Pflichten (einzige Quelle) | bei Regeländerungen |
| `docs/PROGRESS.md` | Meilensteine, „In Arbeit“, nächste Schritte, offene Hardware-Tests, Session-Log | **jede Session** |
| `docs/DECISIONS.md` | Architekturentscheidungen (ADRs) | bei jeder Grundsatzentscheidung |
| `docs/MAINTENANCE.md` | Checklisten: Abhängigkeiten, neues Modell, Fehleranalyse, Release | bei Prozessänderungen |
| `docs/PROTOCOL.md` | Protokollwerte mit Quelle und Status (verifiziert/unverifiziert) | bei neuen Erkenntnissen |
| `CHANGELOG.md` | Nutzerrelevante Änderungen | bei jedem Feature/Fix |

Pflichten:
- **Start:** `AGENTS.md`, `docs/PROGRESS.md`, `docs/DECISIONS.md` lesen, eigene Aufgabe unter „In Arbeit“ eintragen.
- **Ende:** `PROGRESS.md` (Checkboxen, nächste Schritte, Session-Log mit Datum und Werkzeug), ggf. ADR,
  `PROTOCOL.md`, `CHANGELOG.md` aktualisieren, committen.
- Jeder Meilenstein endet mit einem Stand, an dem ein **anderer Agent ohne Kontext** weitermachen kann.

## Arbeitsweise
- Vor jedem Meilenstein kurz den Plan nennen, danach Ergebnis + offene Punkte.
- Unklare Protokolldetails **nicht raten**: als `TODO(verify)` markieren und einen Hardware-Test
  vorschlagen, den ich mit dem echten PT-P710BT ausführe.
- Code und Bezeichner auf Englisch, Doku/UI-Texte auf Deutsch (mit i18n-Schlüsseln).
- Keine `unwrap()` in Bibliothekscode. Fehler typisiert mit `thiserror`.
