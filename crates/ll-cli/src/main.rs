use clap::{Parser, Subcommand};
use ll_core::device;
use ll_protocol::status::{StatusBlock, StatusType};
use ll_transport::Transport;

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
    /// Gefundene Drucker (aktuell: serielle/COM-Ports) auflisten.
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
        #[arg(long, default_value_t = DEFAULT_BAUD_RATE)]
        baud: u32,
        #[arg(long)]
        json: bool,
    },
    /// Textlabel drucken.
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
        /// Bilddatei (PNG/JPEG/BMP) statt Text drucken, skaliert auf die
        /// Bandbreite, Floyd-Steinberg-gedithert.
        #[arg(long, conflicts_with_all = ["text", "qr", "barcode"])]
        image: Option<String>,
        /// Bild invertieren (nur mit --image).
        #[arg(long)]
        invert: bool,
        /// Rahmen um das ganze Label zeichnen.
        #[arg(long)]
        frame: bool,
        /// Leervorschub vor dem Schnitt, in Druckpunkten (180 dpi). `0`
        /// schneidet direkt am letzten bedruckten Punkt.
        #[arg(long, default_value_t = ll_core::print::PrintOptions::default().margin_dots)]
        margin: u16,
        #[arg(long)]
        template: Option<String>,
        #[arg(long)]
        csv: Option<String>,
        #[arg(long)]
        cut: bool,
        #[arg(long, default_value_t = 1)]
        copies: u32,
        #[arg(long)]
        device: Option<String>,
        /// Natives Bluetooth RFCOMM statt seriellem COM-Port verwenden
        /// (nur Windows; `device` ist dann die Geraete-ID aus `devices`).
        #[arg(long)]
        bt: bool,
        #[arg(long, default_value_t = DEFAULT_BAUD_RATE)]
        baud: u32,
        #[arg(long, default_value = DEFAULT_MODEL)]
        model: String,
    },
    /// Text ohne Drucker in eine PNG-Datei rendern (Vorschau).
    ///
    /// Provisorisch: nimmt reinen Text statt eines `.llabel`-Vorlagenformats
    /// (das kommt erst mit dem GUI-Editor in M6) — daher `--width` statt
    /// einer live abgefragten Bandbreite.
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
        /// Bilddatei (PNG/JPEG/BMP) statt Text rendern, skaliert auf die
        /// Bandbreite, Floyd-Steinberg-gedithert.
        #[arg(long, conflicts_with_all = ["text", "qr", "barcode"])]
        image: Option<String>,
        /// Bild invertieren (nur mit --image).
        #[arg(long)]
        invert: bool,
        /// Rahmen um das ganze Label zeichnen.
        #[arg(long)]
        frame: bool,
        #[arg(short, long)]
        output: String,
        /// Bandbreite in mm (kein Drucker verbunden, daher nicht automatisch
        /// erkennbar).
        #[arg(long, default_value_t = 12)]
        width: u8,
        #[arg(long, default_value = DEFAULT_MODEL)]
        model: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Devices { json } => devices(json),
        Command::Status {
            device,
            bt,
            baud,
            json,
        } => status(device, bt, baud, json).await,
        Command::Print {
            text,
            qr,
            barcode,
            barcode_type,
            image,
            invert,
            frame,
            margin,
            template,
            csv,
            cut,
            copies,
            device,
            bt,
            baud,
            model,
        } => {
            if template.is_some() || csv.is_some() {
                eprintln!("--template/--csv sind noch nicht implementiert (folgt in M7).");
                std::process::exit(1);
            }
            print(
                ContentArgs {
                    text,
                    qr,
                    barcode,
                    barcode_type,
                    image,
                    invert,
                },
                ll_core::print::PrintOptions {
                    frame,
                    auto_cut: cut,
                    margin_dots: margin,
                },
                copies,
                ConnectOpts { device, bt, baud },
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
            invert,
            frame,
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
                invert,
            },
            frame,
            output,
            width,
            model,
        ),
    }
}

/// `--device`/`--bt`/`--baud`, grouped so `print`/`status` don't need a
/// handful of separate parameters each (keeps `clippy::too_many_arguments`
/// happy too).
struct ConnectOpts {
    device: Option<String>,
    bt: bool,
    baud: u32,
}

/// Opens a transport by CLI args: `--bt` picks native Bluetooth RFCOMM
/// (Windows only), otherwise a serial/COM port at `baud`.
async fn open_transport(device: &str, bt: bool, baud: u32) -> anyhow::Result<Box<dyn Transport>> {
    if bt {
        #[cfg(windows)]
        {
            Ok(Box::new(
                ll_transport::bluetooth::BluetoothTransport::connect(device).await?,
            ))
        }
        #[cfg(not(windows))]
        {
            let _ = device;
            anyhow::bail!(
                "Natives Bluetooth ist unter Linux noch nicht implementiert (BlueZ folgt)."
            );
        }
    } else {
        Ok(Box::new(ll_transport::serial::SerialTransport::open(
            device, baud,
        )?))
    }
}

fn devices(json: bool) -> anyhow::Result<()> {
    let ports = device::list_serial_devices()?;

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
        let value = serde_json::json!({ "serial": ports, "bluetooth": bt_json });
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
            println!("  {name} ({id})");
        }
    }
    Ok(())
}

async fn status(device: Option<String>, bt: bool, baud: u32, json: bool) -> anyhow::Result<()> {
    let Some(device) = device else {
        eprintln!(
            "Bitte --device angeben (COM-Port oder mit --bt eine Geraete-ID aus `devices`). \
             Automatische Erkennung folgt später."
        );
        std::process::exit(1);
    };

    let status = if bt {
        #[cfg(windows)]
        {
            device::query_status_over_bluetooth(&device).await?
        }
        #[cfg(not(windows))]
        {
            eprintln!("Natives Bluetooth ist unter Linux noch nicht implementiert (BlueZ folgt).");
            std::process::exit(1);
        }
    } else {
        device::query_status_over_serial(&device, baud).await?
    };

    if json {
        println!("{}", status_to_json(&status));
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

/// `text`/`--qr`/`--barcode`(`-type`)/`--image` from `print`/`render`,
/// grouped so those commands don't need six separate parameters each
/// (`clap`'s `conflicts_with_all` keeps more than one of them from being
/// set at once).
struct ContentArgs {
    text: Option<String>,
    qr: Option<String>,
    barcode: Option<String>,
    barcode_type: BarcodeType,
    image: Option<String>,
    invert: bool,
}

/// What to render: plain text, a QR code, a linear barcode or an image.
enum Content {
    Text(String),
    Qr(String),
    Barcode(ll_render::Symbology, String),
    Image(std::path::PathBuf, bool),
}

impl Content {
    fn from_args(args: ContentArgs) -> Option<Self> {
        match (args.text, args.qr, args.barcode, args.image) {
            (Some(t), None, None, None) => Some(Content::Text(t)),
            (None, Some(q), None, None) => Some(Content::Qr(q)),
            (None, None, Some(b), None) => Some(Content::Barcode(args.barcode_type.into(), b)),
            (None, None, None, Some(i)) => Some(Content::Image(i.into(), args.invert)),
            _ => None,
        }
    }

    fn label(&self) -> String {
        match self {
            Content::Text(t) => t.clone(),
            Content::Qr(d) => d.clone(),
            Content::Barcode(_, d) => d.clone(),
            Content::Image(p, _) => p.display().to_string(),
        }
    }
}

async fn print(
    content_args: ContentArgs,
    options: ll_core::print::PrintOptions,
    copies: u32,
    connect: ConnectOpts,
    model_name: String,
) -> anyhow::Result<()> {
    let Some(content) = Content::from_args(content_args) else {
        eprintln!(
            "Bitte Text, --qr, --barcode oder --image angeben: labellab print \"Text\" --device <COM-Port oder BT-ID>"
        );
        std::process::exit(1);
    };
    let Some(device) = connect.device else {
        eprintln!("Bitte --device angeben (COM-Port oder mit --bt eine Geraete-ID aus `devices`).");
        std::process::exit(1);
    };
    let Some(model) = ll_protocol::model::find_by_name(&model_name) else {
        eprintln!(
            "Unbekanntes Modell '{model_name}'. Bekannt: {}",
            ll_protocol::model::MODELS
                .iter()
                .map(|m| m.name)
                .collect::<Vec<_>>()
                .join(", ")
        );
        std::process::exit(1);
    };

    let mut transport = open_transport(&device, connect.bt, connect.baud).await?;

    for copy in 1..=copies {
        if copies > 1 {
            eprintln!("Drucke Kopie {copy}/{copies} ...");
        }
        match &content {
            Content::Text(text) => {
                ll_core::print::print_text(transport.as_mut(), model, text, &options).await?
            }
            Content::Qr(data) => {
                ll_core::print::print_qr(
                    transport.as_mut(),
                    model,
                    data,
                    ll_render::QrErrorCorrection::Medium,
                    &options,
                )
                .await?
            }
            Content::Barcode(symbology, data) => {
                ll_core::print::print_barcode(transport.as_mut(), model, *symbology, data, &options)
                    .await?
            }
            Content::Image(path, invert) => {
                ll_core::print::print_image(transport.as_mut(), model, path, *invert, &options)
                    .await?
            }
        }
    }
    transport.close().await?;

    println!("Gedruckt: \"{}\" ({copies}x)", content.label());
    Ok(())
}

fn render(
    content_args: ContentArgs,
    frame: bool,
    output: String,
    width_mm: u8,
    model_name: String,
) -> anyhow::Result<()> {
    let Some(content) = Content::from_args(content_args) else {
        eprintln!(
            "Bitte Text, --qr, --barcode oder --image angeben: labellab render \"Text\" -o datei.png"
        );
        std::process::exit(1);
    };
    let Some(model) = ll_protocol::model::find_by_name(&model_name) else {
        eprintln!(
            "Unbekanntes Modell '{model_name}'. Bekannt: {}",
            ll_protocol::model::MODELS
                .iter()
                .map(|m| m.name)
                .collect::<Vec<_>>()
                .join(", ")
        );
        std::process::exit(1);
    };
    let Some(geometry) = model
        .tape_geometries
        .iter()
        .find(|g| g.width_mm == width_mm)
    else {
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
    };

    let mut bitmap = match &content {
        Content::Text(text) => ll_render::render_text(
            text,
            model.head_pins,
            geometry.printable_pins,
            geometry.left_offset_pins,
        )?,
        Content::Qr(data) => ll_render::render_qr(
            data,
            model.head_pins,
            geometry.printable_pins,
            geometry.left_offset_pins,
            ll_render::QrErrorCorrection::Medium,
        )?,
        Content::Barcode(symbology, data) => ll_render::render_barcode(
            *symbology,
            data,
            model.head_pins,
            geometry.printable_pins,
            geometry.left_offset_pins,
        )?,
        Content::Image(path, invert) => ll_render::render_image(
            path,
            model.head_pins,
            geometry.printable_pins,
            geometry.left_offset_pins,
            *invert,
        )?,
    };
    if frame {
        ll_render::draw_border(
            &mut bitmap,
            geometry.left_offset_pins,
            geometry.printable_pins,
            2,
        );
    }
    let png = ll_render::png::to_png(&bitmap, geometry.left_offset_pins, geometry.printable_pins)?;
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
