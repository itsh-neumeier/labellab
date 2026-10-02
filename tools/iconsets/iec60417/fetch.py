#!/usr/bin/env python3
"""Fetches all SVGs of the Wikimedia Commons category "IEC 60417 symbols"
(all pages) and writes a manifest for
`cargo run -p ll-render --example build_iconset`.

Only files that Commons marks as public domain (or CC0) are taken; the
license, author and source page of every symbol end up in the icon set.
IEC 60417 has no categories of its own on Commons, so symbols are sorted
into topic groups by keywords in their English description (see TOPICS).

    python3 tools/iconsets/iec60417/fetch.py <work-dir> [--mirror <dir>]

Creates <work-dir>/svg/*.svg and <work-dir>/manifest.json.
"""

import json
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
import commons  # noqa: E402

CATEGORY = "IEC 60417 symbols"

# Topic -> keywords, matched as whole words in the lower-case description
# (`word*` = prefix). First topic that matches wins: specific before general.
TOPICS = [
    ("safety", ["danger", "caution", "warning", "hazard", "dangerous", "laser", "radiation",
                "ionizing", "hot surface", "high voltage", "electrostatic", "esd", "biological"]),
    ("medical", ["medical", "patient", "defibrillat*", "type b", "type bf", "type cf", "x-ray",
                 "applied part", "anaesthe*", "anesthe*"]),
    ("power", ["on", "off", "standby", "stand-by", "power", "start", "stop", "emergency stop",
               "on/off", "push-push"]),
    ("electrical", ["earth", "ground", "protective", "class ii", "class iii", "voltage", "current",
                    "alternating", "direct current", "fuse", "battery", "accumulator", "plug",
                    "socket", "transformer", "insulat*", "equipotential", "frame", "chassis",
                    "polarity", "positive", "negative", "rectifier", "supply", "mains", "dc", "ac",
                    "charging", "charge"]),
    ("audio_video", ["audio", "video", "loudspeaker", "microphone", "headphone", "sound", "volume",
                     "television", "radio", "record", "recording", "play", "playback", "pause",
                     "tape", "disc", "cassette", "camera", "picture", "image", "colour", "color",
                     "contrast", "brightness", "tuning", "channel", "stereo", "mono", "speaker"]),
    ("data", ["data", "computer", "network", "input", "output", "printer", "keyboard", "usb",
              "interface", "telephone", "signal", "transmission", "antenna", "wireless",
              "remote", "monitor", "display", "terminal", "memory", "connection", "connector"]),
    ("temperature", ["temperature", "heat", "heating", "cooling", "cold", "freez*", "humidity",
                     "dry", "drying", "steam", "water", "air", "fan", "ventilat*", "wash"]),
    ("light", ["lamp", "light", "lighting", "illuminat*", "flash", "beam"]),
    ("controls", ["increase", "decrease", "adjust", "variab*", "setting", "reset", "lock",
                  "unlock", "manual", "automatic", "programme", "program", "mode", "timer", "time",
                  "clock", "direction", "rotation", "speed", "move", "button"]),
]


def topic(description: str) -> str:
    text = description.lower()
    for name, words in TOPICS:
        if any(re.search(r"\b" + (re.escape(w[:-1]) if w.endswith("*") else re.escape(w) + r"\b"), text)
               for w in words):
            return name
    return "other"


def ref_no(file: str) -> str:
    m = re.search(r"Ref[- ]?No\.?\s*([0-9A-Za-z\-]+)", file)
    return m.group(1) if m else re.sub(r"\.svg$", "", file).replace("IEC 60417 - ", "")


def title(meta: dict, code: str) -> str:
    """A short English title from the Commons description."""
    text = meta.get("description") or meta.get("object_name") or ""
    text = re.sub(r"(?i)^.*?ref[- ]?no\.?\s*[0-9a-z\-]+\s*[:\-–]?\s*", "", text)
    text = re.sub(r"(?i)^iec 60417\s*[:\-–]?\s*", "", text).strip(" .;:-–")
    if len(text) > 90:
        text = text[:87].rsplit(" ", 1)[0] + " …"
    return text or f"Ref-No {code}"


def main() -> None:
    work, mirror = commons.parse_args(sys.argv[1:])
    files = commons.cached(work / "files.json", lambda: commons.category_files(CATEGORY))
    files = [f for f in files if f.lower().endswith(".svg")]
    print(f"{len(files)} SVG files in category", file=sys.stderr)
    info = commons.cached(work / "commons.json", lambda: commons.file_info(files))
    mirrored = commons.mirror_index(mirror)

    manifest, skipped = [], []
    for file in sorted(files):
        meta = info.get(file, {})
        code = ref_no(file)
        if not meta.get("url") or not commons.license_ok(meta.get("license", "")):
            skipped.append((code, meta.get("license") or "missing"))
            continue
        target = work / "svg" / (re.sub(r"[^0-9A-Za-z\-]", "_", code) + ".svg")
        error = commons.fetch_verified(target, meta, mirrored, file)
        if error:
            skipped.append((code, error))
            continue
        name = title(meta, code)
        manifest.append({"code": code, "name": name, "category": topic(name),
                         "svg": str(target.relative_to(work)), "source": meta["page"],
                         "license": meta["license"], "author": meta["artist"]})
    (work / "manifest.json").write_text(json.dumps(manifest, ensure_ascii=False, indent=1), encoding="utf-8")
    print(f"{len(manifest)} icons written, {len(skipped)} skipped: {skipped}", file=sys.stderr)


if __name__ == "__main__":
    main()
