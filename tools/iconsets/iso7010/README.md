# ISO-7010-Icon-Set bauen

Erzeugt `crates/ll-render/assets/iconsets/iso7010.llabel-iconset`, das mitgelieferte Icon-Set
„ISO 7010 Sicherheitszeichen“ (ADR-023).

## Quelle und Lizenz

- Liste und deutsche Bezeichnungen: Wikipedia-Artikel
  [ISO 7010 – Liste der Symbole](https://de.wikipedia.org/wiki/ISO_7010#Liste_der_Symbole).
- Grafiken: die dort eingebundenen SVG-Dateien von Wikimedia Commons. Übernommen werden nur
  Dateien, die Commons als **gemeinfrei** („Public domain“) oder **CC0** kennzeichnet. Lizenz,
  Urheber und Commons-Seite stehen je Zeichen im Icon-Set (`license`, `author`, `source`).
  Dateien mit anderer Lizenz (z. B. CC BY-SA) werden ausgelassen.
- Die SVGs werden mit `ll_render::iconset::normalize_svg` bereinigt (Editor-Metadaten entfernt,
  Text in Pfade umgewandelt), die Grafik selbst bleibt unverändert.

## Ablauf

```bash
# 1. Liste, Lizenzen (Commons-API) und SVGs holen
python3 tools/iconsets/iso7010/fetch.py /tmp/iso7010 [--mirror <ordner>]

# 2. Icon-Set bauen
cargo run -p ll-render --example build_iconset -- \
  tools/iconsets/iso7010/set.json /tmp/iso7010/manifest.json \
  crates/ll-render/assets/iconsets/iso7010.llabel-iconset
```

Wikimedia drosselt Anfragen von geteilten IP-Adressen stark (HTTP 429). Dann hilft `--mirror`
mit einer lokalen Kopie der Commons-Dateien, z. B. dem npm-Paket `@iso-safety-signs/assets`
(`npm pack @iso-safety-signs/assets`, entpacken, `--mirror package/assets`). Eine Datei aus dem
Mirror wird **nur** verwendet, wenn ihre SHA-1 mit der von Commons gemeldeten übereinstimmt,
also byte-identisch zum Commons-Original ist. Fehlende Dateien meldet das Skript am Ende; ein
erneuter Lauf lädt nur noch diese nach.

Kategorien (Warn-, Verbots-, Gebots-, Brandschutz-, Rettungszeichen, Kombinationen) kommen aus
den Abschnitten des Wikipedia-Artikels, Namen und Übersetzungen der Kategorien aus `set.json`.
