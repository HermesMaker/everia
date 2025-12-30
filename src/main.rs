use std::io;

use anyhow::Ok;
use clap::{CommandFactory, Parser};
use clap_complete::{Shell, generate};
use everia::everia::Everia;
#[derive(Parser)]
#[command(author, version, about)]
struct Args {
    url: Option<String>,
    #[arg(short, long, value_name = "Shell")]
    completion: Option<Shell>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    if let Some(url) = args.url {
        let everia = Everia::new(&url, None)?;
        everia.download().await;
    }
    if let Some(shell) = &args.completion {
        let mut arg_cli = Args::command();
        generate(*shell, &mut arg_cli, "everia", &mut io::stdout());
    } else {
        let _ = Args::command().print_help();
    }
    Ok(())
}
