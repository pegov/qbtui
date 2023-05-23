use std::process::exit;
use std::{io, sync::Arc};

use anyhow::Result;
use api::{ApiEvent, ApiHandler};
use clap::Parser;
use tokio::sync::mpsc::channel;
use tokio::sync::Mutex;
use tracing_subscriber::EnvFilter;

use crate::{
    app::App,
    ui::{start_ui, UiEvent},
};

mod api;
mod app;
mod handlers;
mod humanize;
mod model;
mod ui;

#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {
    /// Format: "http://<host>:<port>"
    #[arg(long)]
    url: String,

    /// Necessary if the certificate is untrusted (e.g. self-signed)
    #[arg(long)]
    do_not_verify_webui_certificate: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(io::stderr)
        .init();

    let args = Args::parse();

    if !args.url.starts_with("http://") && !args.url.starts_with("https://") {
        eprintln!("Url format: \"http://<host>:<port>\"");
        exit(1);
    }

    let (ui_tx, ui_rx) = channel::<UiEvent>(32);
    let (api_tx, mut api_rx) = channel::<ApiEvent>(32);

    let app = Arc::new(Mutex::new(App::new(&args.url, api_tx.clone())));
    // TODO: username, password
    let mut api_handler = ApiHandler::new(
        Arc::clone(&app),
        ui_tx.clone(),
        &args.url,
        args.do_not_verify_webui_certificate,
    );

    if (api_handler.reload().await).is_err() {
        eprintln!("Could not connect to {}", &args.url);
        exit(1);
    }

    tokio::spawn(async move {
        while let Some(event) = api_rx.recv().await {
            if let Err(e) = api_handler.handle(event).await {
                api_handler.handle_error(e).await;
            }
        }
    });

    start_ui(Arc::clone(&app), ui_rx).await?;

    Ok(())
}
