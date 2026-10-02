use clap::{Parser, Subcommand};
use ll_core::device;
use std::ops::RangeInclusive;
use std::path::Path;

use ll_core::label::{Element, Label};
use ll_core::layouts::{
    self, CableFlag, CableWrap, FuseBox, Layout, PatchPanel, SingleFlag, TerminalBlock,
};
use ll_core::series::{self, DataSet, Numbering};
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

/// `clap`-friendly mirror of `ll_render::BorderStyle`.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum BorderStyleArg {
    Solid,
    Dashed,
    Dotted,
    Double,
    Striped,
}

impl From<BorderStyleArg> for ll_render::BorderStyle {
    fn from(value: BorderStyleArg) -> Self {
        match value {
            BorderStyleArg::Solid => ll_render::BorderStyle::Solid,
            BorderStyleArg::Dashed => ll_render::BorderStyle::Dashed,
            BorderStyleArg::Dotted => ll_render::BorderStyle::Dotted,
            BorderStyleArg::Double => ll_render::BorderStyle::Double,
            BorderStyleArg::Striped => ll_render::BorderStyle::Striped,
        }
    }
}

/// Styled border options shared by `print` and `render`.
#[derive(Debug, Clone, clap::Args)]
struct BorderArgs {
    /// Rahmen mit Muster: solid, dashed, dotted, double, striped.
    #[arg(long, value_enum)]
    border: Option<BorderStyleArg>,
    /// Rahmenseiten als Buchstaben: o = oben, u = unten, l = links, r = rechts
    /// (z. B. `o` oder `ou`). Standard: alle.
    #[arg(long)]
    border_sides: Option<String>,
    /// Linienstärke des Rahmens in mm.
    #[arg(long)]
    border_width: Option<f32>,
    /// Strich- bzw. Streifenlänge in mm (dashed/striped).
    #[arg(long)]
    border_pattern: Option<f32>,
    /// Abstand des Rahmens vom Labelrand in mm.
    #[arg(long)]
    border_inset: Option<f32>,
}

impl BorderArgs {
    /// The border these options describe, `None` if none was given.
    fn to_border(&self) -> anyhow::Result<Option<ll_core::label::LabelBorder>> {
        if self.border.is_none()
            && self.border_sides.is_none()
            && self.border_width.is_none()
            && self.border_pattern.is_none()
            && self.border_inset.is_none()
        {
            return Ok(None);
        }
        let mut border = ll_core::label::LabelBorder::default();
        if let Some(style) = self.border {
            border.style = style.into();
        }
        if let Some(sides) = &self.border_sides {
            border.sides = parse_sides(sides)?;
        }
        if let Some(w) = self.border_width {
            border.width_mm = w;
        }
        if let Some(p) = self.border_pattern {
            border.pattern_mm = p;
        }
        if let Some(i) = self.border_inset {
            border.inset_mm = i;
        }
        Ok(Some(border))
    }
}

/// Parses `--border-sides` letters (o/u/l/r, also English t/b).
fn parse_sides(text: &str) -> anyhow::Result<ll_render::BorderSides> {
    let mut sides = ll_render::BorderSides {
        top: false,
        bottom: false,
        left: false,
        right: false,
    };
    for c in text.chars().filter(|c| !matches!(c, ',' | ' ')) {
        match c.to_ascii_lowercase() {
            'o' | 't' => sides.top = true,
            'u' | 'b' => sides.bottom = true,
            'l' => sides.left = true,
            'r' => sides.right = true,
            other => anyhow::bail!("unbekannte Rahmenseite {other:?} (erlaubt: o, u, l, r)"),
        }
    }
    Ok(sides)
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
        /// (Windows und Linux; `device` ist dann Name oder ID aus `devices`,
        /// unter Linux auch die MAC-Adresse).
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
        #[arg(long, conflicts_with_all = ["text", "qr", "barcode", "image"])]
        symbol: Option<String>,
        /// Bild/Symbol invertieren (nur mit --image/--symbol).
        #[arg(long)]
        invert: bool,
        /// Rahmen um das ganze Label zeichnen.
        #[arg(long)]
        frame: bool,
        #[command(flatten)]
        border_args: BorderArgs,
        /// Leervorschub vor dem Schnitt, in Druckpunkten (180 dpi). `0`
        /// schneidet direkt am letzten bedruckten Punkt.
        #[arg(long, default_value_t = ll_core::print::PrintOptions::default().margin_dots)]
        margin: u16,
        /// `.llabel`-Vorlage (JSON) statt Einzelinhalt.
        #[arg(long, conflicts_with_all = ["text", "qr", "barcode", "image", "symbol"])]
        template: Option<String>,
        /// Arbeitsblatt einer `.llabel`-Datei mit mehreren Blättern: Nummer (1, 2 …)
        /// oder Name. Standard: das erste.
        #[arg(long, requires = "template")]
        sheet: Option<String>,
        /// CSV-Datei für Serien: Platzhalter `{{Spalte}}` und `{{#}}` (Nummer)
        /// in der Vorlage werden je Datensatz ersetzt, ein Label pro Datensatz.
        #[arg(long, requires = "template")]
        csv: Option<String>,
        /// Nur diese Datensätze drucken (1-basiert): `5`, `1-10`, `3-`.
        #[arg(long, requires = "csv")]
        rows: Option<String>,
        /// Nummernfolge ohne CSV: so viele Labels drucken; `{{n}}`, `{{n:03}}`,
        /// `{{A}}`/`{{a}}` in der Vorlage zählen hoch.
        #[arg(long, requires = "template", conflicts_with = "csv")]
        count: Option<usize>,
        /// Startwert für `{{n}}` (auch mit CSV).
        #[arg(long, default_value_t = 1, allow_negative_numbers = true)]
        start: i64,
        /// Schrittweite für `{{n}}`.
        #[arg(long, default_value_t = 1, allow_negative_numbers = true)]
        step: i64,
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
        /// (Windows und Linux; `device` ist dann Name oder ID aus `devices`,
        /// unter Linux auch die MAC-Adresse).
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
        #[arg(long, conflicts_with_all = ["text", "qr", "barcode", "image"])]
        symbol: Option<String>,
        /// Bild/Symbol invertieren (nur mit --image/--symbol).
        #[arg(long)]
        invert: bool,
        /// Rahmen um das ganze Label zeichnen.
        #[arg(long)]
        frame: bool,
        #[command(flatten)]
        border_args: BorderArgs,
        /// `.llabel`-Vorlage (JSON) statt Einzelinhalt.
        #[arg(long, conflicts_with_all = ["text", "qr", "barcode", "image", "symbol"])]
        template: Option<String>,
        /// Arbeitsblatt einer `.llabel`-Datei mit mehreren Blättern: Nummer (1, 2 …)
        /// oder Name. Standard: das erste.
        #[arg(long, requires = "template")]
        sheet: Option<String>,
        /// CSV-Datei: Platzhalter mit Datensatz `--row` füllen.
        #[arg(long, requires = "template")]
        csv: Option<String>,
        /// Datensatz bzw. Label der Nummernfolge für die Vorschau (1-basiert).
        #[arg(long, default_value_t = 1)]
        row: usize,
        /// Startwert für `{{n}}`.
        #[arg(long, default_value_t = 1, allow_negative_numbers = true)]
        start: i64,
        /// Schrittweite für `{{n}}`.
        #[arg(long, default_value_t = 1, allow_negative_numbers = true)]
        step: i64,
        #[arg(short, long)]
        output: String,
        /// Bandbreite in mm (kein Drucker verbunden, daher nicht automatisch
        /// erkennbar).
        #[arg(long, default_value_t = 12)]
        width: u8,
        #[arg(long, default_value = DEFAULT_MODEL)]
        model: String,
    },
    /// Symbole aller Icon-Sets auflisten (für `print --symbol`/`render --symbol`),
    /// nach Set und Kategorie gruppiert.
    Symbols {
        /// Nur dieses Icon-Set (z. B. `iso7010`).
        #[arg(long)]
        set: Option<String>,
        /// Nur Symbole, deren Name/ID diesen Text enthält.
        #[arg(long)]
        search: Option<String>,
    },
    /// Icon-Sets (`.llabel-iconset`) verwalten.
    Iconset {
        #[command(subcommand)]
        action: IconsetAction,
    },
    /// Bluetooth-Drucker koppeln. Ohne Angabe: Geräte in der Nähe auflisten,
    /// die gekoppelt werden können (Drucker zuerst).
    Pair {
        /// Name oder ID aus der Liste.
        device: Option<String>,
    },
    /// Vorlage für Kabel/Netzwerk erzeugen (als `.llabel`, danach mit
    /// `print --template` drucken oder in der Oberfläche öffnen).
    Generate {
        #[command(subcommand)]
        kind: GenerateKind,
        /// Zieldatei (`.llabel`).
        #[arg(short, long, global = true, default_value = "label.llabel")]
        output: String,
        /// Bandbreite in mm (bestimmt die Höhe der Felder).
        #[arg(long, global = true, default_value_t = 12)]
        width: u8,
        #[arg(long, global = true, default_value = DEFAULT_MODEL)]
        model: String,
    },
}

#[derive(Subcommand)]
enum GenerateKind {
    /// Kabelfahne: Text zweimal, dazwischen der Wickelbereich (π × Durchmesser).
    CableFlag {
        text: String,
        /// Kabeldurchmesser in mm.
        #[arg(long)]
        diameter: f32,
        /// Länge jedes Fahnenendes in mm.
        #[arg(long, default_value_t = 25.0)]
        flag: f32,
    },
    /// Kabelwickel: Text wiederholt über den ganzen Umfang.
    CableWrap {
        text: String,
        /// Kabeldurchmesser in mm.
        #[arg(long)]
        diameter: f32,
        /// Anzahl Wiederholungen (Standard: etwa alle 15 mm).
        #[arg(long)]
        repeats: Option<u32>,
        /// Text quer zum Band (entlang des Kabels) drehen.
        #[arg(long)]
        vertical: bool,
    },
    /// Patchpanel/Port-Label: n Felder im festen Raster, nummeriert.
    PatchPanel {
        /// Anzahl Ports.
        #[arg(long, default_value_t = 24)]
        count: u32,
        /// Portabstand in mm.
        #[arg(long)]
        pitch: f32,
        #[arg(long, default_value_t = 1, allow_negative_numbers = true)]
        start: i64,
        #[arg(long, default_value_t = 1, allow_negative_numbers = true)]
        step: i64,
        /// Text vor jeder Nummer (z. B. "P").
        #[arg(long, default_value = "")]
        prefix: String,
        /// Mit Nullen auf so viele Stellen auffüllen.
        #[arg(long, default_value_t = 0)]
        digits: usize,
        /// Trennstriche zwischen den Feldern.
        #[arg(long)]
        separators: bool,
        /// Rand vor dem ersten und nach dem letzten Feld in mm.
        #[arg(long, default_value_t = 0.0)]
        margin: f32,
    },
    /// Einzelfähnchen: Wickelbereich (π × Durchmesser), dann ein Fähnchen mit Text.
    SingleFlag {
        text: String,
        /// Kabeldurchmesser in mm.
        #[arg(long)]
        diameter: f32,
        /// Länge des Fähnchens in mm.
        #[arg(long, default_value_t = 25.0)]
        flag: f32,
    },
    /// Klemmblock/LSA-Leiste: Felder im Raster, ein- oder zweireihig nummeriert.
    TerminalBlock {
        #[command(flatten)]
        fields: FieldArgs,
        /// Anzahl Reihen (1 oder 2).
        #[arg(long, default_value_t = 2)]
        rows: u8,
    },
    /// Sicherungskasten/Verteiler: Modulfelder (z. B. 17,5 mm), optional mit Hauptschalter-Feld.
    FuseBox {
        #[command(flatten)]
        fields: FieldArgs,
        /// Text quer zum Band (senkrecht).
        #[arg(long)]
        vertical: bool,
        /// Text des Hauptschalter-Felds (leer = keins).
        #[arg(long, default_value = "")]
        main_switch: String,
        /// Breite des Hauptschalter-Felds in mm.
        #[arg(long, default_value_t = 35.0)]
        main_switch_width: f32,
        /// Hauptschalter-Feld am Ende statt am Anfang.
        #[arg(long)]
        main_switch_right: bool,
    },
}

/// Numbered fields in a fixed pitch (terminal block, fuse box).
#[derive(Debug, Clone, clap::Args)]
struct FieldArgs {
    /// Anzahl Felder.
    #[arg(long, default_value_t = 12)]
    count: u32,
    /// Feldbreite in mm.
    #[arg(long)]
    pitch: f32,
    #[arg(long, default_value_t = 1, allow_negative_numbers = true)]
    start: i64,
    #[arg(long, default_value_t = 1, allow_negative_numbers = true)]
    step: i64,
    /// Text vor jeder Nummer (z. B. "F").
    #[arg(long, default_value = "")]
    prefix: String,
    /// Mit Nullen auf so viele Stellen auffüllen.
    #[arg(long, default_value_t = 0)]
    digits: usize,
    /// Trennstriche zwischen den Feldern.
    #[arg(long)]
    separators: bool,
    /// Rand vor dem ersten und nach dem letzten Feld in mm.
    #[arg(long, default_value_t = 0.0)]
    margin: f32,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    for warning in ll_core::iconsets::load_installed() {
        eprintln!("Warnung: Icon-Set nicht geladen: {warning}");
    }
    for warning in ll_core::frames::load_installed() {
        eprintln!("Warnung: Rahmen-Set nicht geladen: {warning}");
    }

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
            border_args,
            margin,
            template,
            sheet,
            csv,
            rows,
            count,
            start,
            step,
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
                    sheet,
                    border: border_args.to_border()?,
                },
                ll_core::print::PrintOptions {
                    frame,
                    auto_cut: cut,
                    chain,
                    margin_dots: margin,
                    ..Default::default()
                },
                copies,
                series_args(csv, rows.as_deref(), count, Numbering { start, step })?,
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
            border_args,
            template,
            sheet,
            csv,
            row,
            start,
            step,
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
                sheet,
                border: border_args.to_border()?,
            },
            frame,
            series_args(
                csv,
                Some(&row.to_string()),
                Some(row),
                Numbering { start, step },
            )?
            .map(|mut s| {
                s.rows = row..=row;
                s
            }),
            output,
            width,
            model,
        ),
        Command::Generate {
            kind,
            output,
            width,
            model,
        } => generate(kind, &output, width, &model),
        Command::Pair { device } => pair(device).await,
        Command::Symbols { set, search } => {
            list_symbols(set.as_deref(), search.as_deref());
            Ok(())
        }
        Command::Iconset { action } => iconset_command(action),
    }
}

#[derive(Debug, Subcommand)]
enum IconsetAction {
    /// Installierte Icon-Sets auflisten.
    List,
    /// `.llabel-iconset`-Datei importieren (ersetzt ein Set mit gleicher ID).
    Import { file: String },
    /// Importiertes Icon-Set entfernen.
    Remove { id: String },
    /// Icon-Set aus einem Ordner mit SVG-Dateien erstellen; Unterordner
    /// werden zu Kategorien.
    Create {
        /// Ordner mit `.svg`-Dateien.
        dir: String,
        /// Kurze ID (a-z, 0-9, -, _), z. B. `meine-icons`.
        #[arg(long)]
        id: String,
        /// Anzeigename.
        #[arg(long)]
        name: String,
        /// Zieldatei (`.llabel-iconset`).
        #[arg(short, long)]
        output: String,
    },
}

const LANG: &str = "de";

fn list_symbols(set_filter: Option<&str>, search: Option<&str>) {
    let needle = search.map(str::to_lowercase);
    for set in ll_render::iconset::sets() {
        if set_filter.is_some_and(|id| id != set.id) {
            continue;
        }
        let matches = |icon: &&ll_render::iconset::Icon| {
            needle.as_ref().is_none_or(|n| {
                icon.id.to_lowercase().contains(n)
                    || icon.name.get(LANG).to_lowercase().contains(n)
                    || icon.tags.iter().any(|t| t.to_lowercase().contains(n))
            })
        };
        let icons: Vec<_> = set.icons.iter().filter(matches).collect();
        if icons.is_empty() {
            continue;
        }
        println!("{} ({}):", set.name.get(LANG), set.id);
        let groups = set
            .categories
            .iter()
            .map(|c| (Some(c.id.as_str()), c.name.get(LANG)))
            .chain(std::iter::once((None, "Ohne Kategorie")));
        for (cat, cat_name) in groups {
            let in_cat: Vec<_> = icons
                .iter()
                .filter(|i| i.category.as_deref() == cat)
                .collect();
            if in_cat.is_empty() {
                continue;
            }
            println!("  {cat_name}:");
            for icon in in_cat {
                println!("    {}:{}  {}", set.id, icon.id, icon.name.get(LANG));
            }
        }
    }
}

fn iconset_command(action: IconsetAction) -> anyhow::Result<()> {
    match action {
        IconsetAction::List => {
            for set in ll_render::iconset::sets() {
                let kind = if ll_render::iconset::is_builtin(&set.id) {
                    "mitgeliefert"
                } else {
                    "importiert"
                };
                println!(
                    "{:<16} {} – {} Symbole, {} Kategorien ({kind})",
                    set.id,
                    set.name.get(LANG),
                    set.icons.len(),
                    set.categories.len()
                );
            }
            println!(
                "Ordner für importierte Sets: {}",
                ll_core::iconsets::iconset_dir()?.display()
            );
        }
        IconsetAction::Import { file } => {
            let set = ll_core::iconsets::import(Path::new(&file))?;
            println!(
                "Importiert: {} ({}) mit {} Symbolen – verwenden mit --symbol {}:<ID>",
                set.name.get(LANG),
                set.id,
                set.icons.len(),
                set.id
            );
        }
        IconsetAction::Remove { id } => {
            ll_core::iconsets::remove(&id)?;
            println!("Entfernt: {id}");
        }
        IconsetAction::Create {
            dir,
            id,
            name,
            output,
        } => {
            let set = ll_core::iconsets::from_dir(Path::new(&dir), &id, &name)?;
            std::fs::write(&output, set.to_json()?)?;
            println!(
                "Geschrieben: {output} ({} Symbole, {} Kategorien)",
                set.icons.len(),
                set.categories.len()
            );
        }
    }
    Ok(())
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

    #[cfg(any(windows, target_os = "linux"))]
    let bt_devices: Vec<(String, String)> = device::list_bluetooth_devices()
        .await
        .unwrap_or_else(|e| {
            eprintln!("Bluetooth-Geräte konnten nicht aufgelistet werden: {e}");
            Vec::new()
        })
        .into_iter()
        .map(|d| (d.id, d.name))
        .collect();
    #[cfg(not(any(windows, target_os = "linux")))]
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
    /// Sheet of a multi-sheet template (number or name).
    sheet: Option<String>,
    /// Overrides the template's border if set.
    border: Option<ll_core::label::LabelBorder>,
}

impl ContentArgs {
    /// Builds the label to render plus a short description for messages.
    /// `None` if no content was given.
    fn into_label(self) -> anyhow::Result<Option<(Label, String)>> {
        let border = self.border;
        Ok(self.into_plain_label()?.map(|(mut label, desc)| {
            if border.is_some() {
                label.border = border;
            }
            (label, desc)
        }))
    }

    fn into_plain_label(self) -> anyhow::Result<Option<(Label, String)>> {
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
                        brightness: 0,
                        contrast: 0,
                        edit: Default::default(),
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
                    let doc = ll_core::document::Document::load(std::path::Path::new(&t))?;
                    let sheet = doc.sheet(self.sheet.as_deref())?;
                    let desc = if doc.sheets.len() > 1 {
                        format!("{t} / {}", sheet.name)
                    } else {
                        t
                    };
                    Some((sheet.label.clone(), desc))
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
    data: Option<DataSet>,
    rows: RangeInclusive<usize>,
    numbering: Numbering,
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

/// `--csv`/`--rows` or `--count` (numbering only) into a [`Series`];
/// `None` without either.
fn series_args(
    csv: Option<String>,
    rows: Option<&str>,
    count: Option<usize>,
    numbering: Numbering,
) -> anyhow::Result<Option<Series>> {
    let Some(path) = csv else {
        return Ok(count.map(|n| Series {
            data: None,
            rows: 1..=n.max(1),
            numbering,
        }));
    };
    let data = DataSet::load(Path::new(&path))?;
    let rows = data.select(rows.map(parse_rows).transpose()?);
    if rows.is_empty() {
        anyhow::bail!(
            "Keine Datensätze im gewählten Bereich ({} vorhanden).",
            data.rows.len()
        );
    }
    Ok(Some(Series {
        data: Some(data),
        rows,
        numbering,
    }))
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
            .map(|n| series::apply(&label, s.data.as_ref(), n, s.numbering))
            .collect(),
        // Still fill built-in placeholders such as {{datum}} or {{n}}.
        None => vec![series::apply(&label, None, 1, Numbering::default())],
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
        label = series::apply(&label, s.data.as_ref(), *s.rows.start(), s.numbering);
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

fn generate(
    kind: GenerateKind,
    output: &str,
    width_mm: u8,
    model_name: &str,
) -> anyhow::Result<()> {
    let model = find_model(model_name);
    let geometry = ll_core::label::geometry_for(model, width_mm)?;
    let tape_mm = ll_protocol::model::dots_to_mm(geometry.printable_pins as u32);
    let layout = match kind {
        GenerateKind::CableFlag {
            text,
            diameter,
            flag,
        } => Layout::CableFlag(CableFlag {
            text,
            diameter_mm: diameter,
            flag_mm: flag,
        }),
        GenerateKind::CableWrap {
            text,
            diameter,
            repeats,
            vertical,
        } => Layout::CableWrap(CableWrap {
            text,
            diameter_mm: diameter,
            repeats,
            vertical,
        }),
        GenerateKind::PatchPanel {
            count,
            pitch,
            start,
            step,
            prefix,
            digits,
            separators,
            margin,
        } => Layout::PatchPanel(PatchPanel {
            count,
            pitch_mm: pitch,
            start,
            step,
            prefix,
            digits,
            separators,
            margin_mm: margin,
        }),
        GenerateKind::SingleFlag {
            text,
            diameter,
            flag,
        } => Layout::SingleFlag(SingleFlag {
            text,
            diameter_mm: diameter,
            flag_mm: flag,
        }),
        GenerateKind::TerminalBlock { fields: f, rows } => Layout::TerminalBlock(TerminalBlock {
            count: f.count,
            pitch_mm: f.pitch,
            start: f.start,
            step: f.step,
            prefix: f.prefix,
            digits: f.digits,
            rows,
            separators: f.separators,
            margin_mm: f.margin,
        }),
        GenerateKind::FuseBox {
            fields: f,
            vertical,
            main_switch,
            main_switch_width,
            main_switch_right,
        } => Layout::FuseBox(FuseBox {
            count: f.count,
            pitch_mm: f.pitch,
            start: f.start,
            step: f.step,
            prefix: f.prefix,
            digits: f.digits,
            vertical,
            main_switch,
            main_switch_mm: main_switch_width,
            main_switch_right,
            separators: f.separators,
            margin_mm: f.margin,
        }),
    };
    let label = layouts::generate(&layout, tape_mm);
    label.save(Path::new(output))?;
    println!(
        "Geschrieben: {output} ({:.1} mm lang, {} Elemente)",
        label.min_length_mm.unwrap_or_default(),
        label.elements.len()
    );
    Ok(())
}

#[cfg(any(windows, target_os = "linux"))]
async fn pair(spec: Option<String>) -> anyhow::Result<()> {
    eprintln!("Suche Bluetooth-Geräte in der Nähe ...");
    let mut found = device::discover_bluetooth_devices().await?;
    found.sort_by_key(|d| device::model_for_device_name(&d.name).is_none());
    let Some(spec) = spec else {
        if found.is_empty() {
            println!("Keine koppelbaren Geräte gefunden (Drucker eingeschaltet und sichtbar?).");
        }
        for d in &found {
            let model = device::model_for_device_name(&d.name)
                .map(|m| format!(" – Drucker {}", m.name))
                .unwrap_or_default();
            println!("  {}{model}  (labellab pair \"{}\")", d.name, d.name);
        }
        return Ok(());
    };
    let target = found
        .iter()
        .find(|d| d.id.eq_ignore_ascii_case(&spec) || d.name.eq_ignore_ascii_case(&spec))
        .map_or(spec.clone(), |d| d.id.clone());
    device::pair_bluetooth(&target).await?;
    println!("Gekoppelt: {spec}");
    Ok(())
}

#[cfg(not(any(windows, target_os = "linux")))]
async fn pair(_spec: Option<String>) -> anyhow::Result<()> {
    anyhow::bail!("Bluetooth-Kopplung wird nur unter Windows und Linux unterstützt.")
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::Cli;

    /// Catches inconsistent argument definitions (e.g. an argument that
    /// conflicts with itself), which clap only reports at runtime.
    #[test]
    fn cli_definition_is_consistent() {
        Cli::command().debug_assert();
    }
}
