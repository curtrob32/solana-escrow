use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{
        close_account, transfer_checked, CloseAccount, Mint, TokenAccount, TokenInterface,
        TransferChecked,
    },
};

use crate::{constants::OFFER_SEED, errors::EscrowError, events::OfferTaken, state::Offer};

#[derive(Accounts)]
pub struct TakeOffer<'info> {
    #[account(mut)]
    pub taker: Signer<'info>,

    /// Receives the rent from the closed offer; pinned by `has_one` on `offer`.
    #[account(mut)]
    pub maker: SystemAccount<'info>,

    #[account(mint::token_program = token_program)]
    pub mint_a: Box<InterfaceAccount<'info, Mint>>,

    #[account(mint::token_program = token_program)]
    pub mint_b: Box<InterfaceAccount<'info, Mint>>,

    // Idempotent ATA creation only; no program state depends on it being fresh.
    #[account(
        init_if_needed,
        payer = taker,
        associated_token::mint = mint_a,
        associated_token::authority = taker,
        associated_token::token_program = token_program,
    )]
    pub taker_ata_a: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        associated_token::mint = mint_b,
        associated_token::authority = taker,
        associated_token::token_program = token_program,
    )]
    pub taker_ata_b: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        init_if_needed,
        payer = taker,
        associated_token::mint = mint_b,
        associated_token::authority = maker,
        associated_token::token_program = token_program,
    )]
    pub maker_ata_b: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        close = maker,
        has_one = maker,
        has_one = mint_a,
        has_one = mint_b,
        seeds = [OFFER_SEED, maker.key().as_ref(), offer.id.to_le_bytes().as_ref()],
        bump = offer.bump,
    )]
    pub offer: Box<Account<'info, Offer>>,

    #[account(
        mut,
        associated_token::mint = mint_a,
        associated_token::authority = offer,
        associated_token::token_program = token_program,
    )]
    pub vault: Box<InterfaceAccount<'info, TokenAccount>>,

    pub associated_token_program: Program<'info, AssociatedToken>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<TakeOffer>, max_price: u64) -> Result<()> {
    let offer = &ctx.accounts.offer;
    let price = offer.current_price(Clock::get()?.unix_timestamp)?;
    require!(price <= max_price, EscrowError::SlippageExceeded);
    let maker_key = offer.maker;
    let id_bytes = offer.id.to_le_bytes();
    let bump = [offer.bump];
    let signer_seeds: &[&[&[u8]]] = &[&[OFFER_SEED, maker_key.as_ref(), id_bytes.as_ref(), &bump]];

    // Taker pays the maker.
    transfer_checked(
        CpiContext::new(
            ctx.accounts.token_program.key(),
            TransferChecked {
                from: ctx.accounts.taker_ata_b.to_account_info(),
                mint: ctx.accounts.mint_b.to_account_info(),
                to: ctx.accounts.maker_ata_b.to_account_info(),
                authority: ctx.accounts.taker.to_account_info(),
            },
        ),
        price,
        ctx.accounts.mint_b.decimals,
    )?;

    // Vault pays the taker. Use the live balance so Token-2022 transfer fees
    // taken on deposit can't leave dust that blocks closing the vault.
    let vault_amount = ctx.accounts.vault.amount;
    transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.key(),
            TransferChecked {
                from: ctx.accounts.vault.to_account_info(),
                mint: ctx.accounts.mint_a.to_account_info(),
                to: ctx.accounts.taker_ata_a.to_account_info(),
                authority: ctx.accounts.offer.to_account_info(),
            },
            signer_seeds,
        ),
        vault_amount,
        ctx.accounts.mint_a.decimals,
    )?;

    close_account(CpiContext::new_with_signer(
        ctx.accounts.token_program.key(),
        CloseAccount {
            account: ctx.accounts.vault.to_account_info(),
            destination: ctx.accounts.maker.to_account_info(),
            authority: ctx.accounts.offer.to_account_info(),
        },
        signer_seeds,
    ))?;

    emit!(OfferTaken {
        offer: ctx.accounts.offer.key(),
        maker: maker_key,
        taker: ctx.accounts.taker.key(),
        offered_amount: vault_amount,
        price_paid: price,
    });
    Ok(())
}
