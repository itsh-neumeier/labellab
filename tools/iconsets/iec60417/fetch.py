#!/usr/bin/env python3
"""Fetches all SVGs of the Wikimedia Commons category "IEC 60417 symbols"
(all pages), plus files named "IEC 60417 - Ref-No …" that are missing from
the category (e.g. 5007 "On"), and writes a manifest for
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
FILE_PREFIX = "IEC 60417 - Ref-No"

# Topic -> keywords, matched as whole words in the lower-case description
# (`word*` = prefix). First topic that matches wins: specific before general.
TOPICS = [
    ("safety", ["danger*", "caution", "warning", "hazard*", "laser*", "radiation", "ionizing",
                "hot surface", "high voltage", "electrostatic", "esd", "biological", "explos*",
                "flammable", "toxic", "do not", "prohibit*"]),
    ("medical", ["medical", "patient*", "defibrillat*", "type b", "type bf", "type cf", "x-ray",
                 "applied part*", "anaesthe*", "anesthe*", "radiograph*", "radiolog*", "film*",
                 "exposure", "dose", "tomograph*", "ultrasound", "dental", "breath*", "oxygen",
                 "infusion", "blood", "catheter", "nurse", "bed", "fluoroscop*", "collimat*",
                 "tube", "focus", "mammograph*", "stretcher", "body", "lung*", "focal",
                 "radioscop*", "scatter*", "crani*", "angiograph*", "urolog*", "skull"]),
    ("power", ["on", "off", "standby", "stand-by", "power", "start*", "stop", "on/off",
               "push-push", "emergency"]),
    ("household", ["wash*", "clean*", "iron*", "dish*", "laundry", "vacuum", "cook*", "oven",
                   "grill*", "microwave", "refrigerat*", "spin*", "rinse", "bleach", "carpet",
                   "pile", "fabric*", "textile*", "kitchen", "food", "defrost*", "dryer",
                   "tumble", "hob", "hair", "shav*", "brush*", "cleaning", "detergent",
                   "garment*", "floor", "suction", "soiled", "delicate", "regenerat*", "softener", "dust*", "water", "steam*", "boil*", "baking",
                   "toast*", "freez*", "frozen", "ice", "drinking", "coffee", "tea"]),
    ("temperature", ["temperature", "heat*", "cool*", "cold", "humidity", "dry", "drying",
                     "air", "fan", "ventilat*", "thermo*", "climat*", "weather*"]),
    ("data", ["data", "computer*", "network*", "input", "output", "printer*", "keyboard*",
              "usb", "interface", "telephon*", "phone", "call*", "dial*", "ring*", "message*",
              "fax", "signal*", "transmission", "transmit*", "receiv*", "antenna*", "wireless",
              "remote", "terminal", "memory", "store", "storage", "copy*", "print*", "scan*",
              "document*", "mail", "line", "listen", "speech", "handset", "modem", "bus",
              "port", "serial", "parallel", "digital", "analog*", "communicat*", "information",
              "satellite", "broadcast*", "frame*", "multiframe", "text", "heading",
              "error*", "alignment"]),
    ("audio_video", ["audio*", "video*", "loudspeaker*", "microphone*", "headphone*", "sound*",
                     "volume", "television", "radio", "record*", "play*", "pause", "rewind",
                     "fast forward", "eject", "skip", "track*", "tape*", "disc*", "cassette*",
                     "camera*", "picture*", "image*", "colour*", "color*", "contrast",
                     "brightness", "tuning", "tuner", "channel*", "stereo*", "mono*",
                     "speaker*", "linearity", "deflect*", "synchron*", "filter*", "pulse*",
                     "screen*", "display*", "monitor*", "projector*", "slide*", "film",
                     "zoom*", "lens*", "noise", "echo", "balance", "bass", "treble", "tone*",
                     "frame by frame", "still picture", "scene*", "subtitle*", "teletext",
                     "vision", "visual", "hearing", "ear", "music*", "mute", "dolby",
                     "vertical", "horizontal", "amplif*", "bell", "earphone*", "headset*",
                     "speak*", "conference", "dubbing", "hue", "crispener", "mut*", "squelch",
                     "dipole", "hydrophone", "transducer*"]),
    ("electrical", ["earth*", "ground*", "protect*", "class ii", "class iii", "voltage", "current",
                    "alternating", "direct current", "fuse*", "batter*", "accumulator*", "plug*",
                    "socket*", "transformer*", "insulat*", "equipotential*", "frame or chassis",
                    "chassis", "polarity", "positive", "negative", "rectifier*", "supply",
                    "mains", "dc", "ac", "charg*", "electric*", "energy", "generator*", "motor*",
                    "switch*", "contact*", "circuit*", "connect*", "cable*", "wire*",
                    "conductor*", "resist*", "capacit*", "induct*", "frequency", "phase",
                    "converter*", "inverter*", "solar", "photovoltaic", "welding", "electrode*"]),
    ("light", ["lamp*", "light*", "illuminat*", "flash*", "beam*", "bulb*", "sun", "dimm*",
               "luminous", "glare"]),
    ("controls", ["increase", "decrease", "adjust*", "variab*", "setting*", "reset", "lock*",
                  "unlock*", "manual*", "automatic*", "programme*", "program*", "mode*",
                  "timer*", "time", "clock*", "direction*", "rotation", "rotat*", "speed*",
                  "move*", "movement", "button*", "action", "effect", "limit*", "control*",
                  "select*", "position*", "up", "down", "left", "right", "forward", "back*",
                  "open*", "close*", "release", "hold", "push*", "pull*", "turn*", "operat*",
                  "function*", "level*", "step*", "repeat*", "sequence", "count*", "indicat*",
                  "measur*", "test*", "check*", "monitor", "alarm*", "sensor*", "pressure",
                  "flow", "speed", "weight*", "load*", "tool*", "machine*", "pump*", "valve*",
                  "brake*", "engine*", "vehicle*", "seat*", "door*", "window*", "lift*"]),
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
    """The symbol's title from the Commons description, e.g.
    "Symbol 5001B from IEC 60417 - Title/Meaning: Battery, general
    Function: …" -> "Battery, general"."""
    text = meta.get("description") or meta.get("object_name") or ""
    m = re.search(r"(?i)title(?:\s*/\s*meaning)?(?:\s*/\s*referent)?\s*:\s*(.*?)"
                  r"(?=\s+(?:function|description|note|application)\b|$)", text)
    if m:
        text = m.group(1)
    else:
        # "5022 Movement in one direction; To indicate …" style.
        text = re.sub(r"(?i)^(?:symbol\s+)?(?:iec\s*60417\s*[-–:]?\s*)?(?:ref[- ]?no\.?\s*)?"
                      r"[0-9]{4}[a-z]?(?:-[0-9]+)?\s*(?:from iec 60417)?\s*[:\-–]?\s*", "", text)
        text = re.split(r"[;.]\s", text)[0]
    text = re.sub(r'"{2,}', '"', text).strip(" .;:-–/")
    if len(text) > 80:
        text = text[:77].rsplit(" ", 1)[0] + " …"
    return text or f"Ref-No {code}"


def main() -> None:
    work, mirror = commons.parse_args(sys.argv[1:])
    files = commons.cached(work / "files.json", lambda: sorted(
        set(commons.category_files(CATEGORY)) | set(commons.prefix_files(FILE_PREFIX))))
    files = [f for f in files if f.lower().endswith(".svg")]
    print(f"{len(files)} SVG files in category or with prefix", file=sys.stderr)
    info = commons.cached(work / "commons.json", lambda: commons.file_info(files))
    mirrored = commons.mirror_index(mirror)

    manifest, skipped = [], []
    for file in sorted(files):
        meta = info.get(file, {})
        code = ref_no(file)
        if not re.match(r"^[0-9]{4}", code):
            skipped.append((code, "not a numbered IEC 60417 symbol"))
            continue
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
