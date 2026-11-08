use anyhow::{Context, Result};
use solana_address::Address;
use solana_instruction::Instruction;
use solana_keypair::read_keypair_file;
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use solana_transaction::Transaction;
use subscriptions::instructions::{TransferSubscription, TransferSubscriptionInstructionArgs};
use subscriptions::types::TransferData;
use subscriptions::{
    EventAuthority, Plan, SubscriptionAuthority, SubscriptionDelegation, SUBSCRIPTIONS_ID,
};

use crate::chain::ChainClient;

/// The SPL Associated Token Account program.
const ASSOCIATED_TOKEN_PROGRAM_ID: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";

fn to_pubkey(a: &Address) -> Pubkey {
    Pubkey::from(a.to_bytes())
}

fn to_address(p: &Pubkey) -> Address {
    Address::from(p.to_bytes())
}

/// The associated token account that holds `mint` for `owner` under `token_program`.
fn associated_token_address(
    owner: &Address,
    mint: &Address,
    token_program: &Address,
) -> Result<Address> {
    let ata_program: Address = ASSOCIATED_TOKEN_PROGRAM_ID
        .parse()
        .context("parse associated-token program id")?;
    let (ata, _) = Address::find_program_address(
        &[owner.as_ref(), token_program.as_ref(), mint.as_ref()],
        &ata_program,
    );
    Ok(ata)
}

/// The wallet that receives a charge: the first configured destination on the
/// plan, or the plan owner when the plan leaves its destinations open.
///
/// The program validates the receiver against the plan's destination whitelist,
/// so this must resolve to an address the plan actually allows.
fn receiver_owner(plan: &Plan) -> Address {
    plan.data
        .destinations
        .iter()
        .find(|d| **d != Address::default())
        .copied()
        .unwrap_or(plan.owner)
}

/// Build the `transfer_subscription` instruction that charges one period of a
/// subscription. Pure: every value it needs is already decoded.
///
/// The subscriber and amount come from the subscription; the mint, destination,
/// and amount limits from its plan; and the token program from the mint account.
fn build_transfer_instruction(
    subscription: &Pubkey,
    sub: &SubscriptionDelegation,
    plan: &Plan,
    token_program: &Address,
    caller: &Pubkey,
) -> Result<Instruction> {
    let delegator = sub.header.delegator;
    let plan_address = sub.header.delegatee;
    let amount = sub.terms.amount;
    let mint = plan.data.mint;

    let (subscription_authority, _) = SubscriptionAuthority::find_pda(&delegator, &mint);
    let (event_authority, _) = EventAuthority::find_pda();
    let delegator_ata = associated_token_address(&delegator, &mint, token_program)?;
    // Funds go to the plan's destination (its owner when the plan is open); the
    // program rejects any receiver not on the plan's destination whitelist.
    let receiver_ata = associated_token_address(&receiver_owner(plan), &mint, token_program)?;

    Ok(TransferSubscription {
        subscription_pda: to_address(subscription),
        plan_pda: plan_address,
        subscription_authority,
        delegator_ata,
        receiver_ata,
        caller: to_address(caller),
        token_mint: mint,
        token_program: *token_program,
        event_authority,
        self_program: SUBSCRIPTIONS_ID,
    }
    .instruction(TransferSubscriptionInstructionArgs {
        transfer_data: TransferData {
            amount,
            delegator,
            mint,
        },
    }))
}

/// Outcome of a charge submission.
pub struct ChargeResult {
    pub signature: String,
    pub amount: u64,
}

/// Charge the current period of one subscription by submitting a
/// `transfer_subscription` instruction signed by the keeper's puller key.
///
/// Everything needed is read from the chain starting from the subscription
/// address: the subscriber and amount from the subscription, the mint from its
/// plan, and the token program from the mint account.
pub async fn charge_subscription(
    chain: &ChainClient,
    keypair_path: &str,
    subscription: &Pubkey,
) -> Result<ChargeResult> {
    let keypair = read_keypair_file(keypair_path)
        .map_err(|e| anyhow::anyhow!("reading keypair {keypair_path}: {e}"))?;
    let caller = keypair.pubkey();

    let sub_data = chain.get_account_data(subscription).await?;
    let sub =
        SubscriptionDelegation::from_bytes(&sub_data).context("decode subscription delegation")?;

    let plan_data = chain
        .get_account_data(&to_pubkey(&sub.header.delegatee))
        .await?;
    let plan = Plan::from_bytes(&plan_data).context("decode plan")?;

    // The token program is whichever program owns the mint (classic SPL or Token-2022).
    let mint_account = chain.get_account(&to_pubkey(&plan.data.mint)).await?;
    let token_program = to_address(&mint_account.owner);

    let instruction =
        build_transfer_instruction(subscription, &sub, &plan, &token_program, &caller)?;

    let blockhash = chain
        .rpc()
        .get_latest_blockhash()
        .await
        .context("get latest blockhash")?;
    let transaction =
        Transaction::new_signed_with_payer(&[instruction], Some(&caller), &[&keypair], blockhash);
    let signature = chain
        .rpc()
        .send_and_confirm_transaction(&transaction)
        .await
        .context("send and confirm transfer_subscription")?;

    Ok(ChargeResult {
        signature: signature.to_string(),
        amount: sub.terms.amount,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // Real devnet accounts captured as fixtures (see tests/fixtures/).
    const SUBSCRIPTION_PUBKEY: &str = "GvMZT3P1UXhW9GWVWvbzc4ypNdK4H8VjjW2oBfs79w7p";
    const SUBSCRIBER: &str = "ERwxEBkDWwQkN7bo24oxBF21Pp5zpinPYf5Z7kMLgY2J";
    const MINT: &str = "2Y6s1r4T4YoZkaB7XvsonSUv9cnYRf1pctqKgT36UDfV";
    const TOKEN_2022: &str = "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb";
    // The subscriber's associated token account, as reported by devnet.
    const SUBSCRIBER_ATA: &str = "66R8iUKe2mV3ucxHj7StRGn6MhH82mL3UWKj3Hg545gD";
    const CALLER: &str = "2ikHgTR3ysCnJHph5ZDpEbmPJE2mUwP6YFzZ8395Zr8x";

    const SUBSCRIPTION_BYTES: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/subscription_delegation.bin"
    ));
    const PLAN_BYTES: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/plan.bin"
    ));

    fn pubkey(s: &str) -> Pubkey {
        s.parse().unwrap()
    }

    fn address(s: &str) -> Address {
        s.parse().unwrap()
    }

    #[test]
    fn address_pubkey_roundtrip_preserves_bytes() {
        let p = pubkey(CALLER);
        assert_eq!(to_pubkey(&to_address(&p)), p);
        assert_eq!(to_address(&p).to_bytes(), p.to_bytes());
    }

    #[test]
    fn associated_token_address_matches_chain() {
        let ata =
            associated_token_address(&address(SUBSCRIBER), &address(MINT), &address(TOKEN_2022))
                .unwrap();
        assert_eq!(ata.to_bytes(), address(SUBSCRIBER_ATA).to_bytes());
    }

    #[test]
    fn builds_a_well_formed_transfer_instruction() {
        let sub = SubscriptionDelegation::from_bytes(SUBSCRIPTION_BYTES).unwrap();
        let plan = Plan::from_bytes(PLAN_BYTES).unwrap();
        let subscription = pubkey(SUBSCRIPTION_PUBKEY);
        let caller = pubkey(CALLER);

        let ix =
            build_transfer_instruction(&subscription, &sub, &plan, &address(TOKEN_2022), &caller)
                .unwrap();

        // Targets the Subscriptions program with the transfer_subscription discriminator (10).
        assert_eq!(ix.program_id.to_bytes(), SUBSCRIPTIONS_ID.to_bytes());
        assert_eq!(ix.data[0], 10);

        // Documented account layout: 10 accounts, the caller (index 5) is the only signer.
        assert_eq!(ix.accounts.len(), 10);
        assert_eq!(ix.accounts.iter().filter(|a| a.is_signer).count(), 1);
        assert_eq!(ix.accounts[5].pubkey.to_bytes(), caller.to_bytes());
        assert!(ix.accounts[5].is_signer);

        // Subscription PDA is first and writable.
        assert_eq!(ix.accounts[0].pubkey.to_bytes(), subscription.to_bytes());
        assert!(ix.accounts[0].is_writable);

        // The delegator token account (index 3) matches the chain-derived ATA.
        assert_eq!(
            ix.accounts[3].pubkey.to_bytes(),
            address(SUBSCRIBER_ATA).to_bytes()
        );

        // The receiver (index 4): this fixture plan has no destinations, so funds
        // go to the plan owner's ATA — the same wallet as the subscriber here.
        assert_eq!(
            ix.accounts[4].pubkey.to_bytes(),
            address(SUBSCRIBER_ATA).to_bytes()
        );
    }

    #[test]
    fn receiver_owner_prefers_configured_destination() {
        let mut plan = Plan::from_bytes(PLAN_BYTES).unwrap();

        // An open plan (all-zero destinations) pays the plan owner.
        assert_eq!(receiver_owner(&plan).to_bytes(), plan.owner.to_bytes());

        // A configured destination overrides the owner.
        let dest = address(CALLER);
        plan.data.destinations[2] = dest;
        assert_eq!(receiver_owner(&plan).to_bytes(), dest.to_bytes());
    }
}
