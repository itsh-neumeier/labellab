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
- M5 (Teil): Rahmen um das ganze Label — `--frame` auf `print` und `render`, gilt für
  Text/QR/Barcode/Bild
- M5 (Teil): EAN-13, EAN-8, UPC-A, Code39, ITF als weitere Barcode-Symbologien — `--barcode-type
  <code128|ean13|ean8|upca|code39|itf>` auf `print`/`render`. Nur PNG-Vorschau geprüft, noch
  nicht auf Band gedruckt.
- M5 (Teil): SVG-Import über denselben `--image`-Pfad (`resvg`/`usvg`/`tiny-skia`, an `.svg`-
  Endung erkannt). Nur PNG-Vorschau geprüft, noch nicht auf Band gedruckt. Symbolbibliothek
  noch offen — letztes fehlendes M5-Teil.

### Behoben
- Cutter schnitt direkt am Ende des gedruckten Inhalts ohne Nachlauf (`margin(0)` war fest
  einprogrammiert). `--margin <dots>` auf `print` macht den Leervorschub vor dem Schnitt
  konfigurierbar, Default jetzt 28 statt 0 Druckpunkte. Noch nicht erneut hardware-getestet.
