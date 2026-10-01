use clap::{Parser, Subcommand};
use ll_core::device;
use ll_protocol::status::{StatusBlock, StatusType};

/// Default baud rate for serial/COM-port connections. Virtual Bluetooth-SPP
/// ports generally ignore it, but the OS API still requires a value.
const DEFAULT_BAUD_RATE: u32 = 9600;

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
    },
    /// Label ohne Drucker in eine PNG-Datei rendern.
    Render {
        template: String,
        #[arg(short, long)]
        output: String,
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
        Command::Print { .. } | Command::Render { .. } => {
            eprintln!("Noch nicht implementiert (folgt in M3/M5). Siehe docs/PROGRESS.md.");
            std::process::exit(1);
        }
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
