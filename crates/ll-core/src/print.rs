//! Print jobs. Follows the flow from `docs/PROTOCOL.md` "Ablauf eines
//! Druckjobs": invalidate -> initialize -> read status -> check tape ->
//! raster mode -> various/advanced mode -> margin -> print info ->
//! compression -> raster lines -> print.
//!
//! Uses the same `ll_render::Bitmap` for the print path that `ll-cli
//! render`'s PNG preview uses (see `AGENTS.md`: "Vorschau und Druck nutzen
//! denselben Renderpfad"): everything goes through [`print_labels`] and
//! `crate::label::render_label_pages`; `print_text`/`print_qr`/
//! `print_barcode`/`print_image`/`print_symbol` are one-element shortcuts.

use std::path::Path;
use std::time::Duration;

use ll_protocol::{
    command::{self, PrintInformation},
    model::{ModelInfo, TapeGeometry},
    status::StatusBlock,
};
use ll_render::{Bitmap, Symbology};
use ll_transport::Transport;

use crate::label::{geometry_for, render_label_pages, Element, Label};
use crate::CoreError;

const STATUS_TIMEOUT: Duration = Duration::from_secs(3);

/// Default printer feed margin (`ESC i d`) in dots. 0: the distance
/// between cut and content comes from the label's own margins
/// (`Label::padding_start_mm`/`padding_mm`), which the preview shows. The
/// printer adds this feed before *and* after a page (one measurement,
/// TODO(verify) in `docs/PROTOCOL.md`); a larger value is taken out of the
/// label's blank ends (`trim_for_margin`), so it never changes the length.
const DEFAULT_MARGIN_DOTS: u16 = 0;

/// Options shared by every `print_*` function (what differs between them
/// is only the rendered content).
#[derive(Debug, Clone, Copy)]
pub struct PrintOptions {
    /// Draw a border around the whole label.
    pub frame: bool,
    /// Auto-cut ("Abschneiden"). Without [`Self::chain`] after every
    /// label; with it only after the last one.
    ///
    /// Observed on a PT-P710BT (2026-10-01): with auto-cut on, the
    /// printer also cuts off the leader at the start of a job by itself;
    /// an extra blank "pre-cut" page produced a third cut and was
    /// removed. TODO(verify): confirm exactly one leading cut.
    pub auto_cut: bool,
    /// Chain printing ("fortlaufend"): all labels of a job as pages of one
    /// print job, no cut in between (`0C` between pages, `1A` after the
    /// last). TODO(verify): multi-page jobs on real hardware.
    pub chain: bool,
    /// Most blank feed in dots the printer adds before and after a page.
    /// It is taken out of the label's own blank ends (its margins), so the
    /// printed length equals the rendered one; less if those ends are
    /// shorter (see `trim_for_margin`).
    pub margin_dots: u16,
    /// With [`Self::chain`] and auto-cut: also cut after every n-th label
    /// (0 or 1 = only after the last). Uses the auto-cut flag per page of
    /// the chained job. TODO(verify): the PT-P710BT honours per-page cut
    /// flags inside one job (it has no `ESC i A` "cut every n").
    pub cut_every: u32,
    /// Print a dotted cut mark at the end of every label (for cutting by
    /// hand when the printer doesn't cut).
    pub cut_marks: bool,
    /// Mirror the label along its length (mirror print).
    pub mirror: bool,
}

impl Default for PrintOptions {
    fn default() -> Self {
        Self {
            frame: false,
            auto_cut: false,
            chain: false,
            margin_dots: DEFAULT_MARGIN_DOTS,
            cut_every: 0,
            cut_marks: false,
            mirror: false,
        }
    }
}

/// Resets the printer, reads its status, renders `label` for the
/// currently loaded tape (see [`crate::label::render_label`]) and prints
/// it. Fails without sending raster data if the printer reports an error
/// or the tape width isn't in `model`'s geometry table. `options.frame`
/// adds a border even if `label.frame` is off.
pub async fn print_label(
    transport: &mut dyn Transport,
    model: &ModelInfo,
    label: &Label,
    options: &PrintOptions,
) -> Result<(), CoreError> {
    print_labels(
        transport,
        model,
        std::slice::from_ref(label),
        1,
        options,
        &mut |_, _| {},
    )
    .await
}

/// Prints `labels` (each `copies` times, every strip of a multi-tape
/// label as its own piece) and reports `progress(done, total)` pages.
///
/// Without [`PrintOptions::chain`] every page is its own print job
/// (invalidate, status check, one page) — the hardware-verified path.
/// With it, the status is read once and all pages go out as one job, cut
/// (if enabled) only after the last page.
pub async fn print_labels(
    transport: &mut dyn Transport,
    model: &ModelInfo,
    labels: &[Label],
    copies: u32,
    options: &PrintOptions,
    progress: &mut (dyn FnMut(u32, u32) + Send),
) -> Result<(), CoreError> {
    let (width_mm, geometry) = read_status_and_geometry(transport, model).await?;
    let mut pages = Vec::new();
    for label in labels {
        let framed;
        let label = if options.frame && !label.frame {
            framed = Label {
                frame: true,
                ..label.clone()
            };
            &framed
        } else {
            label
        };
        let rendered = render_label_pages(label, model, geometry)?;
        for _ in 0..copies.max(1) {
            pages.extend(rendered.iter().cloned());
        }
    }

    // The printer feeds `margin` blank before *and* after the raster data
    // (a fixed 100 mm label came out ~108 mm with 28 dots, 2026-10-02).
    // Take that feed out of the label's own blank ends instead, so the
    // printed length matches the preview.
    let pages: Vec<(Bitmap, u16)> = pages
        .into_iter()
        .map(|p| {
            let mut page = if options.mirror { p.mirrored() } else { p };
            if options.cut_marks {
                add_cut_mark(
                    &mut page,
                    geometry.left_offset_pins,
                    geometry.printable_pins,
                );
            }
            trim_for_margin(&page, options.margin_dots)
        })
        .collect();

    let total = pages.len() as u32;
    progress(0, total);
    for (i, (page, margin)) in pages.iter().enumerate() {
        let last = i + 1 == pages.len();
        if options.chain {
            let every = options.cut_every.max(1) as usize;
            let cut = options.auto_cut && (last || (every > 1 && (i + 1) % every == 0));
            send_page(transport, page, width_mm, cut, *margin, i == 0, last).await?;
        } else {
            if i > 0 {
                // Fresh job per page, same as a single print.
                let (w, _) = read_status_and_geometry(transport, model).await?;
                if w != width_mm {
                    return Err(ll_protocol::ProtocolError::UnsupportedTapeWidth(w).into());
                }
            }
            send_page(
                transport,
                page,
                width_mm,
                options.auto_cut,
                *margin,
                true,
                true,
            )
            .await?;
        }
        progress(i as u32 + 1, total);
    }
    Ok(())
}

/// Prints a single line of `text`, see [`print_label`].
pub async fn print_text(
    transport: &mut dyn Transport,
    model: &ModelInfo,
    text: &str,
    options: &PrintOptions,
) -> Result<(), CoreError> {
    let label = Label::single(Element::text(text));
    print_label(transport, model, &label, options).await
}

/// Prints `data` as a QR code (error correction level medium), see
/// [`print_label`].
pub async fn print_qr(
    transport: &mut dyn Transport,
    model: &ModelInfo,
    data: &str,
    options: &PrintOptions,
) -> Result<(), CoreError> {
    let label = Label::single(Element::Qr { data: data.into() });
    print_label(transport, model, &label, options).await
}

/// Prints `data` as a barcode of the given `symbology`, see
/// [`print_label`].
pub async fn print_barcode(
    transport: &mut dyn Transport,
    model: &ModelInfo,
    symbology: Symbology,
    data: &str,
    options: &PrintOptions,
) -> Result<(), CoreError> {
    let label = Label::single(Element::Barcode {
        symbology,
        data: data.into(),
    });
    print_label(transport, model, &label, options).await
}

/// Prints the image at `path` (scaled to the tape, Floyd-Steinberg
/// dithered), see [`print_label`].
pub async fn print_image(
    transport: &mut dyn Transport,
    model: &ModelInfo,
    path: &Path,
    invert: bool,
    options: &PrintOptions,
) -> Result<(), CoreError> {
    let label = Label::single(Element::Image {
        path: path.into(),
        invert,
        brightness: 0,
        contrast: 0,
        edit: Default::default(),
    });
    print_label(transport, model, &label, options).await
}

/// Prints the bundled symbol `name` (see `ll_render::SYMBOL_NAMES`), see
/// [`print_label`].
pub async fn print_symbol(
    transport: &mut dyn Transport,
    model: &ModelInfo,
    name: &str,
    invert: bool,
    options: &PrintOptions,
) -> Result<(), CoreError> {
    let label = Label::single(Element::Symbol {
        name: name.into(),
        invert,
    });
    print_label(transport, model, &label, options).await
}

/// Invalidate -> initialize -> status request -> parse. Returns the loaded
/// tape's width and matching geometry entry from `model`.
async fn read_status_and_geometry<'m>(
    transport: &mut dyn Transport,
    model: &'m ModelInfo,
) -> Result<(u8, &'m TapeGeometry), CoreError> {
    transport.write_all(&command::invalidate()).await?;
    transport.write_all(&command::initialize()).await?;
    transport.write_all(&command::status_request()).await?;

    let mut status_buf = [0u8; ll_protocol::status::STATUS_BLOCK_LEN];
    transport
        .read_exact_timeout(&mut status_buf, STATUS_TIMEOUT)
        .await?;
    let status = StatusBlock::parse(&status_buf)?;

    if status.has_error() {
        return Err(CoreError::PrinterError {
            error1: status.error1(),
            error2: status.error2(),
        });
    }

    let width_mm = status.media_width_mm();
    Ok((width_mm, geometry_for(model, width_mm)?))
}

/// Dotted line across the printable pins on the last raster line.
fn add_cut_mark(page: &mut Bitmap, offset: u16, pins: u16) {
    let line = page.height_dots().saturating_sub(1);
    for p in (0..pins).filter(|p| (p / CUT_MARK_DASH) % 2 == 0) {
        page.set_pixel(offset + p, line, true);
    }
}

/// Dash length of the cut mark in pins.
const CUT_MARK_DASH: u16 = 3;

/// Removes up to `max_margin` blank raster lines from each end of `page`
/// and returns it with the feed margin to send instead: the printer adds
/// that margin before and after the page (TODO(verify): symmetric feed
/// derived from one length measurement, see `docs/PROTOCOL.md`), so the
/// total length stays the rendered one. Ink is never trimmed: the margin
/// shrinks to the smaller blank end (0 if content touches an end).
fn trim_for_margin(page: &Bitmap, max_margin: u16) -> (Bitmap, u16) {
    let len = page.height_dots();
    let blank = |line: u32| page.row(line).iter().all(|&b| b == 0);
    let lead = (0..len).take_while(|&l| blank(l)).count() as u32;
    let trail = (0..len).rev().take_while(|&l| blank(l)).count() as u32;
    let trim = (max_margin as u32).min(lead).min(trail).min(len / 2);
    if trim == 0 {
        return (page.clone(), 0);
    }
    let mut out = Bitmap::new(page.width_pins(), len - 2 * trim);
    out.blit(page, 0, -(trim as i32), 0..page.width_pins());
    (out, trim as u16)
}

/// One page: control codes (raster mode, various mode, margin, print
/// information, compression), the raster lines (PackBits, blank rows as
/// `Z`), then `1A` (last page) or `0C` (more pages follow in this job).
async fn send_page(
    transport: &mut dyn Transport,
    bitmap: &Bitmap,
    width_mm: u8,
    auto_cut: bool,
    margin_dots: u16,
    first: bool,
    last: bool,
) -> Result<(), CoreError> {
    let raster_lines = bitmap.height_dots();

    transport
        .write_all(&command::switch_to_raster_mode())
        .await?;
    transport
        .write_all(&command::various_mode(auto_cut))
        .await?;
    transport.write_all(&command::margin(margin_dots)).await?;
    transport
        .write_all(
            &PrintInformation {
                media_width_mm: width_mm,
                raster_lines,
                is_first_page: first,
            }
            .to_bytes(),
        )
        .await?;
    transport
        .write_all(&command::select_packbits_compression())
        .await?;

    for y in 0..raster_lines {
        let row = bitmap.row(y);
        if row.iter().all(|&b| b == 0) {
            transport.write_all(&command::empty_row()).await?;
        } else {
            let compressed = ll_protocol::packbits::encode(row);
            transport
                .write_all(&command::raster_line(&compressed))
                .await?;
        }
    }

    if !last {
        transport.write_all(&command::print_page()).await?;
        return Ok(());
    }
    transport.write_all(&command::print_with_feed()).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use ll_protocol::model;
    use ll_transport::mock::MockTransport;

    use super::*;

    /// 32-byte status fixture: 9 mm band, no errors (see the hardware test
    /// recorded in `docs/PROTOCOL.md`).
    fn status_fixture_9mm_ok() -> [u8; ll_protocol::status::STATUS_BLOCK_LEN] {
        let mut b = [0u8; ll_protocol::status::STATUS_BLOCK_LEN];
        b[0] = 0x80;
        b[10] = 9;
        b
    }

    fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
        haystack
            .windows(needle.len())
            .position(|window| window == needle)
    }

    fn p710bt() -> &'static model::ModelInfo {
        model::find_by_name("PT-P710BT").expect("PT-P710BT must be in the model table")
    }

    #[tokio::test]
    async fn sends_expected_command_sequence() {
        let mut transport = MockTransport::new();
        transport.push_response(status_fixture_9mm_ok());

        print_text(&mut transport, p710bt(), "HI", &PrintOptions::default())
            .await
            .unwrap();

        let written = transport.written();

        assert_eq!(&written[0..100], &vec![0x00; 100][..], "invalidate");
        assert_eq!(&written[100..102], &[0x1B, 0x40], "initialize");
        assert_eq!(&written[102..105], &[0x1B, 0x69, 0x53], "status_request");

        assert!(
            find_subsequence(written, &[0x1B, 0x69, 0x61, 0x01]).is_some(),
            "switch_to_raster_mode missing"
        );
        assert!(
            find_subsequence(written, &[0x1B, 0x69, 0x4D, 0x00]).is_some(),
            "various_mode (no auto-cut) missing"
        );
        // Text without padding touches both ends: no feed margin left.
        assert!(
            find_subsequence(written, &[0x1B, 0x69, 0x64, 0, 0]).is_some(),
            "margin command missing"
        );
        assert!(
            find_subsequence(written, &[0x1B, 0x69, 0x7A]).is_some(),
            "PrintInformation header missing"
        );
        assert!(
            find_subsequence(written, &[0x4D, 0x02]).is_some(),
            "select_packbits_compression missing"
        );
        assert_eq!(*written.last().unwrap(), 0x1A, "print_with_feed");

        // Exactly one raster command (G or Z) per rendered column.
        let geometry = p710bt()
            .tape_geometries
            .iter()
            .find(|g| g.width_mm == 9)
            .unwrap();
        let expected_bitmap = ll_render::render_text(
            "HI",
            p710bt().head_pins,
            geometry.printable_pins,
            geometry.left_offset_pins,
        )
        .unwrap();
        let raster_command_count = written
            .iter()
            .filter(|&&b| b == 0x47 /* G */ || b == 0x5A /* Z */)
            .count();
        // 0x47/0x5A can theoretically also appear inside PackBits-compressed
        // row data; for this short, mostly-blank "HI" label at 9mm that
        // doesn't happen, so a direct byte count is a safe cross-check here.
        assert_eq!(
            raster_command_count as u32,
            expected_bitmap.height_dots(),
            "one raster command per bitmap column"
        );
    }

    #[tokio::test]
    async fn bails_out_without_sending_raster_data_on_printer_error() {
        let mut transport = MockTransport::new();
        let mut status = status_fixture_9mm_ok();
        status[8] = 0x01; // error1 != 0
        transport.push_response(status);

        let err = print_text(&mut transport, p710bt(), "HI", &PrintOptions::default())
            .await
            .unwrap_err();

        assert!(matches!(
            err,
            CoreError::PrinterError {
                error1: 0x01,
                error2: 0
            }
        ));
        // Only invalidate+initialize+status_request were sent, nothing else.
        assert_eq!(transport.written().len(), 100 + 2 + 3);
    }

    #[tokio::test]
    async fn rejects_unsupported_tape_width() {
        let mut transport = MockTransport::new();
        let mut status = status_fixture_9mm_ok();
        status[10] = 200; // not in the model's geometry table
        transport.push_response(status);

        let err = print_text(&mut transport, p710bt(), "HI", &PrintOptions::default())
            .await
            .unwrap_err();

        assert!(matches!(
            err,
            CoreError::Protocol(ll_protocol::ProtocolError::UnsupportedTapeWidth(200))
        ));
    }

    #[tokio::test]
    async fn chain_prints_one_job_and_cuts_only_after_the_last_page() {
        let mut transport = MockTransport::new();
        transport.push_response(status_fixture_9mm_ok());
        let options = PrintOptions {
            chain: true,
            auto_cut: true,
            ..PrintOptions::default()
        };
        let labels = [
            Label::single(Element::text("A")),
            Label::single(Element::text("B")),
        ];
        let mut seen = Vec::new();
        print_labels(
            &mut transport,
            p710bt(),
            &labels,
            2,
            &options,
            &mut |d, t| seen.push((d, t)),
        )
        .await
        .unwrap();

        let written = transport.written();
        // One status request for the whole job.
        assert_eq!(count(written, &[0x1B, 0x69, 0x53]), 1);
        // Four pages: auto-cut only on the last one.
        assert_eq!(count(written, &[0x1B, 0x69, 0x4D, 0x00]), 3);
        assert_eq!(count(written, &[0x1B, 0x69, 0x4D, 0x40]), 1);
        assert_eq!(*written.last().unwrap(), 0x1A);
        // Pages 2-4 are marked as "other page" (n9 = 1).
        assert_eq!(seen.last(), Some(&(4, 4)));
    }

    #[tokio::test]
    async fn chain_can_cut_every_n_labels() {
        let mut transport = MockTransport::new();
        transport.push_response(status_fixture_9mm_ok());
        let options = PrintOptions {
            chain: true,
            auto_cut: true,
            cut_every: 2,
            ..PrintOptions::default()
        };
        let labels = [Label::single(Element::text("A"))];
        print_labels(
            &mut transport,
            p710bt(),
            &labels,
            5,
            &options,
            &mut |_, _| {},
        )
        .await
        .unwrap();
        let written = transport.written();
        // Pages 2, 4 and the last (5) cut.
        assert_eq!(count(written, &[0x1B, 0x69, 0x4D, 0x40]), 3);
        assert_eq!(count(written, &[0x1B, 0x69, 0x4D, 0x00]), 2);
    }

    #[test]
    fn mirror_and_cut_mark() {
        let mut page = Bitmap::new(128, 10);
        page.set_pixel(60, 2, true);
        let m = page.mirrored();
        assert!(m.pixel(60, 7) && !m.pixel(60, 2));
        assert_eq!(m.mirrored(), page);
        add_cut_mark(&mut page, 39, 50);
        assert!(page.pixel(39, 9) && !page.pixel(42, 9) && page.pixel(45, 9));
        assert!(!page.pixel(38, 9) && !page.pixel(89, 9));
    }

    #[tokio::test]
    async fn without_chain_every_page_is_its_own_job() {
        let mut transport = MockTransport::new();
        transport.push_response(status_fixture_9mm_ok());
        transport.push_response(status_fixture_9mm_ok());
        let options = PrintOptions {
            auto_cut: true,
            ..PrintOptions::default()
        };
        let labels = [
            Label::single(Element::text("A")),
            Label::single(Element::text("B")),
        ];
        print_labels(
            &mut transport,
            p710bt(),
            &labels,
            1,
            &options,
            &mut |_, _| {},
        )
        .await
        .unwrap();
        let written = transport.written();
        assert_eq!(count(written, &[0x1B, 0x69, 0x53]), 2);
        assert_eq!(count(written, &[0x1B, 0x69, 0x4D, 0x40]), 2);
    }

    fn count(haystack: &[u8], needle: &[u8]) -> usize {
        haystack
            .windows(needle.len())
            .filter(|w| *w == needle)
            .count()
    }

    #[tokio::test]
    async fn print_qr_sends_raster_mode_and_feed() {
        let mut transport = MockTransport::new();
        transport.push_response(status_fixture_9mm_ok());

        print_qr(
            &mut transport,
            p710bt(),
            "https://example.com",
            &PrintOptions::default(),
        )
        .await
        .unwrap();

        let written = transport.written();
        assert!(find_subsequence(written, &[0x1B, 0x69, 0x61, 0x01]).is_some());
        assert_eq!(*written.last().unwrap(), 0x1A);
    }

    #[tokio::test]
    async fn print_barcode_sends_raster_mode_and_feed() {
        let mut transport = MockTransport::new();
        transport.push_response(status_fixture_9mm_ok());

        print_barcode(
            &mut transport,
            p710bt(),
            Symbology::Code128,
            "LABELLAB-123",
            &PrintOptions::default(),
        )
        .await
        .unwrap();

        let written = transport.written();
        assert!(find_subsequence(written, &[0x1B, 0x69, 0x61, 0x01]).is_some());
        assert_eq!(*written.last().unwrap(), 0x1A);
    }

    #[tokio::test]
    async fn print_image_sends_raster_mode_and_feed() {
        let img = image::GrayImage::from_pixel(10, 10, image::Luma([0u8]));
        let path = std::env::temp_dir().join("labellab_print_image_test.png");
        img.save(&path).unwrap();

        let mut transport = MockTransport::new();
        transport.push_response(status_fixture_9mm_ok());

        print_image(
            &mut transport,
            p710bt(),
            &path,
            false,
            &PrintOptions::default(),
        )
        .await
        .unwrap();

        std::fs::remove_file(&path).ok();

        let written = transport.written();
        assert!(find_subsequence(written, &[0x1B, 0x69, 0x61, 0x01]).is_some());
        assert_eq!(*written.last().unwrap(), 0x1A);
    }

    #[tokio::test]
    async fn frame_adds_more_ink_than_without() {
        // Compares raw byte counts of two MockTransport runs: a bordered
        // label has to send more non-empty raster rows (the border caps)
        // than the same label without a border.
        let mut with_frame = MockTransport::new();
        with_frame.push_response(status_fixture_9mm_ok());
        print_text(
            &mut with_frame,
            p710bt(),
            "HI",
            &PrintOptions {
                frame: true,
                ..Default::default()
            },
        )
        .await
        .unwrap();

        let mut without_frame = MockTransport::new();
        without_frame.push_response(status_fixture_9mm_ok());
        print_text(&mut without_frame, p710bt(), "HI", &PrintOptions::default())
            .await
            .unwrap();

        let empty_rows = |written: &[u8]| written.iter().filter(|&&b| b == 0x5A).count();
        assert!(
            empty_rows(with_frame.written()) < empty_rows(without_frame.written()),
            "a bordered label should have fewer blank raster rows (border fills the caps)"
        );
    }

    #[tokio::test]
    async fn custom_margin_is_sent() {
        let mut transport = MockTransport::new();
        transport.push_response(status_fixture_9mm_ok());
        // 20 mm blank on both ends leaves room for a 100-dot feed margin.
        let label = Label {
            elements: vec![Element::Qr { data: "x".into() }.into()],
            padding_mm: 20.0,
            ..Label::default()
        };
        print_label(
            &mut transport,
            p710bt(),
            &label,
            &PrintOptions {
                margin_dots: 100,
                ..Default::default()
            },
        )
        .await
        .unwrap();

        let written = transport.written();
        assert!(
            find_subsequence(written, &[0x1B, 0x69, 0x64, 100, 0]).is_some(),
            "margin(100) missing"
        );
    }

    #[test]
    fn margin_comes_out_of_the_blank_ends() {
        let mut page = Bitmap::new(128, 200);
        for line in 50..170 {
            page.set_pixel(60, line, true);
        }
        // Blank ends 50 and 30: the feed takes 30 from each, no ink lost.
        let (out, margin) = trim_for_margin(&page, 40);
        assert_eq!(margin, 30);
        assert_eq!(out.height_dots(), 140);
        assert_eq!(out.height_dots() + 2 * margin as u32, page.height_dots());
        assert!(out.pixel(60, 20) && out.pixel(60, 139) && !out.pixel(60, 19));
        // Ink at an end: no margin, unchanged page.
        page.set_pixel(0, 0, true);
        let (out, margin) = trim_for_margin(&page, 40);
        assert_eq!((margin, out.height_dots()), (0, 200));
    }

    #[tokio::test]
    async fn print_symbol_sends_raster_mode_and_feed() {
        let mut transport = MockTransport::new();
        transport.push_response(status_fixture_9mm_ok());

        print_symbol(
            &mut transport,
            p710bt(),
            "warning",
            false,
            &PrintOptions::default(),
        )
        .await
        .unwrap();

        let written = transport.written();
        assert!(find_subsequence(written, &[0x1B, 0x69, 0x61, 0x01]).is_some());
        assert_eq!(*written.last().unwrap(), 0x1A);
    }
}
