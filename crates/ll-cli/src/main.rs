use clap::{Parser, Subcommand};

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
    /// Gefundene Drucker (BT/USB/COM) auflisten.
    Devices,
    /// Bandstatus abfragen.
    Status {
        #[arg(long)]
        device: Option<String>,
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

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Devices
        | Command::Status { .. }
        | Command::Print { .. }
        | Command::Render { .. } => {
            eprintln!("Noch nicht implementiert (M1 – Grundgerüst). Siehe docs/PROGRESS.md.");
            std::process::exit(1);
        }
    }
}
