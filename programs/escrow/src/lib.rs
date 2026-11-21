use anchor_lang::prelude::*;

pub mod constants;
pub mod errors;
pub mod events;
pub mod instructions;
pub mod state;

pub use constants::*;
pub use instructions::*;

// Placeholder ID: run `anchor keys sync` after the first build (localnet only).
declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[program]
pub mod escrow {
    use super::*;

    /// Maker locks `offered_amount` of mint A in a PDA vault, asking `wanted_amount` of mint B.
    pub fn make_offer(
        ctx: Context<MakeOffer>,
        id: u64,
        offered_amount: u64,
        wanted_amount: u64,
    ) -> Result<()> {
        instructions::make_offer::handler(ctx, id, offered_amount, wanted_amount)
    }

    /// Taker pays the wanted B amount and receives the vault's A tokens atomically.
    pub fn take_offer(ctx: Context<TakeOffer>) -> Result<()> {
        instructions::take_offer::handler(ctx)
    }

    /// Maker cancels and reclaims the vault's A tokens.
    pub fn refund(ctx: Context<Refund>) -> Result<()> {
        instructions::refund::handler(ctx)
    }
}
