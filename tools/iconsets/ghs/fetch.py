#!/usr/bin/env python3
"""Fetches the nine GHS hazard pictograms (GHS01-GHS09) from Wikimedia
Commons, as listed in the Wikipedia article "Global harmonisiertes System
zur Einstufung und Kennzeichnung von Chemikalien", and writes a manifest for
`cargo run -p ll-render --example build_iconset`.

Only files that Commons marks as public domain (or CC0) are taken; the
license, author and source page of every pictogram end up in the icon set.

    python3 tools/iconsets/ghs/fetch.py <work-dir> [--mirror <dir>]

Creates <work-dir>/svg/*.svg and <work-dir>/manifest.json.
"""

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
import commons  # noqa: E402

# Code, Commons file, German name (as in the Wikipedia article).
PICTOGRAMS = [
    ("GHS01", "GHS-pictogram-explos.svg", "Explodierende Bombe"),
    ("GHS02", "GHS-pictogram-flamme.svg", "Flamme"),
    ("GHS03", "GHS-pictogram-rondflam.svg", "Flamme über einem Kreis"),
    ("GHS04", "GHS-pictogram-bottle.svg", "Gasflasche"),
    ("GHS05", "GHS-pictogram-acid.svg", "Ätzwirkung"),
    ("GHS06", "GHS-pictogram-skull.svg", "Totenkopf mit gekreuzten Knochen"),
    ("GHS07", "GHS-pictogram-exclam.svg", "Ausrufezeichen"),
    ("GHS08", "GHS-pictogram-silhouette.svg", "Gesundheitsgefahr"),
    ("GHS09", "GHS-pictogram-pollu.svg", "Umwelt"),
]


def main() -> None:
    work, mirror = commons.parse_args(sys.argv[1:])
    files = [f for _, f, _ in PICTOGRAMS]
    info = commons.cached(work / "commons.json", lambda: commons.file_info(files))
    mirrored = commons.mirror_index(mirror)
    manifest, skipped = [], []
    for code, file, name in PICTOGRAMS:
        meta = info.get(file, {})
        license_name = meta.get("license", "")
        if not meta.get("url") or not commons.license_ok(license_name):
            skipped.append((code, license_name or "missing"))
            continue
        target = work / "svg" / f"{code}.svg"
        error = commons.fetch_verified(target, meta, mirrored, file)
        if error:
            skipped.append((code, error))
            continue
        manifest.append({"code": code, "name": f"{code} {name}", "category": "ghs", "file": file,
                         "svg": str(target.relative_to(work)), "source": meta["page"],
                         "license": license_name, "author": meta["artist"]})
    (work / "manifest.json").write_text(json.dumps(manifest, ensure_ascii=False, indent=1), encoding="utf-8")
    print(f"{len(manifest)} pictograms written, {len(skipped)} skipped: {skipped}", file=sys.stderr)
    if skipped:
        sys.exit(1)


if __name__ == "__main__":
    main()
