//! Times the GUI preview of a label file:
//! `cargo run --release -p ll-core --example bench_preview -- <file.llabel> [scale]`.

use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let path = args
        .next()
        .ok_or("usage: bench_preview <file.llabel> [scale]")?;
    let scale: u32 = args.next().map_or(Ok(4), |s| s.parse())?;
    let doc = ll_core::document::Document::load(std::path::Path::new(&path))?;
    let label = &doc.sheets[0].label;
    let model = ll_protocol::model::find_by_name("PT-P710BT").ok_or("model")?;
    for run in 0..4 {
        let t = Instant::now();
        let p = ll_core::label::render_label_preview(label, model, 24, scale)?;
        println!(
            "run {run}: {:?} ({} KB png)",
            t.elapsed(),
            p.png.len() / 1024
        );
    }
    Ok(())
}
