//! Builds a `.llabel-iconset` from a set header (JSON with everything but
//! `icons`) and a manifest of SVG files, cleaning every SVG with
//! `ll_render::iconset::normalize_svg`. Used for the bundled ISO 7010 set,
//! see `tools/iconsets/iso7010/README.md`.
//!
//! cargo run -p ll-render --example build_iconset -- <set.json> <manifest.json> <out>
//!
//! Manifest: JSON array of objects with `code`, `name`, `category`, `svg`
//! (path relative to the manifest), and optional `license`, `author`,
//! `source`.

use std::collections::HashSet;
use std::path::Path;

use ll_render::iconset::{Icon, IconSet, Text};
use serde::Deserialize;

#[derive(Deserialize)]
struct Entry {
    code: String,
    name: String,
    category: Option<String>,
    svg: String,
    license: Option<String>,
    author: Option<String>,
    source: Option<String>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [set_file, manifest_file, out] = args.as_slice() else {
        return Err("usage: build_iconset <set.json> <manifest.json> <out>".into());
    };
    let mut set: IconSet = serde_json::from_slice(&std::fs::read(set_file)?)?;
    let entries: Vec<Entry> = serde_json::from_slice(&std::fs::read(manifest_file)?)?;
    let base = Path::new(manifest_file).parent().unwrap_or(Path::new("."));

    let mut ids = HashSet::new();
    for e in entries {
        let mut id = e.code.trim().replace([' ', ':', '/'], "-");
        let mut n = 2;
        while !ids.insert(id.clone()) {
            id = format!("{}-{n}", e.code.trim().replace([' ', ':', '/'], "-"));
            n += 1;
        }
        let raw = std::fs::read(base.join(&e.svg))?;
        let svg = match ll_render::iconset::normalize_svg(&raw) {
            Ok(svg) => svg,
            Err(err) => {
                eprintln!("skipping {}: {err}", e.code);
                continue;
            }
        };
        set.icons.push(Icon {
            tags: vec![e.code.clone()],
            id,
            name: Text::Plain(e.name),
            category: e.category,
            svg,
            license: e.license,
            author: e.author,
            source: e.source,
        });
    }
    set.validate(true)?;
    std::fs::write(out, set.to_json()?)?;
    eprintln!("{out}: {} icons", set.icons.len());
    Ok(())
}
