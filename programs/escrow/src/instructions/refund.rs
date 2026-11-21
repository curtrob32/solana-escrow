use anchor_lang::prelude::*;
use anchor_spl::token_interface::{
    close_account, transfer_checked, CloseAccount, Mint, TokenAccount, TokenInterface,
    TransferChecked,
};

use crate::{constants::OFFER_SEED, events::OfferRefunded, state::Offer};

#[derive(Accounts)]
pub struct Refund<'info> {
    #[account(mut)]
    pub maker: Signer<'info>,

    #[account(mint::token_program = token_program)]
    pub mint_a: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        mut,
        associated_token::mint = mint_a,
        associated_token::authority = maker,
        associated_token::token_program = token_program,
    )]
    pub maker_ata_a: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        close = maker,
        has_one = maker,
        has_one = mint_a,
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

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<Refund>) -> Result<()> {
    let offer = &ctx.accounts.offer;
    let maker_key = offer.maker;
    let id_bytes = offer.id.to_le_bytes();
    let bump = [offer.bump];
    let signer_seeds: &[&[&[u8]]] = &[&[OFFER_SEED, maker_key.as_ref(), id_bytes.as_ref(), &bump]];

    let refunded_amount = ctx.accounts.vault.amount;
    transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.key(),
            TransferChecked {
                from: ctx.accounts.vault.to_account_info(),
                mint: ctx.accounts.mint_a.to_account_info(),
                to: ctx.accounts.maker_ata_a.to_account_info(),
                authority: ctx.accounts.offer.to_account_info(),
            },
            signer_seeds,
        ),
        refunded_amount,
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

    emit!(OfferRefunded {
        offer: ctx.accounts.offer.key(),
        maker: maker_key,
        refunded_amount,
    });
    Ok(())
}
