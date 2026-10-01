use clap::{Parser, Subcommand};
use ll_core::device;
use ll_core::label::{Element, Label};
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
        /// `.llabel`-Vorlage (JSON) statt Einzelinhalt.
        #[arg(long, conflicts_with_all = ["text", "qr", "barcode", "image"])]
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
        /// `.llabel`-Vorlage (JSON) statt Einzelinhalt.
        #[arg(long, conflicts_with_all = ["text", "qr", "barcode", "image"])]
        template: Option<String>,
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
            invert,
            frame,
            margin,
            template,
            csv,
            cut,
            copies,
            device,
            bt,
            usb,
            baud,
            model,
        } => {
            if csv.is_some() {
                eprintln!("--csv ist noch nicht implementiert (folgt in M7).");
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
                    template,
                },
                ll_core::print::PrintOptions {
                    frame,
                    auto_cut: cut,
                    margin_dots: margin,
                },
                copies,
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
            invert,
            frame,
            template,
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
                template,
            },
            frame,
            output,
            width,
            model,
        ),
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

/// Opens a transport by CLI args: `--usb` picks USB (device optional),
/// `--bt` native Bluetooth RFCOMM (Windows only), otherwise a serial/COM
/// port at `baud`.
async fn open_transport(connect: &ConnectOpts) -> anyhow::Result<Box<dyn Transport>> {
    if connect.usb {
        return Ok(Box::new(device::open_usb(connect.device.as_deref()).await?));
    }
    let Some(device) = connect.device.as_deref() else {
        eprintln!(
            "Bitte --device angeben (COM-Port oder mit --bt eine Geraete-ID aus `devices`), \
             oder --usb verwenden."
        );
        std::process::exit(1);
    };
    let (bt, baud) = (connect.bt, connect.baud);
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
            println!("  {name} ({id})");
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
    if connect.usb {
        let status = device::query_status_over_usb(connect.device.as_deref()).await?;
        return print_status(&status, json);
    }
    let (bt, baud) = (connect.bt, connect.baud);
    let Some(device) = connect.device else {
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

/// `text`/`--qr`/`--barcode`(`-type`)/`--image`/`--template` from
/// `print`/`render`, grouped so those commands don't need seven separate
/// parameters each (`clap`'s `conflicts_with_all` keeps more than one of
/// them from being set at once).
struct ContentArgs {
    text: Option<String>,
    qr: Option<String>,
    barcode: Option<String>,
    barcode_type: BarcodeType,
    image: Option<String>,
    invert: bool,
    template: Option<String>,
}

impl ContentArgs {
    /// Builds the label to render plus a short description for messages.
    /// `None` if no content was given.
    fn into_label(self) -> anyhow::Result<Option<(Label, String)>> {
        let single = |element, desc: &str| Some((Label::single(element), desc.to_owned()));
        Ok(
            match (self.text, self.qr, self.barcode, self.image, self.template) {
                (Some(t), None, None, None, None) => single(Element::Text { text: t.clone() }, &t),
                (None, Some(q), None, None, None) => single(Element::Qr { data: q.clone() }, &q),
                (None, None, Some(b), None, None) => single(
                    Element::Barcode {
                        symbology: self.barcode_type.into(),
                        data: b.clone(),
                    },
                    &b,
                ),
                (None, None, None, Some(i), None) => single(
                    Element::Image {
                        path: i.clone().into(),
                        invert: self.invert,
                    },
                    &i,
                ),
                (None, None, None, None, Some(t)) => {
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

async fn print(
    content_args: ContentArgs,
    options: ll_core::print::PrintOptions,
    copies: u32,
    connect: ConnectOpts,
    model_name: String,
) -> anyhow::Result<()> {
    let Some((label, description)) = content_args.into_label()? else {
        eprintln!(
            "Bitte Text, --qr, --barcode, --image oder --template angeben: labellab print \"Text\" --device <COM-Port oder BT-ID>"
        );
        std::process::exit(1);
    };
    let model = find_model(&model_name);

    let mut transport = open_transport(&connect).await?;
    for copy in 1..=copies {
        if copies > 1 {
            eprintln!("Drucke Kopie {copy}/{copies} ...");
        }
        ll_core::print::print_label(transport.as_mut(), model, &label, &options).await?;
    }
    transport.close().await?;

    println!("Gedruckt: \"{description}\" ({copies}x)");
    Ok(())
}

fn render(
    content_args: ContentArgs,
    frame: bool,
    output: String,
    width_mm: u8,
    model_name: String,
) -> anyhow::Result<()> {
    let Some((mut label, _)) = content_args.into_label()? else {
        eprintln!(
            "Bitte Text, --qr, --barcode, --image oder --template angeben: labellab render \"Text\" -o datei.png"
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
