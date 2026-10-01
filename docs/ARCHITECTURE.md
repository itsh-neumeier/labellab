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

## Stand nach M1

- Crates angelegt, kompilieren, `cargo fmt`/`clippy -D warnings`/`test` grün.
- `ll-protocol`: Modelltabelle (PT-P710BT/P715eBT/E720BT) mit Status-Byte-Lookup; Status-Parser,
  Befehlsaufbau und PackBits folgen in M2.
- `ll-transport`: `Transport`-Trait + `MockTransport`; Serial-Transport folgt in M2, native
  Bluetooth/USB in M4.
- `ll-render`: `Bitmap`-Typ (1-Bit-Raster); Text-/Barcode-/Bild-Rendering folgt in M5.
- `ll-core`: gemeinsamer `CoreError`; Gerätemanager und Jobs folgen in M2/M3.
- `ll-cli`: Subcommands `devices`/`status`/`print`/`render` als Gerüst (clap), geben aktuell
  „noch nicht implementiert“ aus.
- CI (`.github/workflows/ci.yml`): fmt-check, clippy -D warnings, test auf windows-latest und
  ubuntu-latest.
