use anchor_lang::prelude::*;

#[error_code]
pub enum EscrowError {
    #[msg("Amount must be greater than zero")]
    InvalidAmount,
    #[msg("Offered and wanted mints must differ")]
    SameMint,
    #[msg("start_price must be >= end_price and end_price must be > 0")]
    InvalidPriceRange,
    #[msg("Duration must be greater than zero")]
    InvalidDuration,
    #[msg("Current price exceeds the taker's max_price")]
    SlippageExceeded,
    #[msg("Arithmetic overflow")]
    MathOverflow,
}
