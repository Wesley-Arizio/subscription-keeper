use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use keeper_core::{accounts::ProgramAccount, chain::ChainClient, charge, config::Config, db};
use solana_pubkey::Pubkey;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "keepercli", version, about = "subscription-keeper keeper")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Verify configuration and connectivity to the RPC and database.
    Check,
    /// Fetch an on-chain account and decode it as a Subscriptions program account.
    Inspect {
        /// Base-58 address of the account to fetch.
        address: String,
    },
    /// Charge the current period of one subscription.
    Pull {
        /// Base-58 address of the subscription to charge.
        address: String,
    },
    /// Run the keeper.
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
        Command::Inspect { address } => inspect(&cfg, &address).await,
        Command::Pull { address } => pull(&cfg, &address).await,
        Command::Run => anyhow::bail!("`run` is not implemented yet"),
    }
}

async fn pull(cfg: &Config, address: &str) -> Result<()> {
    let subscription: Pubkey = address.parse().context("invalid subscription address")?;
    let chain = ChainClient::new(&cfg.rpc);

    let result =
        charge::charge_subscription(&chain, &cfg.signer.keypair_path, &subscription).await?;
    info!(signature = %result.signature, amount = result.amount, "charge confirmed");

    // Re-read the subscription to show the period being recorded on-chain.
    let data = chain.get_account_data(&subscription).await?;
    info!("{:#?}", ProgramAccount::decode(&data)?);
    Ok(())
}

async fn inspect(cfg: &Config, address: &str) -> Result<()> {
    let address: Pubkey = address.parse().context("invalid base-58 address")?;
    let chain = ChainClient::new(&cfg.rpc);

    let data = chain.get_account_data(&address).await?;
    let account = ProgramAccount::decode(&data)?;

    info!(%address, bytes = data.len(), "fetched account");
    info!("{account:#?}");
    Ok(())
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
