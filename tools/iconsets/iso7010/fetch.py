#!/usr/bin/env python3
"""Fetches the ISO 7010 safety sign SVGs listed on the German Wikipedia
article "ISO 7010" from Wikimedia Commons and writes a manifest for
`cargo run -p ll-render --example build_iconset`.

Only files that Commons marks as public domain (or CC0) are taken; the
license, author and source page of every icon end up in the manifest and
in the generated `.llabel-iconset` file.

Usage (network access to Wikimedia required, be polite: it is slow on
purpose because Wikimedia rate-limits shared IPs):

    python3 tools/iconsets/iso7010/fetch.py <work-dir> [--mirror <dir>]

`--mirror` points to a local copy of the Commons files (any folder tree
containing e.g. `ISO_7010_W001.svg`, such as the npm package
`@iso-safety-signs/assets`). A mirrored file is only used if its SHA-1
equals the one Commons reports for that file, i.e. it is byte-identical to
the Commons original; otherwise the file is downloaded from Commons.

Creates <work-dir>/svg/*.svg and <work-dir>/manifest.json.
"""

import json
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
import commons  # noqa: E402

WIKI_RAW = "https://de.wikipedia.org/w/index.php?title=ISO_7010&action=raw"

# Section heading on the Wikipedia page -> iconset category id.
SECTIONS = {
    "Warnzeichen": "warning",
    "Verbotszeichen": "prohibition",
    "Gebotszeichen": "mandatory",
    "Brandschutzzeichen": "fire",
    "Rettungszeichen": "escape",
    "Fluchtwegzeichen mit Richtungspfeil": "escape",
    "Fluchtwegzeichen mit Text": "escape",
    "Rothalbmond-Variante": "escape",
    # "Zusatzzeichen und Kombinationen von Zeichen" only shows layout
    # examples, not signs: left out.
}


def plain(text: str) -> str:
    """Strips wiki markup ([[a|b]] -> b, [[a]] -> a, '' -> )."""
    text = re.sub(r"\[\[(?:[^|\]]*\|)?([^\]]*)\]\]", r"\1", text)
    text = re.sub(r"<[^>]+>", "", text)
    return text.replace("''", "").strip()


def parse_entries(wikitext: str) -> list[dict]:
    entries, category, in_gallery = [], None, False
    for line in wikitext.splitlines():
        heading = re.match(r"^=+\s*(.*?)\s*=+$", line)
        if heading:
            category = SECTIONS.get(heading.group(1))
            continue
        if line.startswith("<gallery"):
            in_gallery = True
            continue
        if line.startswith("</gallery"):
            in_gallery = False
            continue
        if not (in_gallery and category):
            continue
        m = re.match(r"^(?:Datei:|File:)?(ISO[ _]7010[^|]*\.svg)\|(.*)$", line)
        if not m:
            continue
        file = m.group(1).replace("_", " ")
        caption = plain(m.group(2))
        code_match = re.match(r"^([A-Z]\d{3}[^:]*):\s*(.*)$", caption)
        if code_match:
            code, name = code_match.group(1).strip(), code_match.group(2).strip()
        else:
            code = Path(file).stem.replace("ISO 7010 ", "")
            name = caption
        entries.append({"file": file, "code": code, "name": name or code, "category": category})
    # Keep the first occurrence of every file.
    seen, unique = set(), []
    for e in entries:
        if e["file"] not in seen:
            seen.add(e["file"])
            unique.append(e)
    return unique


def main() -> None:
    work, mirror = commons.parse_args(sys.argv[1:])
    wiki_file = work / "page.wiki"
    if not wiki_file.exists():
        wiki_file.write_bytes(commons.get(WIKI_RAW))
    entries = parse_entries(wiki_file.read_text(encoding="utf-8"))
    print(f"{len(entries)} signs listed", file=sys.stderr)

    info = commons.cached(work / "commons.json", lambda: commons.file_info([e["file"] for e in entries]))
    mirrored = commons.mirror_index(mirror)
    manifest, skipped = [], []
    for e in entries:
        meta = info.get(e["file"], {})
        license_name = meta.get("license", "")
        if not meta.get("url") or not commons.license_ok(license_name):
            skipped.append((e["code"], license_name or "missing"))
            continue
        target = work / "svg" / (re.sub(r"[^0-9A-Za-z\-]", "_", e["code"]) + ".svg")
        error = commons.fetch_verified(target, meta, mirrored, e["file"])
        if error:
            skipped.append((e["code"], error))
            continue
        manifest.append({**e, "svg": str(target.relative_to(work)), "source": meta["page"],
                         "license": license_name, "author": meta["artist"]})
    (work / "manifest.json").write_text(json.dumps(manifest, ensure_ascii=False, indent=1), encoding="utf-8")
    print(f"{len(manifest)} icons written, {len(skipped)} skipped: {skipped}", file=sys.stderr)


if __name__ == "__main__":
    main()
