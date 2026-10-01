# Changelog

Format nach [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionierung nach [SemVer](https://semver.org/lang/de/).

## [Unreleased]
### Hinzugefügt
- Projektstart: README, MIT-Lizenz, Master-Prompt, Multi-Agent-Struktur, Bluetooth-Diagnose-Skript
- M1: Cargo-Workspace mit `ll-protocol`, `ll-transport`, `ll-render`, `ll-core`, `ll-cli`; CI
  (fmt/clippy/test auf Windows + Linux); `docs/PROTOCOL.md`, `docs/ARCHITECTURE.md`
- M2: Status-Parser, Befehlsaufbau und PackBits-Encoder in `ll-protocol`; Serial-Transport in
  `ll-transport`; CLI `labellab devices` und `labellab status [--device COM<n>] [--json]`
