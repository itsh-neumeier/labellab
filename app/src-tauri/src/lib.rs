//! Tauri backend: thin command layer over `ll-core`. All rendering,
//! protocol and transport logic stays in the library crates; the preview
//! and the print job both go through `ll_core::label::render_label`
//! (`AGENTS.md`: "Vorschau und Druck nutzen denselben Renderpfad").
//!
//! Errors are returned to the frontend as plain strings; the frontend
//! shows them under a localized heading.

use std::path::PathBuf;
use std::sync::Mutex;

use base64::Engine;
use ll_core::device::{self, Connection};
use ll_core::label::{self, Label, Rect};
use ll_core::layouts::{self, Layout};
use ll_core::print::PrintOptions;
use ll_core::series::{self, DataSet, Numbering};
use ll_protocol::model::{self, dots_to_mm, ModelInfo, MODELS};
use serde::{Deserialize, Serialize};
use tauri::{Emitter, State};

/// CSV data loaded for series printing, shared by preview and print.
#[derive(Default)]
struct SeriesState(Mutex<Option<DataSet>>);

impl SeriesState {
    fn get(&self) -> Option<DataSet> {
        self.0.lock().ok().and_then(|g| g.clone())
    }
}

/// `label` with placeholders filled from record `row` of the loaded CSV,
/// or unchanged without CSV/row.
fn with_record(
    label: Label,
    series: &SeriesState,
    row: Option<usize>,
    numbering: Option<Numbering>,
) -> Label {
    let Some(n) = row else { return label };
    let data = series.get();
    if data.is_none() && numbering.is_none() {
        return label;
    }
    series::apply(&label, data.as_ref(), n, numbering.unwrap_or_default())
}

/// Baud rate for serial ports picked in the GUI. Virtual Bluetooth-SPP
/// ports ignore it, but the OS API needs one (same as the CLI default).
const SERIAL_BAUD_RATE: u32 = 9600;

/// Error sent to the frontend: `code` selects a translated message
/// (`error.<code>` in the i18n files), `detail` is the raw text.
#[derive(Debug, Serialize)]
struct AppError {
    code: &'static str,
    detail: String,
}

impl AppError {
    fn new(code: &'static str, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: detail.into(),
        }
    }
}

impl From<ll_core::CoreError> for AppError {
    fn from(e: ll_core::CoreError) -> Self {
        Self::new(e.code(), e.to_string())
    }
}

fn err(e: impl Into<AppError>) -> AppError {
    e.into()
}

fn find_model(name: &str) -> Result<&'static ModelInfo, AppError> {
    model::find_by_name(name)
        .ok_or_else(|| AppError::new("unknown_model", format!("unknown model {name:?}")))
}

#[derive(Serialize)]
struct TapeDto {
    width_mm: u8,
    /// Height of the printable area across the tape, in mm (the editor's
    /// vertical extent).
    printable_mm: f32,
}

#[derive(Serialize)]
struct ModelDto {
    name: &'static str,
    tapes: Vec<TapeDto>,
}

/// Known printer models and their supported tapes.
#[tauri::command]
fn models() -> Vec<ModelDto> {
    MODELS
        .iter()
        .map(|m| ModelDto {
            name: m.name,
            tapes: m
                .tape_geometries
                .iter()
                .map(|g| TapeDto {
                    width_mm: g.width_mm,
                    printable_mm: dots_to_mm(g.printable_pins as u32),
                })
                .collect(),
        })
        .collect()
}

/// Renders `label` for a `width_mm` tape as a PNG, base64-encoded. A plain
/// string survives every IPC transport (raw binary responses arrived
/// broken in the Windows WebView2 build, preview stayed empty).
///
/// Async and on a blocking worker: a synchronous command runs on the main
/// thread and froze the window while a large label rendered.
#[tauri::command]
async fn render_preview(
    label: Label,
    model: String,
    width_mm: u8,
    row: Option<usize>,
    numbering: Option<Numbering>,
    scale: u32,
    series: State<'_, SeriesState>,
) -> Result<PreviewDto, AppError> {
    let label = with_record(label, &series, row, numbering);
    let model = find_model(&model)?;
    tauri::async_runtime::spawn_blocking(move || {
        let preview =
            label::render_label_preview(&label, model, width_mm, scale.clamp(1, 8)).map_err(err)?;
        Ok(PreviewDto {
            png: base64::engine::general_purpose::STANDARD.encode(preview.png),
            overflowing: preview.overflowing,
        })
    })
    .await
    .map_err(|e| AppError::new("internal", e.to_string()))?
}

#[derive(Serialize)]
struct PreviewDto {
    /// Base64 PNG mask.
    png: String,
    /// Indices of elements whose text is clipped.
    overflowing: Vec<usize>,
}

/// Every element's box in mm as rendered (flow elements get the box the
/// flow layout gives them), so the editor can make them movable.
#[tauri::command]
fn resolve_rects(label: Label, model: String, width_mm: u8) -> Result<Vec<Rect>, AppError> {
    let model = find_model(&model)?;
    let geometry = label::geometry_for(model, width_mm).map_err(err)?;
    label::resolved_rects(&label, model, geometry).map_err(err)
}

#[derive(Serialize)]
struct DeviceDto {
    /// Human-readable name for the device picker.
    name: String,
    connection: Connection,
    /// Recognized printer model, if any (USB VID:PID or Bluetooth name).
    model: Option<&'static str>,
}

#[derive(Serialize)]
struct DeviceListDto {
    /// Recognized printers first, then other Bluetooth devices, then
    /// serial ports.
    devices: Vec<DeviceDto>,
    /// Non-fatal enumeration failures (e.g. no USB access).
    warnings: Vec<String>,
}

/// Lists USB printers, paired Bluetooth devices (Windows) and serial
/// ports. One failing transport doesn't hide the others.
#[tauri::command]
async fn list_devices() -> DeviceListDto {
    let mut devices = Vec::new();
    let mut warnings = Vec::new();

    match device::list_usb_printers().await {
        Ok(printers) => devices.extend(printers.into_iter().map(|p| {
            let spec = p.serial_number.clone().unwrap_or_else(|| p.usb_id());
            DeviceDto {
                name: format!("{} (USB)", p.model.name),
                connection: Connection::Usb { spec: Some(spec) },
                model: Some(p.model.name),
            }
        })),
        Err(e) => warnings.push(format!("USB: {e}")),
    }

    #[cfg(any(windows, target_os = "linux"))]
    match device::list_bluetooth_devices().await {
        Ok(bt) => devices.extend(bt.into_iter().map(|d| DeviceDto {
            model: device::model_for_device_name(&d.name).map(|m| m.name),
            name: format!("{} (Bluetooth)", d.name),
            connection: Connection::Bluetooth { device_id: d.id },
        })),
        Err(e) => warnings.push(format!("Bluetooth: {e}")),
    }

    match device::list_serial_devices() {
        Ok(ports) => devices.extend(ports.into_iter().map(|port| DeviceDto {
            name: port.clone(),
            connection: Connection::Serial {
                port,
                baud_rate: SERIAL_BAUD_RATE,
            },
            model: None,
        })),
        Err(e) => warnings.push(format!("Serial: {e}")),
    }

    // Stable sort: recognized printers first, original order otherwise.
    devices.sort_by_key(|d| d.model.is_none());
    DeviceListDto { devices, warnings }
}

#[derive(Serialize)]
struct StatusDto {
    width_mm: u8,
    media_type: u8,
    tape_color: u8,
    text_color: u8,
    /// Ids from `ll_protocol::media` (e.g. "white", "black"), if known.
    tape_color_id: Option<&'static str>,
    text_color_id: Option<&'static str>,
    has_error: bool,
    error1: u8,
    error2: u8,
}

/// Connects and reads the tape status.
///
/// TODO: WinRT Bluetooth blocks the calling thread while connecting (see
/// `ll_transport::bluetooth`); fine for now since Tauri runs async
/// commands on a worker pool, but worth a `spawn_blocking` later.
#[tauri::command]
async fn query_status(connection: Connection) -> Result<StatusDto, AppError> {
    let s = device::query_status_on(&connection).await.map_err(err)?;
    Ok(StatusDto {
        width_mm: s.media_width_mm(),
        media_type: s.media_type(),
        tape_color: s.tape_color(),
        text_color: s.text_color(),
        tape_color_id: ll_protocol::media::tape_color_id(s.tape_color()),
        text_color_id: ll_protocol::media::text_color_id(s.text_color()),
        has_error: s.has_error(),
        error1: s.error1(),
        error2: s.error2(),
    })
}

/// Print job settings from the print bar.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PrintJob {
    copies: u32,
    /// Cut (after every label, or only at the end when chained).
    cut: bool,
    /// All labels in one job without cuts in between.
    chain: bool,
    margin_dots: u16,
    /// Record numbers (1-based, inclusive) of the loaded CSV; `None`
    /// without CSV prints the label as is, with CSV all records.
    rows: Option<(usize, usize)>,
    /// Without CSV: number of labels of a numbered series (`{{n}}`).
    count: Option<usize>,
    /// Running number start/step for `{{n}}`/`{{a}}`/`{{A}}`.
    numbering: Option<Numbering>,
}

/// Progress event payload (`print-progress`).
#[derive(Clone, Serialize)]
struct Progress {
    done: u32,
    total: u32,
}

/// Prints `label` (per selected CSV record, if a CSV is loaded) `copies`
/// times each over one connection, emitting `print-progress` events.
#[tauri::command]
async fn print_label(
    app: tauri::AppHandle,
    label: Label,
    connection: Connection,
    model: String,
    job: PrintJob,
    series: State<'_, SeriesState>,
) -> Result<(), AppError> {
    let model = find_model(&model)?;
    let numbering = job.numbering.unwrap_or_default();
    let labels: Vec<Label> = match (series.get(), job.count) {
        (Some(data), _) => {
            let range = data.select(job.rows.map(|(a, b)| a..=b));
            if range.is_empty() {
                return Err(AppError::new(
                    "no_records",
                    "no records in the selected range",
                ));
            }
            range
                .map(|n| series::apply(&label, Some(&data), n, numbering))
                .collect()
        }
        (None, Some(count)) if count > 0 => (1..=count)
            .map(|n| series::apply(&label, None, n, numbering))
            .collect(),
        // Single label: still fill {{n}}/{{A}} with the first number so
        // print matches the preview.
        (None, _) => vec![series::apply(&label, None, 1, numbering)],
    };
    let options = PrintOptions {
        frame: false,
        auto_cut: job.cut,
        chain: job.chain,
        margin_dots: job.margin_dots,
    };

    let mut transport = device::connect(&connection).await.map_err(err)?;
    ll_core::print::print_labels(
        transport.as_mut(),
        model,
        &labels,
        job.copies.max(1),
        &options,
        &mut |done, total| {
            let _ = app.emit("print-progress", Progress { done, total });
        },
    )
    .await
    .map_err(err)?;
    transport
        .close()
        .await
        .map_err(|e| err(ll_core::CoreError::from(e)))
}

#[derive(Serialize)]
struct CsvDto {
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
}

/// Loads a CSV for series printing (replaces a previously loaded one).
#[tauri::command]
fn load_csv(path: PathBuf, series: State<'_, SeriesState>) -> Result<CsvDto, AppError> {
    let data = DataSet::load(&path).map_err(err)?;
    let dto = CsvDto {
        headers: data.headers.clone(),
        rows: data.rows.clone(),
    };
    if let Ok(mut guard) = series.0.lock() {
        *guard = Some(data);
    }
    Ok(dto)
}

#[tauri::command]
fn clear_csv(series: State<'_, SeriesState>) {
    if let Ok(mut guard) = series.0.lock() {
        *guard = None;
    }
}

/// Installed font families (scans system fonts once, can take a moment).
#[tauri::command]
async fn font_families() -> Vec<String> {
    tauri::async_runtime::spawn_blocking(ll_render::fonts::families)
        .await
        .unwrap_or_default()
}

#[derive(Serialize)]
struct PairableDto {
    id: String,
    name: String,
    model: Option<&'static str>,
}

/// Bluetooth devices nearby that can be paired, recognized printers first.
#[tauri::command]
async fn discover_bluetooth() -> Result<Vec<PairableDto>, AppError> {
    #[cfg(any(windows, target_os = "linux"))]
    {
        let mut found: Vec<PairableDto> = device::discover_bluetooth_devices()
            .await
            .map_err(err)?
            .into_iter()
            .map(|d| PairableDto {
                model: device::model_for_device_name(&d.name).map(|m| m.name),
                id: d.id,
                name: d.name,
            })
            .collect();
        found.sort_by_key(|d| d.model.is_none());
        Ok(found)
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    Err(AppError::new(
        "unsupported",
        "Bluetooth pairing is only supported on Windows and Linux",
    ))
}

/// Pairs a device found by `discover_bluetooth`.
#[tauri::command]
async fn pair_bluetooth(id: String) -> Result<(), AppError> {
    #[cfg(any(windows, target_os = "linux"))]
    {
        device::pair_bluetooth(&id).await.map_err(err)
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        let _ = id;
        Err(AppError::new(
            "unsupported",
            "Bluetooth pairing is only supported on Windows and Linux",
        ))
    }
}

/// Size of the image shown in the image editor (longest side, px).
const IMAGE_EDITOR_PX: u32 = 640;

/// The image for the editor: rotated/flipped (no crop), once with the
/// background removed (`masked`) and once without (`plain`, for picking a
/// color), as base64 PNGs.
#[derive(Serialize)]
struct ImageEditorDto {
    masked: String,
    plain: String,
    width: u32,
    height: u32,
}

#[tauri::command]
fn image_editor_source(
    path: PathBuf,
    edit: ll_render::ImageEdit,
) -> Result<ImageEditorDto, AppError> {
    use ll_render::image_edit;
    let source = image_edit::load_rgba(&path, IMAGE_EDITOR_PX as u16)
        .map_err(|e| err(ll_core::CoreError::from(e)))?;
    let source = if source.width().max(source.height()) > IMAGE_EDITOR_PX {
        image::imageops::thumbnail(
            &source,
            IMAGE_EDITOR_PX * source.width() / source.width().max(source.height()),
            IMAGE_EDITOR_PX * source.height() / source.width().max(source.height()),
        )
    } else {
        source
    };
    let plain = image_edit::transform_and_mask(
        source.clone(),
        &ll_render::ImageEdit {
            background: None,
            ..edit
        },
    );
    let masked = image_edit::transform_and_mask(source, &edit);
    let png = |img: &image::RgbaImage| -> Result<String, AppError> {
        let mut out = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
            .map_err(|e| AppError::new("image", e.to_string()))?;
        Ok(base64::engine::general_purpose::STANDARD.encode(out))
    };
    Ok(ImageEditorDto {
        masked: png(&masked)?,
        plain: png(&plain)?,
        width: masked.width(),
        height: masked.height(),
    })
}

/// One print history entry with its preview (base64 PNG mask, may be empty).
#[derive(Serialize)]
struct HistoryDto {
    #[serde(flatten)]
    entry: ll_core::history::Entry,
    preview: String,
}

/// Print history, newest first.
#[tauri::command]
fn history() -> Result<Vec<HistoryDto>, AppError> {
    Ok(ll_core::history::list()
        .map_err(err)?
        .into_iter()
        .map(|entry| HistoryDto {
            preview: ll_core::history::preview(&entry.id)
                .map(|png| base64::engine::general_purpose::STANDARD.encode(png))
                .unwrap_or_default(),
            entry,
        })
        .collect())
}

/// Adds a printed label to the history.
#[tauri::command]
fn record_history(
    label: Label,
    model: String,
    width_mm: u8,
    name: String,
    count: usize,
) -> Result<(), AppError> {
    ll_core::history::record(&label, find_model(&model)?, width_mm, &name, count)
        .map(|_| ())
        .map_err(err)
}

/// The label stored with history entry `id`.
#[tauri::command]
fn load_history(id: String) -> Result<Label, AppError> {
    ll_core::history::load(&id).map_err(err)
}

/// An icon set for the symbol picker (all icons incl. SVG).
#[derive(Serialize)]
struct IconSetDto {
    #[serde(flatten)]
    set: ll_render::IconSet,
    builtin: bool,
}

fn iconset_dto(set: &ll_render::IconSet) -> IconSetDto {
    IconSetDto {
        builtin: ll_render::iconset::is_builtin(&set.id),
        set: set.clone(),
    }
}

/// All registered icon sets, built-in first.
#[tauri::command]
fn iconsets() -> Vec<IconSetDto> {
    ll_render::iconset::sets()
        .iter()
        .map(|s| iconset_dto(s))
        .collect()
}

/// Stores an image pasted from the clipboard (base64) and returns its path.
#[tauri::command]
fn save_pasted_image(data: String, extension: String) -> Result<PathBuf, AppError> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data)
        .map_err(|e| AppError::new("image", e.to_string()))?;
    ll_core::pasted::save_image(&bytes, &extension).map_err(err)
}

/// A pasted image stored in the data folder, with its size in pixels.
#[derive(Serialize)]
struct PastedImageDto {
    path: PathBuf,
    width: u32,
    height: u32,
}

/// Reads an image from the system clipboard (fallback where the webview's
/// paste event carries no image data) and stores it as PNG. `None` when
/// the clipboard holds no image.
#[tauri::command]
fn paste_clipboard_image(app: tauri::AppHandle) -> Result<Option<PastedImageDto>, AppError> {
    use tauri_plugin_clipboard_manager::ClipboardExt;
    let Ok(image) = app.clipboard().read_image() else {
        return Ok(None);
    };
    let (width, height) = (image.width(), image.height());
    let Some(rgba) = image::RgbaImage::from_raw(width, height, image.rgba().to_vec()) else {
        return Ok(None);
    };
    let mut png = Vec::new();
    rgba.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .map_err(|e| AppError::new("image", e.to_string()))?;
    let path = ll_core::pasted::save_image(&png, "png").map_err(err)?;
    Ok(Some(PastedImageDto {
        path,
        width,
        height,
    }))
}

/// Imports a `.llabel-iconset` file (kept in the user data folder).
#[tauri::command]
fn import_iconset(path: PathBuf) -> Result<IconSetDto, AppError> {
    let set = ll_core::iconsets::import(&path).map_err(err)?;
    Ok(iconset_dto(&set))
}

/// Removes an imported icon set.
#[tauri::command]
fn remove_iconset(id: String) -> Result<(), AppError> {
    ll_core::iconsets::remove(&id).map_err(err)
}

/// Builds a cable flag / cable wrap / patch panel label for the tape.
#[tauri::command]
fn generate_layout(layout: Layout, model: String, width_mm: u8) -> Result<Label, AppError> {
    let model = find_model(&model)?;
    let geometry = label::geometry_for(model, width_mm).map_err(err)?;
    Ok(layouts::generate(
        &layout,
        dots_to_mm(geometry.printable_pins as u32),
    ))
}

/// Default feed margin before the cut, for the GUI's initial value.
#[tauri::command]
fn default_margin_dots() -> u16 {
    PrintOptions::default().margin_dots
}

/// Opens a `.llabel` file (one or more sheets).
#[tauri::command]
fn load_document(path: PathBuf) -> Result<ll_core::document::Document, AppError> {
    ll_core::document::Document::load(&path).map_err(err)
}

/// Saves a document (a single sheet is written as a plain label).
#[tauri::command]
fn save_document(path: PathBuf, document: ll_core::document::Document) -> Result<(), AppError> {
    document.save(&path).map_err(err)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    for warning in ll_core::iconsets::load_installed() {
        eprintln!("icon set not loaded: {warning}");
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(SeriesState::default())
        .invoke_handler(tauri::generate_handler![
            models,
            render_preview,
            resolve_rects,
            list_devices,
            query_status,
            print_label,
            load_csv,
            clear_csv,
            font_families,
            iconsets,
            history,
            image_editor_source,
            save_pasted_image,
            paste_clipboard_image,
            record_history,
            load_history,
            import_iconset,
            remove_iconset,
            generate_layout,
            discover_bluetooth,
            pair_bluetooth,
            default_margin_dots,
            load_document,
            save_document,
        ])
        .run(tauri::generate_context!())
        .unwrap_or_else(|e| {
            eprintln!("error while running LabelLab: {e}");
            std::process::exit(1);
        });
}
