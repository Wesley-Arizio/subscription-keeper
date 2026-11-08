use anyhow::{Context, Result};

#[derive(Debug, Clone)]
pub struct Config {
    pub rpc: RpcConfig,
    pub database: DatabaseConfig,
    pub signer: SignerConfig,
}

#[derive(Debug, Clone)]
pub struct RpcConfig {
    pub url: String,
    pub commitment: String,
}

#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    pub url: String,
}

#[derive(Debug, Clone)]
pub struct SignerConfig {
    /// Path to the local keypair file the keeper signs charges with.
    pub keypair_path: String,
}

impl Config {
    /// Load configuration from the environment, reading a `.env` file in the
    /// current directory first if one is present.
    pub fn from_env() -> Result<Self> {
        dotenvy::dotenv().ok();

        let database_url = std::env::var("DATABASE_URL").context("DATABASE_URL must be set")?;
        let rpc_url = std::env::var("KEEPER_RPC_URL")
            .unwrap_or_else(|_| "https://api.devnet.solana.com".to_string());
        let rpc_commitment =
            std::env::var("KEEPER_RPC_COMMITMENT").unwrap_or_else(|_| "confirmed".to_string());
        let signer_keypair_path = std::env::var("KEEPER_SIGNER_PATH")
            .unwrap_or_else(|_| "keys/keeper-puller-devnet.json".to_string());

        Ok(Config {
            rpc: RpcConfig {
                url: rpc_url,
                commitment: rpc_commitment,
            },
            database: DatabaseConfig { url: database_url },
            signer: SignerConfig {
                keypair_path: signer_keypair_path,
            },
        })
    }
}
