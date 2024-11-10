use crate::gossamer::InMemoryGossamer;
use brongnal::BrongnalController;
use persistence::SqliteStorage;
use proto::gossamer::gossamer_server::GossamerServer;
use proto::service::brongnal_server::BrongnalServer;
use proto::FILE_DESCRIPTOR_SET;
use sentry::ClientInitGuard;
use std::net::{IpAddr, Ipv4Addr};
use std::path::PathBuf;
use std::str::FromStr;
use tokio_rusqlite::Connection;
use tonic::transport::Server;
use tonic_reflection::server::Builder;
use tracing::{info, warn, Level};
use tracing_subscriber::filter::Targets;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

mod brongnal;
mod gossamer;
mod persistence;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let filter = Targets::from_str(std::env::var("RUST_LOG").as_deref().unwrap_or("info"))
        .expect("RUST_LOG should be a valid tracing filter");
    tracing_subscriber::fmt()
        .with_max_level(Level::TRACE)
        .finish()
        .with(filter)
        .try_init()?;

    let _guard: Option<ClientInitGuard> = if let Ok(dsn) = std::env::var("SENTRY_DSN") {
        info!("Creating Sentry guard.");
        Some(sentry::init((
            dsn,
            sentry::ClientOptions {
                release: sentry::release_name!(),
                ..Default::default()
            },
        )))
    } else {
        warn!("Not creating Sentry guard.");
        None
    };

    let reflection_service = Builder::configure()
        .register_encoded_file_descriptor_set(FILE_DESCRIPTOR_SET)
        .build()
        .unwrap();
    let server_addr = (IpAddr::V4(Ipv4Addr::UNSPECIFIED), 8080).into();

    // let xdg_dirs = xdg::BaseDirectories::with_prefix("brongnal")?;
    let dirs = directories::BaseDirs::new().unwrap(); // xdg does not support windows
    let mut dirs = { let mut buf = PathBuf::from(dirs.data_dir()); buf.push("brongnal"); buf };
    let db_path: PathBuf = if let Ok(db_dir) = std::env::var("DB") {
        [&db_dir, "brongnal.db3"].iter().collect()
    } else {
        std::fs::create_dir_all(&dirs)?;
        dirs.push("brongnal_server.db3");
        dirs
    };
    info!("Database Path: {}", db_path.display());
    let connection = Connection::open(db_path).await?;
    let controller = BrongnalController::new(SqliteStorage::new(connection).await?);

    info!("Brongnal Server listening at: {server_addr}");

    Server::builder()
        .add_service(BrongnalServer::new(controller))
        .add_service(GossamerServer::new(InMemoryGossamer::default()))
        .add_service(reflection_service)
        .serve(server_addr)
        .await?;

    Ok(())
}
