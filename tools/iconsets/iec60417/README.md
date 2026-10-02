# IEC-60417-Icon-Set bauen

Erzeugt `crates/ll-render/assets/iconsets/iec60417.llabel-iconset` („IEC 60417 Gerätesymbole“,
ADR-023).

- Quelle: alle SVG-Dateien der Wikimedia-Commons-Kategorie
  [IEC 60417 symbols](https://commons.wikimedia.org/wiki/Category:IEC_60417_symbols) (alle
  Seiten der Kategorie, über die Commons-API).
- Lizenz: nur Dateien, die Commons als gemeinfrei oder CC0 kennzeichnet; Lizenz, Urheber und
  Commons-Seite je Symbol im Icon-Set. Die ISO/IEC-Datenbank (OBP) wird **nicht** als Quelle
  genutzt: deren Inhalte sind urheberrechtlich geschützt und nicht frei lizenziert.
- Kategorien: Commons ordnet die Symbole nicht thematisch; `fetch.py` sortiert sie nach
  Stichworten der englischen Beschreibung (`TOPICS`), Rest unter „Sonstige“.

```bash
python3 tools/iconsets/iec60417/fetch.py /tmp/iec60417
cargo run -p ll-render --example build_iconset -- \
  tools/iconsets/iec60417/set.json /tmp/iec60417/manifest.json \
  crates/ll-render/assets/iconsets/iec60417.llabel-iconset
```

Wikimedia drosselt geteilte IP-Adressen stark. Der Workflow „Icon sets“
(`.github/workflows/iconsets.yml`) baut beide Sets auf GitHub und lädt sie als Artefakt hoch.
