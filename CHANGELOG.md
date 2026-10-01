# Changelog

Format nach [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionierung nach [SemVer](https://semver.org/lang/de/).

## [Unreleased]
### Hinzugefügt
- Projektstart: README, MIT-Lizenz, Master-Prompt, Multi-Agent-Struktur, Bluetooth-Diagnose-Skript
- M1: Cargo-Workspace mit `ll-protocol`, `ll-transport`, `ll-render`, `ll-core`, `ll-cli`; CI
  (fmt/clippy/test auf Windows + Linux); `docs/PROTOCOL.md`, `docs/ARCHITECTURE.md`
- M2: Status-Parser, Befehlsaufbau und PackBits-Encoder in `ll-protocol`; Serial-Transport in
  `ll-transport`; CLI `labellab devices` und `labellab status [--device COM<n>] [--json]`
- M4 (Windows-Teil, vorgezogen): natives Bluetooth-RFCOMM über WinRT als Ersatz für den
  unzuverlässigen seriellen BT-SPP-Fallback; `labellab devices` listet gekoppelte
  Bluetooth-Geräte, `labellab status --device <ID> --bt`
  — hardware-verifiziert gegen echten PT-P710BT
- M3: `labellab print "Text" --device <...> [--bt] [--cut] [--copies N]` — erster echter
  Druckjob (Status lesen, Band automatisch erkennen, Platzhalter-Bitmapfont rendern, PackBits,
  an den Drucker senden) — hardware-verifiziert: druckt lesbaren Text auf echtes Band
- M5 (Teil): echte Systemschriften (`fontdue`) ersetzen den M3-Platzhalter-Font;
  `labellab render "Text" -o vorschau.png --width <mm>` für Vorschau ohne Drucker (gleicher
  Renderpfad wie der Druck) — hardware-verifiziert
- M5 (Teil): QR-Codes (`qrcode`-Crate) — `labellab print --qr "..."` und
  `labellab render --qr "..." -o vorschau.png` — hardware-verifiziert (gedruckt + gescannt)
- M5 (Teil): Code128-Barcode (`barcoders`-Crate) — `labellab print --barcode "..."` und
  `labellab render --barcode "..." -o vorschau.png` — gedruckt (sauberes Balkenmuster),
  Scan-Lesbarkeit noch nicht verifiziert
- M5 (Teil): Bildimport (PNG/JPEG/BMP) mit Floyd-Steinberg-Dithering — `labellab print --image
  <datei> [--invert]` und `labellab render --image <datei> -o vorschau.png`
- M4 (Teil): USB-Transport (`nusb`) für Windows und Linux — `labellab devices` listet
  angeschlossene USB-Drucker, `labellab status --usb` und `labellab print ... --usb
  [--device <Modell|VVVV:PPPP|Seriennummer>]`; noch nicht hardware-getestet
- M6 (Teil): Vorlagenformat `.llabel` (JSON) mit mehreren Elementen pro Label (Text, QR,
  Barcode, Bild), Abstand, Rand, Mindestlänge und Rahmen — `labellab print/render --template
  <datei.llabel>`
- M6 (Teil): erste Desktop-Oberfläche (Tauri 2) mit Editor, Live-Vorschau, Geräteauswahl,
  Bandstatus, Drucken, Öffnen/Speichern, Rückgängig/Wiederholen, Deutsch/Englisch
- Editor: Elemente frei als Boxen platzieren und skalieren (Maus, Pfeiltasten, X/Y/Breite/Höhe),
  Einrasten an Bandkanten und anderen Boxen zum bündigen Aneinanderlegen, Duplizieren
- Text: Schriftgröße in pt oder automatisch, mehrzeilig, Ausrichtung links/Mitte/rechts
- Schriftarten aus den installierten Systemschriften, fett und kursiv (fehlende Schnitte werden
  nachgebildet)
- Serien aus CSV: Platzhalter `{{Spalte}}` und `{{#}}`, Vorschau je Datensatz, Druck aller oder
  eines Bereichs (GUI und `labellab print --csv … --rows 1-10`)
- Mehrband-Labels: Label über 2×, 3× oder 4× Band übereinander gestalten, gedruckt als ein
  Streifen pro Band
- Fortlaufender Druck („Fortlaufend“, `--chain`): Serien, Kopien und Streifen in einem Auftrag
  ohne Schnitt dazwischen, optional ein Schnitt am Ende
- Vorschau in allen gängigen Band-/Schriftfarben (schwarz auf weiß/gelb/transparent, weiß auf
  schwarz, rot/blau/gold …), nach „Status lesen“ automatisch passend zum eingelegten Band
- Glatte, hochauflösende Vorschau (umschaltbar auf das exakte Druckraster)
- Gerätesuche zeigt den Bluetooth-Gerätenamen statt „SPP SERVER“, erkennt den Drucker, wählt ihn
  aus und liest den Bandstatus automatisch
- Druckknopf zeigt „Wird gedruckt … n/m“ und ist während der Übertragung gesperrt
- Vorlagen-Assistent und `labellab generate`: Kabelfahne (Wickelbereich π × Durchmesser),
  Kabelwickel (Text wiederholt, optional gedreht), Patchpanel/Port-Labels (festes Raster,
  Nummerierung, Trennstriche)
- Nummernfolgen ohne CSV: `{{n}}`, `{{n:03}}`, `{{A}}`/`{{a}}` mit Start/Schritt/Anzahl
  (`--count/--start/--step`, Bereich „Nummerierung“)
- Elemente drehen (90°-Schritte), neues Element „Linie/Fläche“, Symbol-Element in der Oberfläche
- Symbole aus der mitgelieferten Bibliothek auch in Vorlagen und Serien (`{"type": "symbol"}`)
- Vorlagenformat `.llabel` Version 2 (Boxen, Textgröße/-ausrichtung); Version 1 wird weiter gelesen
- Windows: portable `LabelLab.exe` (ohne Installation startbar) und `labellab.exe` werden bei
  jedem Push automatisch gebaut (GitHub Actions, Artefakt `LabelLab-windows-x64-portable`)
- M5 (Teil): Rahmen um das ganze Label — `--frame` auf `print` und `render`, gilt für
  Text/QR/Barcode/Bild
- M5 (Teil): EAN-13, EAN-8, UPC-A, Code39, ITF als weitere Barcode-Symbologien — `--barcode-type
  <code128|ean13|ean8|upca|code39|itf>` auf `print`/`render`. Nur PNG-Vorschau geprüft, noch
  nicht auf Band gedruckt.
- M5 (Teil): SVG-Import über denselben `--image`-Pfad (`resvg`/`usvg`/`tiny-skia`, an `.svg`-
  Endung erkannt). Nur PNG-Vorschau geprüft, noch nicht auf Band gedruckt.
- M5: Symbolbibliothek — 10 eingebettete Material-Symbols-Icons (Apache-2.0: `network`, `wifi`,
  `power`, `warning`, `arrow-up/-down/-left/-right`, `fire`, `fire-extinguisher`) über
  `--symbol <name>` auf `print`/`render`, `labellab symbols` listet verfügbare Namen. Eigene
  SVGs bleiben unabhängig davon über `--image` nutzbar. **M5 damit code-seitig vollständig**;
  nur PNG-Vorschau geprüft, noch nicht auf Band gedruckt.
- M4: Bluetooth unter Linux (BlueZ) und Kopplung des Druckers direkt aus LabelLab
  (`labellab pair`, in der Oberfläche „Koppeln …“) unter Windows und Linux — noch nicht am
  Gerät getestet
- Oberfläche: verständliche deutsche Fehlermeldungen (z. B. „Der Drucker antwortet nicht …“)
  und Warnung, wenn Text nicht in seine Box passt und abgeschnitten wird

### Behoben
- `labellab` brach nach dem Zusammenführen mit der Symbolbibliothek beim Start ab
  (`--symbol` widersprach sich selbst)
- Vorschau blieb in der Windows-Oberfläche leer
- Cutter schnitt direkt am Ende des gedruckten Inhalts ohne Nachlauf (`margin(0)` war fest
  einprogrammiert). `--margin <dots>` auf `print` macht den Leervorschub vor dem Schnitt
  konfigurierbar, Default jetzt 28 statt 0 Druckpunkte. Noch nicht erneut hardware-getestet.
