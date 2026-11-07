use anyhow::Result;
use clap::{Parser, Subcommand};
use keeper_core::{chain::ChainClient, config::Config, db};
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "keepercli", version, about = "subscription-keeper daemon")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Verify configuration and connectivity to the RPC and database.
    Check,
    /// Run the keeper daemon.
    Run,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();
    let cfg = Config::from_env()?;

    match cli.command {
        Command::Check => check(&cfg).await,
        Command::Run => anyhow::bail!("`run` is not implemented yet"),
    }
}

async fn check(cfg: &Config) -> Result<()> {
    let mut ok = true;

    let chain = ChainClient::new(&cfg.rpc);
    match chain.health().await {
        Ok(h) => info!(url = %cfg.rpc.url, version = %h.version, slot = h.slot, "RPC OK"),
        Err(e) => {
            ok = false;
            error!(url = %cfg.rpc.url, "RPC check failed: {e:#}");
        }
    }

    match db::connect(&cfg.database.url).await {
        Ok(pool) => match db::ping(&pool).await {
            Ok(()) => info!("database OK"),
            Err(e) => {
                ok = false;
                error!("database ping failed: {e:#}");
            }
        },
        Err(e) => {
            ok = false;
            error!(url = %cfg.database.url, "database connect failed: {e:#}");
        }
    }

    if ok {
        info!("all checks passed");
        Ok(())
    } else {
        anyhow::bail!("one or more checks failed")
    }
}
