use anchor_lang::prelude::*;

#[event]
pub struct OfferMade {
    pub offer: Pubkey,
    pub maker: Pubkey,
    pub mint_a: Pubkey,
    pub mint_b: Pubkey,
    pub offered_amount: u64,
    pub wanted_amount: u64,
}

#[event]
pub struct OfferTaken {
    pub offer: Pubkey,
    pub maker: Pubkey,
    pub taker: Pubkey,
    pub offered_amount: u64,
    pub wanted_amount: u64,
}

#[event]
pub struct OfferRefunded {
    pub offer: Pubkey,
    pub maker: Pubkey,
    pub refunded_amount: u64,
}
