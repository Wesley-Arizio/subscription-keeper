use anyhow::{Context, Result};
use num_traits::FromPrimitive;
use subscriptions::generated::types::AccountDiscriminator;
use subscriptions::{
    FixedDelegation, Plan, RecurringDelegation, SubscriptionAuthority, SubscriptionDelegation,
};

/// A decoded account owned by the Subscriptions & Allowances program.
///
/// Every account the program owns begins with a one-byte discriminator that
/// identifies its type; the remaining bytes are the borsh-encoded struct.
#[derive(Debug)]
pub enum ProgramAccount {
    SubscriptionAuthority(Box<SubscriptionAuthority>),
    Plan(Box<Plan>),
    FixedDelegation(Box<FixedDelegation>),
    RecurringDelegation(Box<RecurringDelegation>),
    SubscriptionDelegation(Box<SubscriptionDelegation>),
}

impl ProgramAccount {
    /// Decode raw account data, selecting the struct from the leading
    /// discriminator byte.
    pub fn decode(data: &[u8]) -> Result<Self> {
        let discriminator = *data.first().context("account data is empty")?;
        let kind = AccountDiscriminator::from_u8(discriminator)
            .with_context(|| format!("unknown account discriminator {discriminator}"))?;

        Ok(match kind {
            AccountDiscriminator::SubscriptionAuthority => {
                Self::SubscriptionAuthority(Box::new(SubscriptionAuthority::from_bytes(data)?))
            }
            AccountDiscriminator::Plan => Self::Plan(Box::new(Plan::from_bytes(data)?)),
            AccountDiscriminator::FixedDelegation => {
                Self::FixedDelegation(Box::new(FixedDelegation::from_bytes(data)?))
            }
            AccountDiscriminator::RecurringDelegation => {
                Self::RecurringDelegation(Box::new(RecurringDelegation::from_bytes(data)?))
            }
            AccountDiscriminator::SubscriptionDelegation => {
                Self::SubscriptionDelegation(Box::new(SubscriptionDelegation::from_bytes(data)?))
            }
        })
    }
}
