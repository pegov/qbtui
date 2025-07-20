use std::process::exit;
use std::{io, sync::Arc};

use anyhow::Result;
use api::{ApiError, ApiEvent, ApiHandler, LoginError};
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

    #[arg(long)]
    username: Option<String>,

    #[arg(long)]
    password: Option<String>,

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

    let mut api_handler = ApiHandler::new(
        Arc::clone(&app),
        ui_tx.clone(),
        &args.url,
        args.do_not_verify_webui_certificate,
        args.username.clone(),
        args.password.clone(),
    );

    if args.username.is_some() && args.password.is_some() {
        if let Err(e) = api_handler.api.login().await {
            match e {
                ApiError::External(e) => {
                    tracing::debug!(?e);
                    eprintln!("Could not connect to {}: Check connection!", &args.url);
                    exit(1);
                }
                ApiError::Login(login_error) => match login_error {
                    LoginError::WrongCredentials => {
                        eprintln!("Could not connect to {}: Check credentials!", &args.url);
                        exit(1);
                    }
                    LoginError::TooManyAttempts => {
                        eprintln!(
                            "Could not connect to {}: Too many failed login attempts!",
                            &args.url
                        );
                        exit(1);
                    }
                },
                _ => unreachable!(),
            }
        }
    }

    if let Err(e) = api_handler.reload().await {
        match e {
            ApiError::External(e) => {
                tracing::debug!(?e);
                eprintln!("Could not connect to {}: Check connection!", &args.url);
                exit(1);
            }
            ApiError::NotAuthenticated => {
                eprintln!(
                    "Could not connect to {}: Authentication is required!",
                    &args.url
                );
                exit(1);
            }
            _ => unreachable!(),
        }
    }

    let api_handler_arc1 = Arc::new(Mutex::new(api_handler));
    let api_handler_arc2 = Arc::clone(&api_handler_arc1);

    tokio::spawn(async move {
        while let Some(event) = api_rx.recv().await {
            let mut api_handler = api_handler_arc2.lock().await;
            if let Err(e) = api_handler.handle(event).await {
                api_handler.handle_error(e).await;
            }
        }
    });

    start_ui(Arc::clone(&app), ui_rx).await?;

    let mut api_handler = api_handler_arc1.lock().await;
    let _ = api_handler.api.logout().await;

    Ok(())
}
