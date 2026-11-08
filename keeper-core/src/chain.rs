use anyhow::{Context, Result};
use solana_account::Account;
use solana_commitment_config::CommitmentConfig;
use solana_pubkey::Pubkey;
use solana_rpc_client::nonblocking::rpc_client::RpcClient;

use crate::config::RpcConfig;

pub struct ChainClient {
    rpc: RpcClient,
}

#[derive(Debug)]
pub struct ChainHealth {
    pub version: String,
    pub slot: u64,
}

impl ChainClient {
    pub fn new(cfg: &RpcConfig) -> Self {
        let commitment = match cfg.commitment.as_str() {
            "processed" => CommitmentConfig::processed(),
            "finalized" => CommitmentConfig::finalized(),
            _ => CommitmentConfig::confirmed(),
        };
        let rpc = RpcClient::new_with_commitment(cfg.url.clone(), commitment);
        Self { rpc }
    }

    /// Prove we can reach the RPC endpoint and read chain state.
    pub async fn health(&self) -> Result<ChainHealth> {
        let version = self.rpc.get_version().await.context("rpc get_version")?;
        let slot = self.rpc.get_slot().await.context("rpc get_slot")?;
        Ok(ChainHealth {
            version: version.solana_core,
            slot,
        })
    }

    /// Fetch the raw data of an account owned by any program.
    pub async fn get_account_data(&self, address: &Pubkey) -> Result<Vec<u8>> {
        self.rpc
            .get_account_data(address)
            .await
            .with_context(|| format!("rpc get_account_data {address}"))
    }

    /// Fetch a full account, including its owning program.
    pub async fn get_account(&self, address: &Pubkey) -> Result<Account> {
        self.rpc
            .get_account(address)
            .await
            .with_context(|| format!("rpc get_account {address}"))
    }

    pub fn rpc(&self) -> &RpcClient {
        &self.rpc
    }
}
