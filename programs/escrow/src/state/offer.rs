use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct Offer {
    /// Caller-chosen id so one maker can have many open offers.
    pub id: u64,
    pub maker: Pubkey,
    pub mint_a: Pubkey,
    pub mint_b: Pubkey,
    /// Amount of `mint_a` the maker deposited (nominal, pre-transfer-fee).
    pub offered_amount: u64,
    /// Amount of `mint_b` the maker wants in return.
    pub wanted_amount: u64,
    /// Canonical PDA bump, stored and reused for signing.
    pub bump: u8,
}
