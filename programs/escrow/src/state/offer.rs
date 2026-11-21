use anchor_lang::prelude::*;

use crate::errors::EscrowError;

/// A Dutch-auction offer: the price (in `mint_b`) decays linearly from
/// `start_price` at `start_ts` to `end_price` at `end_ts`, then stays at the floor.
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
    /// Price in `mint_b` at `start_ts`.
    pub start_price: u64,
    /// Floor price in `mint_b` from `end_ts` onward.
    pub end_price: u64,
    pub start_ts: i64,
    pub end_ts: i64,
    /// Canonical PDA bump, stored and reused for signing.
    pub bump: u8,
}

impl Offer {
    /// Current price at `now`. The decay is floored, so the price is rounded
    /// up, in the maker's favor.
    pub fn current_price(&self, now: i64) -> Result<u64> {
        if now <= self.start_ts {
            return Ok(self.start_price);
        }
        if now >= self.end_ts {
            return Ok(self.end_price);
        }
        let elapsed = (now - self.start_ts) as u128;
        let duration = (self.end_ts - self.start_ts) as u128;
        let range = self
            .start_price
            .checked_sub(self.end_price)
            .ok_or(EscrowError::InvalidPriceRange)? as u128;
        let drop = range
            .checked_mul(elapsed)
            .ok_or(EscrowError::MathOverflow)?
            .checked_div(duration)
            .ok_or(EscrowError::MathOverflow)?;
        let drop = u64::try_from(drop).map_err(|_| EscrowError::MathOverflow)?;
        self.start_price
            .checked_sub(drop)
            .ok_or_else(|| EscrowError::MathOverflow.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn offer() -> Offer {
        Offer {
            id: 0,
            maker: Pubkey::default(),
            mint_a: Pubkey::default(),
            mint_b: Pubkey::default(),
            offered_amount: 1,
            start_price: 500_000,
            end_price: 100_000,
            start_ts: 1_000,
            end_ts: 2_000,
            bump: 255,
        }
    }

    #[test]
    fn price_decays_linearly_and_clamps() {
        let o = offer();
        assert_eq!(o.current_price(0).unwrap(), 500_000);
        assert_eq!(o.current_price(1_000).unwrap(), 500_000);
        assert_eq!(o.current_price(1_500).unwrap(), 300_000);
        assert_eq!(o.current_price(2_000).unwrap(), 100_000);
        assert_eq!(o.current_price(9_999).unwrap(), 100_000);
    }

    #[test]
    fn price_rounds_up_for_maker() {
        let mut o = offer();
        o.start_price = 10;
        o.end_price = 0;
        o.start_ts = 0;
        o.end_ts = 3;
        // drop = 10 * 1 / 3 = 3 (floored) -> price 7 (rounded up from 6.67)
        assert_eq!(o.current_price(1).unwrap(), 7);
    }
}
