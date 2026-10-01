//! Print jobs. Follows the flow from `docs/PROTOCOL.md` "Ablauf eines
//! Druckjobs": invalidate -> initialize -> read status -> check tape ->
//! raster mode -> various/advanced mode -> margin -> print info ->
//! compression -> raster lines -> print.
//!
//! Uses the same `ll_render::Bitmap` for the print path that `ll-cli
//! render`'s PNG preview uses (see `AGENTS.md`: "Vorschau und Druck nutzen
//! denselben Renderpfad"). `print_text`/`print_qr`/`print_code128`/
//! `print_image` share the protocol sequence; only the rendered content
//! differs (frames, other barcode symbologies and symbols are still open
//! M5 scope).

use std::path::Path;
use std::time::Duration;

use ll_protocol::{
    command::{self, PrintInformation},
    model::{ModelInfo, TapeGeometry},
    status::StatusBlock,
};
use ll_render::{Bitmap, QrErrorCorrection};
use ll_transport::Transport;

use crate::CoreError;

const STATUS_TIMEOUT: Duration = Duration::from_secs(3);

/// Resets the printer, reads its status, renders `text` to fit the
/// currently loaded tape and prints it. Fails without sending raster data
/// if the printer reports an error or the tape width isn't in `model`'s
/// geometry table.
pub async fn print_text(
    transport: &mut dyn Transport,
    model: &ModelInfo,
    text: &str,
    auto_cut: bool,
) -> Result<(), CoreError> {
    let (width_mm, geometry) = read_status_and_geometry(transport, model).await?;
    let bitmap = ll_render::render_text(
        text,
        model.head_pins,
        geometry.printable_pins,
        geometry.left_offset_pins,
    )?;
    send_bitmap(transport, &bitmap, width_mm, auto_cut).await
}

/// Resets the printer, reads its status, renders `data` as a QR code to
/// fit the currently loaded tape and prints it. Same failure behavior as
/// [`print_text`].
pub async fn print_qr(
    transport: &mut dyn Transport,
    model: &ModelInfo,
    data: &str,
    ec_level: QrErrorCorrection,
    auto_cut: bool,
) -> Result<(), CoreError> {
    let (width_mm, geometry) = read_status_and_geometry(transport, model).await?;
    let bitmap = ll_render::render_qr(
        data,
        model.head_pins,
        geometry.printable_pins,
        geometry.left_offset_pins,
        ec_level,
    )?;
    send_bitmap(transport, &bitmap, width_mm, auto_cut).await
}

/// Resets the printer, reads its status, renders `data` as a Code128
/// barcode to fit the currently loaded tape and prints it. Same failure
/// behavior as [`print_text`].
pub async fn print_code128(
    transport: &mut dyn Transport,
    model: &ModelInfo,
    data: &str,
    auto_cut: bool,
) -> Result<(), CoreError> {
    let (width_mm, geometry) = read_status_and_geometry(transport, model).await?;
    let bitmap = ll_render::render_code128(
        data,
        model.head_pins,
        geometry.printable_pins,
        geometry.left_offset_pins,
    )?;
    send_bitmap(transport, &bitmap, width_mm, auto_cut).await
}

/// Resets the printer, reads its status, renders the image at `path`
/// (scaled to fit the currently loaded tape, Floyd-Steinberg dithered)
/// and prints it. Same failure behavior as [`print_text`].
pub async fn print_image(
    transport: &mut dyn Transport,
    model: &ModelInfo,
    path: &Path,
    invert: bool,
    auto_cut: bool,
) -> Result<(), CoreError> {
    let (width_mm, geometry) = read_status_and_geometry(transport, model).await?;
    let bitmap = ll_render::render_image(
        path,
        model.head_pins,
        geometry.printable_pins,
        geometry.left_offset_pins,
        invert,
    )?;
    send_bitmap(transport, &bitmap, width_mm, auto_cut).await
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
    let geometry = model
        .tape_geometries
        .iter()
        .find(|g| g.width_mm == width_mm)
        .ok_or(ll_protocol::ProtocolError::UnsupportedTapeWidth(width_mm))?;
    Ok((width_mm, geometry))
}

/// Raster-Modus/Various-Mode/Rand/PrintInformation/Kompression, then the
/// raster lines (PackBits, blank rows as `Z`), then print-with-feed.
async fn send_bitmap(
    transport: &mut dyn Transport,
    bitmap: &Bitmap,
    width_mm: u8,
    auto_cut: bool,
) -> Result<(), CoreError> {
    let raster_lines = bitmap.height_dots();

    transport
        .write_all(&command::switch_to_raster_mode())
        .await?;
    transport
        .write_all(&command::various_mode(auto_cut))
        .await?;
    transport.write_all(&command::margin(0)).await?;
    transport
        .write_all(
            &PrintInformation {
                media_width_mm: width_mm,
                raster_lines,
                is_first_page: true,
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

        print_text(&mut transport, p710bt(), "HI", false)
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
        assert!(
            find_subsequence(written, &[0x1B, 0x69, 0x64, 0x00, 0x00]).is_some(),
            "margin(0) missing"
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

        let err = print_text(&mut transport, p710bt(), "HI", false)
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

        let err = print_text(&mut transport, p710bt(), "HI", false)
            .await
            .unwrap_err();

        assert!(matches!(
            err,
            CoreError::Protocol(ll_protocol::ProtocolError::UnsupportedTapeWidth(200))
        ));
    }

    #[tokio::test]
    async fn print_qr_sends_raster_mode_and_feed() {
        let mut transport = MockTransport::new();
        transport.push_response(status_fixture_9mm_ok());

        print_qr(
            &mut transport,
            p710bt(),
            "https://example.com",
            QrErrorCorrection::Medium,
            false,
        )
        .await
        .unwrap();

        let written = transport.written();
        assert!(find_subsequence(written, &[0x1B, 0x69, 0x61, 0x01]).is_some());
        assert_eq!(*written.last().unwrap(), 0x1A);
    }

    #[tokio::test]
    async fn print_code128_sends_raster_mode_and_feed() {
        let mut transport = MockTransport::new();
        transport.push_response(status_fixture_9mm_ok());

        print_code128(&mut transport, p710bt(), "LABELLAB-123", false)
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

        print_image(&mut transport, p710bt(), &path, false, false)
            .await
            .unwrap();

        std::fs::remove_file(&path).ok();

        let written = transport.written();
        assert!(find_subsequence(written, &[0x1B, 0x69, 0x61, 0x01]).is_some());
        assert_eq!(*written.last().unwrap(), 0x1A);
    }
}
