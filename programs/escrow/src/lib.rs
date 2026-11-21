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

    /// Maker locks `offered_amount` of mint A and starts a Dutch auction: the price in
    /// mint B falls linearly from `start_price` to `end_price` over `duration_secs`.
    pub fn make_offer(
        ctx: Context<MakeOffer>,
        id: u64,
        offered_amount: u64,
        start_price: u64,
        end_price: u64,
        duration_secs: i64,
    ) -> Result<()> {
        instructions::make_offer::handler(
            ctx,
            id,
            offered_amount,
            start_price,
            end_price,
            duration_secs,
        )
    }

    /// Taker buys at the current price (reverts if it exceeds `max_price`).
    pub fn take_offer(ctx: Context<TakeOffer>, max_price: u64) -> Result<()> {
        instructions::take_offer::handler(ctx, max_price)
    }

    /// Maker cancels and reclaims the vault's A tokens.
    pub fn refund(ctx: Context<Refund>) -> Result<()> {
        instructions::refund::handler(ctx)
    }
}
