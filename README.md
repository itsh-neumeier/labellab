# LabelLab

Schnelle, schlanke Label-Software in Rust für Brother-Labeldrucker der PT-P7xx-Serie (primär **PT-P710BT**).
Druckt direkt über **Bluetooth** oder **USB**, ohne Brother-Druckertreiber, unter **Windows und Linux**.

> Status: Planung. Die Umsetzung folgt dem [Master-Prompt](docs/MASTER_PROMPT.md).

## Geplante Funktionen (v1.0)
- Direktdruck per Bluetooth (Windows: native RFCOMM inkl. Kopplung, ohne virtuelle COM-Ports; Linux: BlueZ) und USB
- Live-Abfrage des eingelegten Bands (Breite, Typ, Farben) mit automatischer Anpassung des Editors
- Label-Editor: Text, Rahmen, Barcodes, QR/DataMatrix, Bilder, Symbole, pixelgenaue Vorschau
- Kabelfahnen, Kabelwickel, Patchpanel-Labels, Serien mit Nummerierung und CSV-Import, Kettendruck
- CLI `labellab` für Skripte und Automatisierung
- GUI mit Tauri 2

## Für Mitwirkende und KI-Agenten
- [`AGENTS.md`](AGENTS.md): Regeln für alle KI-Werkzeuge
- [`docs/PROGRESS.md`](docs/PROGRESS.md): aktueller Stand
- [`docs/MAINTENANCE.md`](docs/MAINTENANCE.md): Wartung und Release

## Werkzeuge
- [`tools/bt-diagnose.ps1`](tools/bt-diagnose.ps1): prüft unter Windows die Bluetooth-COM-Ports und fragt den Bandstatus direkt am Drucker ab.

## Hinweis
Dies ist ein unabhängiges Projekt, nicht mit Brother Industries, Ltd. verbunden.
„Brother“ und „P-touch“ sind Marken von Brother Industries, Ltd. Die Modellnamen werden nur zur Beschreibung der Kompatibilität genannt.

## Lizenz
[MIT](LICENSE)
