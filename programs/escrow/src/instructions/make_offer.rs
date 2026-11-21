use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked},
};

use crate::{constants::OFFER_SEED, errors::EscrowError, events::OfferMade, state::Offer};

#[derive(Accounts)]
#[instruction(id: u64)]
pub struct MakeOffer<'info> {
    #[account(mut)]
    pub maker: Signer<'info>,

    #[account(mint::token_program = token_program)]
    pub mint_a: Box<InterfaceAccount<'info, Mint>>,

    #[account(mint::token_program = token_program)]
    pub mint_b: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        mut,
        associated_token::mint = mint_a,
        associated_token::authority = maker,
        associated_token::token_program = token_program,
    )]
    pub maker_ata_a: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        init,
        payer = maker,
        space = 8 + Offer::INIT_SPACE,
        seeds = [OFFER_SEED, maker.key().as_ref(), id.to_le_bytes().as_ref()],
        bump,
    )]
    pub offer: Box<Account<'info, Offer>>,

    #[account(
        init,
        payer = maker,
        associated_token::mint = mint_a,
        associated_token::authority = offer,
        associated_token::token_program = token_program,
    )]
    pub vault: Box<InterfaceAccount<'info, TokenAccount>>,

    pub associated_token_program: Program<'info, AssociatedToken>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

pub fn handler(
    ctx: Context<MakeOffer>,
    id: u64,
    offered_amount: u64,
    start_price: u64,
    end_price: u64,
    duration_secs: i64,
) -> Result<()> {
    require!(offered_amount > 0, EscrowError::InvalidAmount);
    require!(end_price > 0 && start_price >= end_price, EscrowError::InvalidPriceRange);
    require!(duration_secs > 0, EscrowError::InvalidDuration);
    let start_ts = Clock::get()?.unix_timestamp;
    let end_ts = start_ts
        .checked_add(duration_secs)
        .ok_or(EscrowError::MathOverflow)?;
    require_keys_neq!(
        ctx.accounts.mint_a.key(),
        ctx.accounts.mint_b.key(),
        EscrowError::SameMint
    );

    transfer_checked(
        CpiContext::new(
            ctx.accounts.token_program.key(),
            TransferChecked {
                from: ctx.accounts.maker_ata_a.to_account_info(),
                mint: ctx.accounts.mint_a.to_account_info(),
                to: ctx.accounts.vault.to_account_info(),
                authority: ctx.accounts.maker.to_account_info(),
            },
        ),
        offered_amount,
        ctx.accounts.mint_a.decimals,
    )?;

    let offer = &mut ctx.accounts.offer;
    offer.id = id;
    offer.maker = ctx.accounts.maker.key();
    offer.mint_a = ctx.accounts.mint_a.key();
    offer.mint_b = ctx.accounts.mint_b.key();
    offer.offered_amount = offered_amount;
    offer.start_price = start_price;
    offer.end_price = end_price;
    offer.start_ts = start_ts;
    offer.end_ts = end_ts;
    offer.bump = ctx.bumps.offer;

    emit!(OfferMade {
        offer: offer.key(),
        maker: offer.maker,
        mint_a: offer.mint_a,
        mint_b: offer.mint_b,
        offered_amount,
        start_price,
        end_price,
        start_ts,
        end_ts,
    });
    Ok(())
}
