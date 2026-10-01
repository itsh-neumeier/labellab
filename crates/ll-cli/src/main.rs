use clap::{Parser, Subcommand};
use ll_core::device;
use std::ops::RangeInclusive;
use std::path::Path;

use ll_core::label::{Element, Label};
use ll_core::series::{self, DataSet};
use ll_protocol::status::{StatusBlock, StatusType};

/// Default baud rate for serial/COM-port connections. Virtual Bluetooth-SPP
/// ports generally ignore it, but the OS API still requires a value.
const DEFAULT_BAUD_RATE: u32 = 9600;

/// Default model, looked up in `ll_protocol::model::MODELS`.
const DEFAULT_MODEL: &str = "PT-P710BT";

/// `clap`-friendly mirror of `ll_render::Symbology` (can't derive
/// `clap::ValueEnum` on a foreign type).
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum BarcodeType {
    Code128,
    Ean13,
    Ean8,
    UpcA,
    Code39,
    Itf,
}

impl From<BarcodeType> for ll_render::Symbology {
    fn from(value: BarcodeType) -> Self {
        match value {
            BarcodeType::Code128 => ll_render::Symbology::Code128,
            BarcodeType::Ean13 => ll_render::Symbology::Ean13,
            BarcodeType::Ean8 => ll_render::Symbology::Ean8,
            BarcodeType::UpcA => ll_render::Symbology::UpcA,
            BarcodeType::Code39 => ll_render::Symbology::Code39,
            BarcodeType::Itf => ll_render::Symbology::Itf,
        }
    }
}

#[derive(Parser)]
#[command(
    name = "labellab",
    version,
    about = "Direktdruck fuer Brother PT-P7xx ohne Treiber"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Gefundene Drucker (COM-Ports, gekoppeltes Bluetooth, USB) auflisten.
    Devices {
        #[arg(long)]
        json: bool,
    },
    /// Bandstatus abfragen.
    Status {
        #[arg(long)]
        device: Option<String>,
        /// Natives Bluetooth RFCOMM statt seriellem COM-Port verwenden
        /// (nur Windows; `device` ist dann die Geraete-ID aus `devices`).
        #[arg(long)]
        bt: bool,
        /// USB statt seriellem COM-Port verwenden. `device` ist dann optional:
        /// Modellname, `VVVV:PPPP` oder Seriennummer (ohne: erster gefundener).
        #[arg(long, conflicts_with = "bt")]
        usb: bool,
        #[arg(long, default_value_t = DEFAULT_BAUD_RATE)]
        baud: u32,
        #[arg(long)]
        json: bool,
    },
    /// Label drucken (Text, QR, Barcode, Bild oder `.llabel`-Vorlage).
    Print {
        text: Option<String>,
        /// QR-Code statt Text drucken (Daten für den Code, z. B. eine URL).
        #[arg(long, conflicts_with_all = ["text", "barcode"])]
        qr: Option<String>,
        /// Barcode statt Text drucken (Symbologie mit --barcode-type).
        #[arg(long, conflicts_with_all = ["text", "qr"])]
        barcode: Option<String>,
        /// Barcode-Symbologie (nur mit --barcode).
        #[arg(long, value_enum, default_value = "code128")]
        barcode_type: BarcodeType,
        /// Bilddatei (PNG/JPEG/BMP/SVG) statt Text drucken, skaliert auf
        /// die Bandbreite, Floyd-Steinberg-gedithert.
        #[arg(long, conflicts_with_all = ["text", "qr", "barcode", "symbol"])]
        image: Option<String>,
        /// Mitgeliefertes Symbol statt Text drucken (siehe `labellab symbols`).
        #[arg(long, conflicts_with_all = ["text", "qr", "barcode", "image", "symbol"])]
        symbol: Option<String>,
        /// Bild/Symbol invertieren (nur mit --image/--symbol).
        #[arg(long)]
        invert: bool,
        /// Rahmen um das ganze Label zeichnen.
        #[arg(long)]
        frame: bool,
        /// Leervorschub vor dem Schnitt, in Druckpunkten (180 dpi). `0`
        /// schneidet direkt am letzten bedruckten Punkt.
        #[arg(long, default_value_t = ll_core::print::PrintOptions::default().margin_dots)]
        margin: u16,
        /// `.llabel`-Vorlage (JSON) statt Einzelinhalt.
        #[arg(long, conflicts_with_all = ["text", "qr", "barcode", "image", "symbol"])]
        template: Option<String>,
        /// CSV-Datei für Serien: Platzhalter `{{Spalte}}` und `{{#}}` (Nummer)
        /// in der Vorlage werden je Datensatz ersetzt, ein Label pro Datensatz.
        #[arg(long, requires = "template")]
        csv: Option<String>,
        /// Nur diese Datensätze drucken (1-basiert): `5`, `1-10`, `3-`.
        #[arg(long, requires = "csv")]
        rows: Option<String>,
        /// Abschneiden (ohne --chain nach jedem Label, mit --chain nur am Ende).
        #[arg(long)]
        cut: bool,
        /// Fortlaufend drucken: alle Labels (Serie, Kopien, Mehrband-Streifen)
        /// in einem Auftrag ohne Schnitt dazwischen.
        #[arg(long)]
        chain: bool,
        #[arg(long, default_value_t = 1)]
        copies: u32,
        #[arg(long)]
        device: Option<String>,
        /// Natives Bluetooth RFCOMM statt seriellem COM-Port verwenden
        /// (nur Windows; `device` ist dann die Geraete-ID aus `devices`).
        #[arg(long)]
        bt: bool,
        /// USB statt seriellem COM-Port verwenden. `device` ist dann optional:
        /// Modellname, `VVVV:PPPP` oder Seriennummer (ohne: erster gefundener).
        #[arg(long, conflicts_with = "bt")]
        usb: bool,
        #[arg(long, default_value_t = DEFAULT_BAUD_RATE)]
        baud: u32,
        #[arg(long, default_value = DEFAULT_MODEL)]
        model: String,
    },
    /// Label ohne Drucker in eine PNG-Datei rendern (Vorschau). `--width`
    /// statt live abgefragter Bandbreite, da kein Drucker verbunden ist.
    Render {
        text: Option<String>,
        /// QR-Code statt Text rendern (Daten für den Code, z. B. eine URL).
        #[arg(long, conflicts_with_all = ["text", "barcode"])]
        qr: Option<String>,
        /// Barcode statt Text rendern (Symbologie mit --barcode-type).
        #[arg(long, conflicts_with_all = ["text", "qr"])]
        barcode: Option<String>,
        /// Barcode-Symbologie (nur mit --barcode).
        #[arg(long, value_enum, default_value = "code128")]
        barcode_type: BarcodeType,
        /// Bilddatei (PNG/JPEG/BMP/SVG) statt Text rendern, skaliert auf
        /// die Bandbreite, Floyd-Steinberg-gedithert.
        #[arg(long, conflicts_with_all = ["text", "qr", "barcode", "symbol"])]
        image: Option<String>,
        /// Mitgeliefertes Symbol statt Text rendern (siehe `labellab symbols`).
        #[arg(long, conflicts_with_all = ["text", "qr", "barcode", "image", "symbol"])]
        symbol: Option<String>,
        /// Bild/Symbol invertieren (nur mit --image/--symbol).
        #[arg(long)]
        invert: bool,
        /// Rahmen um das ganze Label zeichnen.
        #[arg(long)]
        frame: bool,
        /// `.llabel`-Vorlage (JSON) statt Einzelinhalt.
        #[arg(long, conflicts_with_all = ["text", "qr", "barcode", "image", "symbol"])]
        template: Option<String>,
        /// CSV-Datei: Platzhalter mit Datensatz `--row` füllen.
        #[arg(long, requires = "template")]
        csv: Option<String>,
        /// Datensatz für die Vorschau (1-basiert).
        #[arg(long, default_value_t = 1, requires = "csv")]
        row: usize,
        #[arg(short, long)]
        output: String,
        /// Bandbreite in mm (kein Drucker verbunden, daher nicht automatisch
        /// erkennbar).
        #[arg(long, default_value_t = 12)]
        width: u8,
        #[arg(long, default_value = DEFAULT_MODEL)]
        model: String,
    },
    /// Mitgelieferte Symbole auflisten (für `print --symbol`/`render --symbol`).
    Symbols,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Devices { json } => devices(json).await,
        Command::Status {
            device,
            bt,
            usb,
            baud,
            json,
        } => {
            status(
                ConnectOpts {
                    device,
                    bt,
                    usb,
                    baud,
                },
                json,
            )
            .await
        }
        Command::Print {
            text,
            qr,
            barcode,
            barcode_type,
            image,
            symbol,
            invert,
            frame,
            margin,
            template,
            csv,
            rows,
            cut,
            chain,
            copies,
            device,
            bt,
            usb,
            baud,
            model,
        } => {
            print(
                ContentArgs {
                    text,
                    qr,
                    barcode,
                    barcode_type,
                    image,
                    symbol,
                    invert,
                    template,
                },
                ll_core::print::PrintOptions {
                    frame,
                    auto_cut: cut,
                    chain,
                    margin_dots: margin,
                },
                copies,
                series_args(csv, rows.as_deref())?,
                ConnectOpts {
                    device,
                    bt,
                    usb,
                    baud,
                },
                model,
            )
            .await
        }
        Command::Render {
            text,
            qr,
            barcode,
            barcode_type,
            image,
            symbol,
            invert,
            frame,
            template,
            csv,
            row,
            output,
            width,
            model,
        } => render(
            ContentArgs {
                text,
                qr,
                barcode,
                barcode_type,
                image,
                symbol,
                invert,
                template,
            },
            frame,
            series_args(csv, Some(&row.to_string()))?,
            output,
            width,
            model,
        ),
        Command::Symbols => {
            println!("Mitgelieferte Symbole:");
            for name in ll_render::SYMBOL_NAMES {
                println!("  {name}");
            }
            Ok(())
        }
    }
}

/// `--device`/`--bt`/`--usb`/`--baud`, grouped so `print`/`status` don't need a
/// handful of separate parameters each (keeps `clippy::too_many_arguments`
/// happy too).
struct ConnectOpts {
    device: Option<String>,
    bt: bool,
    usb: bool,
    baud: u32,
}

impl ConnectOpts {
    /// Maps CLI flags to a [`device::Connection`]: `--usb` (device
    /// optional), `--bt` (native Bluetooth), otherwise a serial port.
    /// Exits with a hint if a required `--device` is missing.
    fn into_connection(self) -> device::Connection {
        if self.usb {
            return device::Connection::Usb { spec: self.device };
        }
        let Some(device) = self.device else {
            eprintln!(
                "Bitte --device angeben (COM-Port oder mit --bt eine Geraete-ID aus `devices`), \
                 oder --usb verwenden."
            );
            std::process::exit(1);
        };
        if self.bt {
            device::Connection::Bluetooth { device_id: device }
        } else {
            device::Connection::Serial {
                port: device,
                baud_rate: self.baud,
            }
        }
    }
}

async fn devices(json: bool) -> anyhow::Result<()> {
    let ports = device::list_serial_devices()?;
    // USB enumeration can fail where the other transports still work (no
    // USB subsystem, missing permissions); report it instead of aborting.
    let usb_printers = device::list_usb_printers().await.unwrap_or_else(|e| {
        eprintln!("USB-Geräte konnten nicht aufgelistet werden: {e}");
        Vec::new()
    });

    #[cfg(windows)]
    let bt_devices: Vec<(String, String)> = device::list_bluetooth_devices()?
        .into_iter()
        .map(|d| (d.id, d.name))
        .collect();
    #[cfg(not(windows))]
    let bt_devices: Vec<(String, String)> = Vec::new();

    if json {
        let bt_json: Vec<_> = bt_devices
            .iter()
            .map(|(id, name)| serde_json::json!({"id": id, "name": name}))
            .collect();
        let usb_json: Vec<_> = usb_printers
            .iter()
            .map(|p| {
                serde_json::json!({
                    "model": p.model.name,
                    "id": p.usb_id(),
                    "serial_number": p.serial_number,
                })
            })
            .collect();
        let value = serde_json::json!({ "serial": ports, "bluetooth": bt_json, "usb": usb_json });
        println!("{}", serde_json::to_string_pretty(&value)?);
        return Ok(());
    }

    if ports.is_empty() {
        println!("Keine seriellen Geräte gefunden.");
    } else {
        println!("Serielle Geräte (COM-Ports, inkl. Bluetooth-SPP):");
        for port in ports {
            println!("  {port}");
        }
    }

    if bt_devices.is_empty() {
        println!("Keine gekoppelten Bluetooth-SPP-Geräte gefunden.");
    } else {
        println!("Gekoppelte Bluetooth-Geräte (SPP), mit --bt verwendbar:");
        for (id, name) in bt_devices {
            let model = device::model_for_device_name(&name)
                .map(|m| format!(" – Drucker {}", m.name))
                .unwrap_or_default();
            println!("  {name}{model}  (--bt --device \"{name}\")");
            println!("      ID: {id}");
        }
    }

    if usb_printers.is_empty() {
        println!("Keine USB-Drucker gefunden.");
    } else {
        println!("USB-Drucker, mit --usb verwendbar:");
        for p in usb_printers {
            match &p.serial_number {
                Some(serial) => println!("  {} ({}, SN {serial})", p.model.name, p.usb_id()),
                None => println!("  {} ({})", p.model.name, p.usb_id()),
            }
        }
    }
    Ok(())
}

async fn status(connect: ConnectOpts, json: bool) -> anyhow::Result<()> {
    let status = device::query_status_on(&connect.into_connection()).await?;
    print_status(&status, json)
}

fn print_status(status: &StatusBlock, json: bool) -> anyhow::Result<()> {
    if json {
        println!("{}", status_to_json(status));
        return Ok(());
    }

    println!("Bandbreite: {} mm", status.media_width_mm());
    println!("Status: {}", status_type_text(status.status_type()));
    if status.has_error() {
        println!(
            "Fehler (Rohbytes, Bedeutung unverifiziert): error1={:#04x} error2={:#04x}",
            status.error1(),
            status.error2()
        );
    }
    Ok(())
}

/// `text`/`--qr`/`--barcode`(`-type`)/`--image`/`--symbol`/`--template` from
/// `print`/`render`, grouped so those commands don't need seven separate
/// parameters each (`clap`'s `conflicts_with_all` keeps more than one of
/// them from being set at once).
struct ContentArgs {
    text: Option<String>,
    qr: Option<String>,
    barcode: Option<String>,
    barcode_type: BarcodeType,
    image: Option<String>,
    symbol: Option<String>,
    invert: bool,
    template: Option<String>,
}

impl ContentArgs {
    /// Builds the label to render plus a short description for messages.
    /// `None` if no content was given.
    fn into_label(self) -> anyhow::Result<Option<(Label, String)>> {
        let single = |element, desc: &str| Some((Label::single(element), desc.to_owned()));
        Ok(
            match (
                self.text,
                self.qr,
                self.barcode,
                self.image,
                self.symbol,
                self.template,
            ) {
                (Some(t), None, None, None, None, None) => single(Element::text(t.clone()), &t),
                (None, Some(q), None, None, None, None) => {
                    single(Element::Qr { data: q.clone() }, &q)
                }
                (None, None, Some(b), None, None, None) => single(
                    Element::Barcode {
                        symbology: self.barcode_type.into(),
                        data: b.clone(),
                    },
                    &b,
                ),
                (None, None, None, Some(i), None, None) => single(
                    Element::Image {
                        path: i.clone().into(),
                        invert: self.invert,
                    },
                    &i,
                ),
                (None, None, None, None, Some(name), None) => single(
                    Element::Symbol {
                        name: name.clone(),
                        invert: self.invert,
                    },
                    &name,
                ),
                (None, None, None, None, None, Some(t)) => {
                    Some((Label::load(std::path::Path::new(&t))?, t))
                }
                _ => None,
            },
        )
    }
}

fn find_model(model_name: &str) -> &'static ll_protocol::model::ModelInfo {
    if let Some(model) = ll_protocol::model::find_by_name(model_name) {
        return model;
    }
    eprintln!(
        "Unbekanntes Modell '{model_name}'. Bekannt: {}",
        ll_protocol::model::MODELS
            .iter()
            .map(|m| m.name)
            .collect::<Vec<_>>()
            .join(", ")
    );
    std::process::exit(1);
}

/// `--csv`/`--rows`: the data set and the selected record numbers.
struct Series {
    data: DataSet,
    rows: RangeInclusive<usize>,
}

/// Parses `5`, `1-10` or `3-` (1-based, inclusive).
fn parse_rows(spec: &str) -> anyhow::Result<RangeInclusive<usize>> {
    let num = |s: &str| -> anyhow::Result<usize> {
        s.trim()
            .parse()
            .map_err(|_| anyhow::anyhow!("ungültige Zeilenangabe '{spec}' (z. B. 5, 1-10, 3-)"))
    };
    Ok(match spec.split_once('-') {
        None => num(spec)?..=num(spec)?,
        Some((a, b)) if b.trim().is_empty() => num(a)?..=usize::MAX,
        Some((a, b)) => num(a)?..=num(b)?,
    })
}

fn series_args(csv: Option<String>, rows: Option<&str>) -> anyhow::Result<Option<Series>> {
    let Some(path) = csv else { return Ok(None) };
    let data = DataSet::load(Path::new(&path))?;
    let rows = data.select(rows.map(parse_rows).transpose()?);
    if rows.is_empty() {
        anyhow::bail!(
            "Keine Datensätze im gewählten Bereich ({} vorhanden).",
            data.rows.len()
        );
    }
    Ok(Some(Series { data, rows }))
}

async fn print(
    content_args: ContentArgs,
    options: ll_core::print::PrintOptions,
    copies: u32,
    series: Option<Series>,
    connect: ConnectOpts,
    model_name: String,
) -> anyhow::Result<()> {
    let Some((label, description)) = content_args.into_label()? else {
        eprintln!(
            "Bitte Text, --qr, --barcode, --image, --symbol oder --template angeben: labellab print \"Text\" --device <COM-Port oder BT-ID>"
        );
        std::process::exit(1);
    };
    let model = find_model(&model_name);

    let labels: Vec<Label> = match &series {
        Some(s) => s
            .rows
            .clone()
            .map(|n| series::apply(&label, &s.data, n))
            .collect(),
        None => vec![label],
    };
    let mut transport = device::connect(&connect.into_connection()).await?;
    let mut printed = 0;
    ll_core::print::print_labels(
        transport.as_mut(),
        model,
        &labels,
        copies,
        &options,
        &mut |done, total| {
            printed = total;
            if total > 1 && done > 0 {
                eprintln!("Label {done}/{total} gesendet");
            }
        },
    )
    .await?;
    transport.close().await?;

    let total = printed;
    println!("Gedruckt: \"{description}\" ({total} Label)");
    Ok(())
}

fn render(
    content_args: ContentArgs,
    frame: bool,
    series: Option<Series>,
    output: String,
    width_mm: u8,
    model_name: String,
) -> anyhow::Result<()> {
    let Some((mut label, _)) = content_args.into_label()? else {
        eprintln!(
            "Bitte Text, --qr, --barcode, --image, --symbol oder --template angeben: labellab render \"Text\" -o datei.png"
        );
        std::process::exit(1);
    };
    let model = find_model(&model_name);
    if model.tape_geometries.iter().all(|g| g.width_mm != width_mm) {
        eprintln!(
            "Bandbreite {width_mm} mm nicht bekannt für {model_name}. Bekannt: {}",
            model
                .tape_geometries
                .iter()
                .map(|g| g.width_mm.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
        std::process::exit(1);
    }

    if let Some(s) = &series {
        label = series::apply(&label, &s.data, *s.rows.start());
    }
    label.frame |= frame;
    let png = ll_core::label::render_label_png(&label, model, width_mm)?;
    std::fs::write(&output, png)?;

    println!("Geschrieben: {output}");
    Ok(())
}

fn status_type_text(status_type: StatusType) -> &'static str {
    match status_type {
        StatusType::Reply => "Antwort",
        StatusType::PrintingComplete => "Druck fertig",
        StatusType::Error => "Fehler",
        StatusType::PhaseChange => "Phasenwechsel",
        StatusType::Unknown(_) => "unbekannt",
    }
}

fn status_to_json(status: &StatusBlock) -> String {
    let value = serde_json::json!({
        "media_width_mm": status.media_width_mm(),
        "media_type": status.media_type(),
        "status_type": status_type_text(status.status_type()),
        "phase_type": status.phase_type(),
        "has_error": status.has_error(),
        "error1": status.error1(),
        "error2": status.error2(),
        "tape_color": status.tape_color(),
        "text_color": status.text_color(),
    });
    serde_json::to_string_pretty(&value).unwrap_or_default()
}
