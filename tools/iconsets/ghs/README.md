# GHS-Icon-Set bauen

Erzeugt `crates/ll-render/assets/iconsets/ghs.llabel-iconset` („GHS-Gefahrenpiktogramme“).

- Quelle: die neun Piktogramme GHS01–GHS09 von Wikimedia Commons (`GHS-pictogram-*.svg`), wie im
  Wikipedia-Artikel „Global harmonisiertes System zur Einstufung und Kennzeichnung von
  Chemikalien“ gezeigt.
- Lizenz: nur Dateien, die Commons als gemeinfrei oder CC0 kennzeichnet; Lizenz, Urheber und
  Commons-Seite je Piktogramm im Icon-Set.
- Einfarbiger Druck: rote Raute und schwarzes Symbol werden per Schwelle schwarz.

```bash
python3 tools/iconsets/ghs/fetch.py /tmp/ghs
cargo run -p ll-render --example build_iconset -- \
  tools/iconsets/ghs/set.json /tmp/ghs/manifest.json \
  crates/ll-render/assets/iconsets/ghs.llabel-iconset
```

Wikimedia drosselt geteilte IP-Adressen; der Workflow „Icon sets“ baut das Set auf GitHub.
