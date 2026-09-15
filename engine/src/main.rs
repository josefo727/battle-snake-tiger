//! The deployable server: settings from the environment, the production clock,
//! JSON logs on stdout, and the Battlesnake routes.

use std::process::ExitCode;
use std::sync::Arc;

use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use tiger_engine::build_service;
use tiger_engine::gateway::beacon::TracingBeacon;
use tiger_engine::gateway::calendar::SystemCalendar;
use tiger_engine::gateway::clock::SystemClock;
use tiger_engine::gateway::lifecycle::TracingLifecycleBeacon;
use tiger_engine::gateway::logfile::DailyFileWriter;
use tiger_engine::gateway::settings::Settings;

/// JSON lines on standard output and, when `settings` names a log directory, the same lines
/// in that directory's daily files.
fn start_logging(settings: Option<&Settings>) {
    let file = settings.and_then(|settings| {
        let directory = settings.log_dir.clone()?;
        let writer =
            DailyFileWriter::new(directory, settings.log_keep_days, Arc::new(SystemCalendar));
        Some(
            tracing_subscriber::fmt::layer()
                .json()
                .with_ansi(false)
                .with_writer(writer),
        )
    });
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().json())
        .with(file)
        .init();
}

#[tokio::main]
async fn main() -> ExitCode {
    let settings = match Settings::from_env() {
        Ok(settings) => {
            start_logging(Some(&settings));
            settings
        }
        Err(error) => {
            // The settings decide where logs go, so a bad one is reported on standard output only.
            start_logging(None);
            tracing::error!("{error}");
            return ExitCode::FAILURE;
        }
    };

    let listener = match tokio::net::TcpListener::bind(settings.socket_addr()).await {
        Ok(listener) => listener,
        Err(error) => {
            tracing::error!("cannot listen on {}: {error}", settings.socket_addr());
            return ExitCode::FAILURE;
        }
    };
    match listener.local_addr() {
        Ok(addr) => tracing::info!(%addr, "listening"),
        Err(error) => {
            tracing::error!("cannot read the listening address: {error}");
            return ExitCode::FAILURE;
        }
    }

    let app = build_service(
        Arc::new(SystemClock::new()),
        Arc::new(TracingBeacon),
        Arc::new(TracingLifecycleBeacon),
    );
    if let Err(error) = axum::serve(listener, app).await {
        tracing::error!("the server stopped: {error}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
