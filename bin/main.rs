use anyhow::Result;
use elara_kv_component::cmd::CliOpts;
use elara_kv_component::config::ServiceConfig;
use elara_kv_component::start_server;
use std::fs;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let opt = CliOpts::init();
    let toml = fs::read_to_string(opt.config.as_path())?;
    let config = ServiceConfig::parse(toml)?;
    log::info!("Load config: {:#?}", config);

    start_server(config).await
}
