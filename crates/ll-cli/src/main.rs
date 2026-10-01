use clap::{Parser, Subcommand};
use ll_core::device;
use ll_protocol::status::{StatusBlock, StatusType};
use ll_transport::Transport;

/// Default baud rate for serial/COM-port connections. Virtual Bluetooth-SPP
/// ports generally ignore it, but the OS API still requires a value.
const DEFAULT_BAUD_RATE: u32 = 9600;

/// Default model, looked up in `ll_protocol::model::MODELS`.
const DEFAULT_MODEL: &str = "PT-P710BT";

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
        #[arg(long, conflicts_with = "text")]
        qr: Option<String>,
        #[arg(long)]
        template: Option<String>,
        #[arg(long)]
        csv: Option<String>,
        #[arg(long)]
        image: Option<String>,
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
        #[arg(long, conflicts_with = "text")]
        qr: Option<String>,
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
            template,
            csv,
            image,
            cut,
            copies,
            device,
            bt,
            baud,
            model,
        } => {
            if template.is_some() || csv.is_some() || image.is_some() {
                eprintln!(
                    "--template/--csv/--image sind noch nicht implementiert (folgen in M5/M7)."
                );
                std::process::exit(1);
            }
            print(
                text,
                qr,
                cut,
                copies,
                ConnectOpts { device, bt, baud },
                model,
            )
            .await
        }
        Command::Render {
            text,
            qr,
            output,
            width,
            model,
        } => render(text, qr, output, width, model),
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

/// What to render: plain text or a QR code. `print`/`render` both take
/// either `text` or `--qr`, never both (`clap`'s `conflicts_with`).
enum Content {
    Text(String),
    Qr(String),
}

impl Content {
    fn from_args(text: Option<String>, qr: Option<String>) -> Option<Self> {
        match (text, qr) {
            (Some(t), None) => Some(Content::Text(t)),
            (None, Some(q)) => Some(Content::Qr(q)),
            _ => None,
        }
    }

    fn label(&self) -> &str {
        match self {
            Content::Text(t) => t,
            Content::Qr(d) => d,
        }
    }
}

async fn print(
    text: Option<String>,
    qr: Option<String>,
    cut: bool,
    copies: u32,
    connect: ConnectOpts,
    model_name: String,
) -> anyhow::Result<()> {
    let Some(content) = Content::from_args(text, qr) else {
        eprintln!(
            "Bitte Text oder --qr angeben: labellab print \"Text\" --device <COM-Port oder BT-ID>"
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
                ll_core::print::print_text(transport.as_mut(), model, text, cut).await?
            }
            Content::Qr(data) => {
                ll_core::print::print_qr(
                    transport.as_mut(),
                    model,
                    data,
                    ll_render::QrErrorCorrection::Medium,
                    cut,
                )
                .await?
            }
        }
    }
    transport.close().await?;

    println!("Gedruckt: \"{}\" ({copies}x)", content.label());
    Ok(())
}

fn render(
    text: Option<String>,
    qr: Option<String>,
    output: String,
    width_mm: u8,
    model_name: String,
) -> anyhow::Result<()> {
    let Some(content) = Content::from_args(text, qr) else {
        eprintln!("Bitte Text oder --qr angeben: labellab render \"Text\" -o datei.png");
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

    let bitmap = match &content {
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
    };
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
