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
