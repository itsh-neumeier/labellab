# Architektur

Cargo-Workspace, fünf Crates plus Tauri-App. Details und Begründungen: `docs/DECISIONS.md`.

```
labellab/
├─ crates/
│  ├─ ll-protocol/   # Befehle, Status-Parser, PackBits, Modelltabelle (keine GUI-/Transport-Abhängigkeiten)
│  ├─ ll-transport/  # Transport-Trait + Bluetooth RFCOMM / Serial / USB / Mock
│  ├─ ll-render/      # Label-Modell → 1-Bit-Bitmap (gleicher Pfad für Vorschau und Druck)
│  ├─ ll-core/        # Gerätemanager, Jobs, Serien/CSV, Vorlagen, gemeinsamer Fehlertyp
│  └─ ll-cli/         # Kommandozeile `labellab` (clap), nutzt nur ll-core
├─ app/               # Tauri 2 (src-tauri nutzt ll-core) + Frontend (ab M6)
├─ docs/              # MASTER_PROMPT, PROGRESS, DECISIONS, MAINTENANCE, PROTOCOL, ARCHITECTURE
└─ tools/             # Hilfsskripte (z. B. bt-diagnose.ps1)
```

## Abhängigkeitsrichtung

```
ll-cli ──> ll-core ──> ll-protocol
                   └─> ll-transport
                   └─> ll-render
```

`ll-protocol`, `ll-transport`, `ll-render` sind voneinander unabhängig und ohne Hardware testbar
(`ll-transport::mock::MockTransport`). `ll-core` verdrahtet sie zu Gerätesuche, Statusabfrage und
Druckjobs. Die GUI (`app/`) ruft ausschließlich `ll-core` auf — derselbe Code wie die CLI.

## Fehlerbehandlung

Jedes `ll-*`-Bibliotheks-Crate hat einen eigenen `thiserror`-Fehlertyp (`ProtocolError`,
`TransportError`, `RenderError`). `ll-core::CoreError` bündelt sie mit `#[from]`. `anyhow` nur in
`ll-cli` und später im Tauri-Binary. Kein `unwrap()`/`expect()` in Bibliothekscode außer in Tests.

## Module (Stand 2026-10-01)

| Crate / Ordner | Wichtige Module |
|---|---|
| `ll-protocol` | `command` (Befehle inkl. `0C`/`1A`), `status` (32-Byte-Status), `media` (Band-/Schriftfarben), `model` (Modell-/Bandtabelle, `mm_to_dots`, `pt_to_dots`, 180 dpi), `packbits` |
| `ll-transport` | `Transport`-Trait, `bluetooth` (WinRT-RFCOMM, Gerätename), `serial`, `usb` (`nusb`), `mock` |
| `ll-render` | `Bitmap` (inkl. `blit`), `text` (Fluss-Text), `boxed` (Element in Box: Text mehrzeilig/Größe/Ausrichtung, QR, Barcode, Bild), `fonts` (Systemschriften via fontdb, fett/kursiv, Ersatz), `barcode` (QR), `linear_barcode`, `picture` (PNG/JPEG/BMP/SVG + Dithering), `frame`, `png` (PNG, transparente Maske) |
| `ll-core` | `label` (`Label`/`Item`/`Rect`, `.llabel` v2, `Canvas` für Mehrband/Vorschau-Skalierung, `render_label`, `render_label_pages`, `render_label_preview`, `resolved_rects`), `print` (`print_labels`: Einzelaufträge oder Kettendruck, Fortschritt), `series` (CSV, Platzhalter, Nummernfolgen), `layouts` (Kabelfahne, Kabelwickel, Patchpanel), `device` (`Connection`, USB/BT/seriell, Modell aus Gerätename) |
| `ll-cli` | `labellab devices/status/print/render/generate/symbols` (`--usb`, `--bt`, `--template`, `--csv`, `--rows`, `--count`, `--chain`) |
| `app/` | Tauri 2, eigener Cargo-Workspace (`app/src-tauri`, Befehle in `lib.rs`), Frontend Vite + TypeScript (`main.ts` Editor, `snap.ts` Einrasten, `tapes.ts` Bandfarben, `api.ts`, `i18n/` de/en) |

Renderpfad: `Label` → `compose` (Fluss-Layout + Boxen auf einer `Canvas`) → `Bitmap`. Druck nutzt
`render_label_pages` (Mehrband in Streifen geschnitten), Vorschau `render_label_preview` (gleicher
Code, optional höhere Auflösung). CI: Job `test` (Workspace), Job `app` (GUI), Workflow
„Windows build“ (portable `.exe`).
