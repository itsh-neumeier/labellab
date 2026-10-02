"""Shared helpers for building LabelLab icon sets from Wikimedia Commons:
polite HTTP with backoff, file metadata (URL, SHA-1, license, author,
description) and verified downloads (optionally from a local mirror whose
files must be byte-identical to Commons, checked by SHA-1).
"""

import hashlib
import json
import re
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

USER_AGENT = "LabelLab-iconset-builder/1.0 (https://github.com/itsh-neumeier/labellab)"
COMMONS_API = "https://commons.wikimedia.org/w/api.php"
# Only licenses that are compatible with shipping inside an MIT program.
ACCEPTED_LICENSES = ("public domain", "cc0", "pd")


def get(url: str, retries: int = 6) -> bytes:
    """GET with backoff on rate limiting (HTTP 429/503 or a text notice)."""
    for attempt in range(retries):
        req = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
        try:
            with urllib.request.urlopen(req, timeout=60) as resp:
                data = resp.read()
            if b"too many requests" not in data[:300].lower():
                return data
        except urllib.error.HTTPError as e:
            if e.code not in (429, 503):
                raise
        wait = 10 * (attempt + 1)
        print(f"  rate limited, waiting {wait}s", file=sys.stderr)
        time.sleep(wait)
    raise RuntimeError(f"giving up on {url}")


def api(params: dict) -> dict:
    return json.loads(get(COMMONS_API + "?" + urllib.parse.urlencode({**params, "format": "json"})))


def strip_html(text: str) -> str:
    text = re.sub(r"<[^>]+>", "", text)
    return re.sub(r"\s+", " ", text).strip()


def category_files(category: str) -> list[str]:
    """All file names (without `File:`) in a Commons category, all pages."""
    files, cont = [], {}
    while True:
        data = api({"action": "query", "list": "categorymembers", "cmtitle": "Category:" + category,
                    "cmtype": "file", "cmlimit": "500", **cont})
        files += [m["title"].removeprefix("File:") for m in data["query"]["categorymembers"]]
        if "continue" not in data:
            return files
        cont = {"cmcontinue": data["continue"]["cmcontinue"]}
        time.sleep(2)


def file_info(files: list[str]) -> dict:
    """File name -> {url, sha1, page, license, artist, description}."""
    info = {}
    for i in range(0, len(files), 40):
        batch = files[i : i + 40]
        data = api({"action": "query", "prop": "imageinfo", "iiprop": "url|sha1|extmetadata",
                    "iiextmetadatalanguage": "en",
                    "titles": "|".join("File:" + f for f in batch)})
        for page in data["query"]["pages"].values():
            ii = (page.get("imageinfo") or [{}])[0]
            meta = ii.get("extmetadata", {})
            value = lambda k: strip_html(str(meta.get(k, {}).get("value", "")))
            info[page["title"].removeprefix("File:")] = {
                "url": ii.get("url"),
                "sha1": ii.get("sha1"),
                "page": ii.get("descriptionurl"),
                "license": value("LicenseShortName"),
                "artist": value("Artist"),
                "description": value("ImageDescription"),
                "object_name": value("ObjectName"),
            }
        time.sleep(2)
    return info


def cached(path: Path, compute):
    """JSON cache: returns the stored value or computes and stores it."""
    if path.exists():
        return json.loads(path.read_text(encoding="utf-8"))
    value = compute()
    path.write_text(json.dumps(value, ensure_ascii=False, indent=1), encoding="utf-8")
    return value


def license_ok(name: str) -> bool:
    return name.lower().startswith(ACCEPTED_LICENSES)


def mirror_index(mirror: Path | None) -> dict:
    """Commons file name (spaces) -> path in a local mirror."""
    if not mirror:
        return {}
    return {p.name.replace("_", " "): p for p in mirror.rglob("*.svg")}


def fetch_verified(target: Path, meta: dict, mirror: dict, file: str) -> str | None:
    """Stores the Commons file at `target` (from the mirror if its SHA-1
    matches, else downloaded). Returns an error text or None."""
    if target.exists():
        return None
    local = mirror.get(file)
    data = local.read_bytes() if local else None
    if data is None or hashlib.sha1(data).hexdigest() != meta.get("sha1"):
        try:
            data = get(meta["url"], retries=4)
        except Exception as err:  # rate limit or network: retry on a later run
            return f"download failed: {err}"
        time.sleep(1)
    if hashlib.sha1(data).hexdigest() != meta.get("sha1"):
        return "checksum mismatch"
    target.write_bytes(data)
    return None


def parse_args(argv: list[str]) -> tuple[Path, Path | None]:
    """`<work-dir> [--mirror <dir>]`."""
    args, mirror = list(argv), None
    if "--mirror" in args:
        i = args.index("--mirror")
        mirror = Path(args[i + 1])
        del args[i : i + 2]
    work = Path(args[0] if args else "iconset-build")
    (work / "svg").mkdir(parents=True, exist_ok=True)
    return work, mirror
