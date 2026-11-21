use anchor_lang::prelude::*;

#[error_code]
pub enum EscrowError {
    #[msg("Amount must be greater than zero")]
    InvalidAmount,
    #[msg("Offered and wanted mints must differ")]
    SameMint,
    #[msg("Vault balance does not match the offered amount")]
    VaultMismatch,
}
