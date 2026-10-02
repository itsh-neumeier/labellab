# LabelLab

Schnelle, schlanke Label-Software in Rust für Brother-Labeldrucker der PT-P7xx-Serie (primär **PT-P710BT**).
Druckt direkt über **Bluetooth** oder **USB**, ohne Brother-Druckertreiber, unter **Windows und Linux**.

> Status: in Entwicklung (Vorabversion 0.1). Druck per Bluetooth funktioniert, erste Oberfläche
> mit Editor und Live-Vorschau vorhanden. Details: [`docs/PROGRESS.md`](docs/PROGRESS.md).

## Windows: herunterladen und starten
Jeder Push baut automatisch die portable Windows-Version (GitHub → **Actions** → „Windows build“
→ Lauf öffnen → **Artifacts** → `LabelLab-windows-x64-portable`). Keine Installation nötig:

| Datei | Zweck |
|---|---|
| `LabelLab.exe` | Oberfläche, Doppelklick genügt |
| `labellab-cli.exe` | Kommandozeile (`labellab-cli devices`, `labellab-cli print …`) |

Voraussetzungen: Windows 10/11 (64 Bit) mit WebView2-Laufzeit (bei Windows 11 und aktuellem
Windows 10 vorinstalliert). Den Drucker in den Windows-Bluetooth-Einstellungen oder direkt in LabelLab („Koppeln …“ bzw.
`labellab pair`) koppeln.
Die Dateien sind nicht signiert; Windows SmartScreen kann beim ersten Start warnen
(„Weitere Informationen“ → „Trotzdem ausführen“).

## Geplante Funktionen (v1.0)
- Direktdruck per Bluetooth (Windows: native RFCOMM inkl. Kopplung, ohne virtuelle COM-Ports; Linux: BlueZ) und USB
- Live-Abfrage des eingelegten Bands (Breite, Typ, Farben) mit automatischer Anpassung des Editors
- Label-Editor: Text, Rahmen (verschiedene Stile, Seiten frei wählbar), Barcodes, QR/DataMatrix, Bilder, Symbole, pixelgenaue Vorschau
- Kabelfahnen, Kabelwickel, Patchpanel-Labels, Serien mit Nummerierung und CSV-Import, Kettendruck
- CLI `labellab` für Skripte und Automatisierung
- GUI mit Tauri 2

## Für Mitwirkende und KI-Agenten
- [`AGENTS.md`](AGENTS.md): Regeln für alle KI-Werkzeuge
- [`docs/PROGRESS.md`](docs/PROGRESS.md): aktueller Stand
- [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md): Crate-Aufbau
- [`docs/PROTOCOL.md`](docs/PROTOCOL.md): Druckerprotokoll (verifiziert/dokumentiert/unverifiziert)
- [`docs/MAINTENANCE.md`](docs/MAINTENANCE.md): Wartung und Release

## Build
```bash
cargo build --workspace
cargo test --workspace
cargo run -p ll-cli -- devices
cd app && npm install && npm run tauri dev   # Oberfläche
cd app && npx tauri build                    # portable LabelLab(.exe) für das eigene System
```

## Werkzeuge
- [`tools/bt-diagnose.ps1`](tools/bt-diagnose.ps1): prüft unter Windows die Bluetooth-COM-Ports und fragt den Bandstatus direkt am Drucker ab.

## Hinweis
Dies ist ein unabhängiges Projekt, nicht mit Brother Industries, Ltd. verbunden.
„Brother“ und „P-touch“ sind Marken von Brother Industries, Ltd. Die Modellnamen werden nur zur Beschreibung der Kompatibilität genannt.

## Lizenz
[MIT](LICENSE)
