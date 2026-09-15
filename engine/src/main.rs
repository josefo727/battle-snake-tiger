//! The deployable server: settings from the environment, the production clock,
//! JSON logs on stdout, and the Battlesnake routes.

use std::process::ExitCode;
use std::sync::Arc;

use tiger_engine::build_service;
use tiger_engine::gateway::beacon::TracingBeacon;
use tiger_engine::gateway::clock::SystemClock;
use tiger_engine::gateway::lifecycle::TracingLifecycleBeacon;
use tiger_engine::gateway::settings::Settings;

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt().json().init();

    let settings = match Settings::from_env() {
        Ok(settings) => settings,
        Err(error) => {
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
