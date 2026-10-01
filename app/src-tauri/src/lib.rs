//! Tauri backend: thin command layer over `ll-core`. All rendering,
//! protocol and transport logic stays in the library crates; the preview
//! and the print job both go through `ll_core::label::render_label`
//! (`AGENTS.md`: "Vorschau und Druck nutzen denselben Renderpfad").
//!
//! Errors are returned to the frontend as plain strings; the frontend
//! shows them under a localized heading.

use std::path::PathBuf;

use ll_core::device::{self, Connection};
use ll_core::label::{self, Label};
use ll_core::print::PrintOptions;
use ll_protocol::model::{self, ModelInfo, MODELS};
use serde::Serialize;
use tauri::ipc::Response;

/// Baud rate for serial ports picked in the GUI. Virtual Bluetooth-SPP
/// ports ignore it, but the OS API needs one (same as the CLI default).
const SERIAL_BAUD_RATE: u32 = 9600;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

fn find_model(name: &str) -> Result<&'static ModelInfo, String> {
    model::find_by_name(name).ok_or_else(|| format!("unknown model {name:?}"))
}

#[derive(Serialize)]
struct ModelDto {
    name: &'static str,
    tape_widths: Vec<u8>,
}

/// Known printer models and their supported tape widths.
#[tauri::command]
fn models() -> Vec<ModelDto> {
    MODELS
        .iter()
        .map(|m| ModelDto {
            name: m.name,
            tape_widths: m.tape_geometries.iter().map(|g| g.width_mm).collect(),
        })
        .collect()
}

/// Renders `label` for a `width_mm` tape as PNG bytes (raw IPC response,
/// arrives as `ArrayBuffer` in the frontend).
#[tauri::command]
fn render_preview(label: Label, model: String, width_mm: u8) -> Result<Response, String> {
    let png = label::render_label_png(&label, find_model(&model)?, width_mm).map_err(err)?;
    Ok(Response::new(png))
}

#[derive(Serialize)]
struct DeviceDto {
    /// Human-readable name for the device picker.
    name: String,
    connection: Connection,
}

#[derive(Serialize)]
struct DeviceListDto {
    devices: Vec<DeviceDto>,
    /// Non-fatal enumeration failures (e.g. no USB access).
    warnings: Vec<String>,
}

/// Lists USB printers, paired Bluetooth printers (Windows) and serial
/// ports, in that order. One failing transport doesn't hide the others.
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
            }
        })),
        Err(e) => warnings.push(format!("USB: {e}")),
    }

    #[cfg(windows)]
    match device::list_bluetooth_devices() {
        Ok(bt) => devices.extend(bt.into_iter().map(|d| DeviceDto {
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
        })),
        Err(e) => warnings.push(format!("Serial: {e}")),
    }

    DeviceListDto { devices, warnings }
}

#[derive(Serialize)]
struct StatusDto {
    width_mm: u8,
    media_type: u8,
    tape_color: u8,
    text_color: u8,
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
async fn query_status(connection: Connection) -> Result<StatusDto, String> {
    let s = device::query_status_on(&connection).await.map_err(err)?;
    Ok(StatusDto {
        width_mm: s.media_width_mm(),
        media_type: s.media_type(),
        tape_color: s.tape_color(),
        text_color: s.text_color(),
        has_error: s.has_error(),
        error1: s.error1(),
        error2: s.error2(),
    })
}

/// Prints `label` `copies` times over `connection`.
#[tauri::command]
async fn print_label(
    label: Label,
    connection: Connection,
    model: String,
    copies: u32,
    auto_cut: bool,
    margin_dots: u16,
) -> Result<(), String> {
    let model = find_model(&model)?;
    let options = PrintOptions {
        frame: false,
        auto_cut,
        margin_dots,
    };
    let mut transport = device::connect(&connection).await.map_err(err)?;
    for _ in 0..copies.max(1) {
        ll_core::print::print_label(transport.as_mut(), model, &label, &options)
            .await
            .map_err(err)?;
    }
    transport.close().await.map_err(err)
}

/// Default feed margin before the cut, for the GUI's initial value.
#[tauri::command]
fn default_margin_dots() -> u16 {
    PrintOptions::default().margin_dots
}

#[tauri::command]
fn load_label(path: PathBuf) -> Result<Label, String> {
    Label::load(&path).map_err(err)
}

#[tauri::command]
fn save_label(path: PathBuf, label: Label) -> Result<(), String> {
    label.save(&path).map_err(err)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            models,
            render_preview,
            list_devices,
            query_status,
            print_label,
            default_margin_dots,
            load_label,
            save_label,
        ])
        .run(tauri::generate_context!())
        .unwrap_or_else(|e| {
            eprintln!("error while running LabelLab: {e}");
            std::process::exit(1);
        });
}
