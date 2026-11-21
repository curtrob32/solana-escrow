//! LiteSVM tests for the Dutch-auction escrow program.
//! Run `NO_DNA=1 anchor build` first so `target/deploy/escrow.so` exists.
//! NOTE: written without a local toolchain; crate versions/APIs may need small tweaks.

use anchor_lang::{prelude::Clock, system_program, InstructionData, ToAccountMetas};
use anchor_spl::{
    associated_token::{self, get_associated_token_address},
    token,
};
use litesvm::LiteSVM;
use litesvm_token::{spl_token, CreateAssociatedTokenAccount, CreateMint, MintTo};
use solana_keypair::Keypair;
use solana_message::Message;
use solana_program_pack::Pack;
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use solana_transaction::Transaction;

const OFFERED: u64 = 1_000_000;
const START: u64 = 500_000;
const END: u64 = 100_000;
const DURATION: i64 = 1_000;
const DECIMALS: u8 = 6;

struct Ctx {
    svm: LiteSVM,
    maker: Keypair,
    taker: Keypair,
    mint_a: Pubkey,
    mint_b: Pubkey,
    maker_ata_a: Pubkey,
    taker_ata_b: Pubkey,
}

fn offer_pda(maker: &Pubkey, id: u64) -> Pubkey {
    Pubkey::find_program_address(&[b"offer", maker.as_ref(), &id.to_le_bytes()], &escrow::ID).0
}

fn setup() -> Ctx {
    let mut svm = LiteSVM::new();
    svm.add_program_from_file(escrow::ID, "../../target/deploy/escrow.so")
        .expect("run `anchor build` first");

    let maker = Keypair::new();
    let taker = Keypair::new();
    svm.airdrop(&maker.pubkey(), 10_000_000_000).unwrap();
    svm.airdrop(&taker.pubkey(), 10_000_000_000).unwrap();

    let mint_a = CreateMint::new(&mut svm, &maker).decimals(DECIMALS).send().unwrap();
    let mint_b = CreateMint::new(&mut svm, &maker).decimals(DECIMALS).send().unwrap();

    let maker_ata_a = CreateAssociatedTokenAccount::new(&mut svm, &maker, &mint_a)
        .owner(&maker.pubkey())
        .send()
        .unwrap();
    let taker_ata_b = CreateAssociatedTokenAccount::new(&mut svm, &taker, &mint_b)
        .owner(&taker.pubkey())
        .send()
        .unwrap();

    MintTo::new(&mut svm, &maker, &mint_a, &maker_ata_a, OFFERED * 2).send().unwrap();
    MintTo::new(&mut svm, &maker, &mint_b, &taker_ata_b, START * 2).send().unwrap();

    Ctx { svm, maker, taker, mint_a, mint_b, maker_ata_a, taker_ata_b }
}

fn make_ix(
    c: &Ctx,
    id: u64,
    offered: u64,
    start: u64,
    end: u64,
    duration: i64,
    mint_b: Pubkey,
) -> solana_instruction::Instruction {
    let offer = offer_pda(&c.maker.pubkey(), id);
    solana_instruction::Instruction {
        program_id: escrow::ID,
        accounts: escrow::accounts::MakeOffer {
            maker: c.maker.pubkey(),
            mint_a: c.mint_a,
            mint_b,
            maker_ata_a: c.maker_ata_a,
            offer,
            vault: get_associated_token_address(&offer, &c.mint_a),
            associated_token_program: associated_token::ID,
            token_program: token::ID,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
        data: escrow::instruction::MakeOffer {
            id,
            offered_amount: offered,
            start_price: start,
            end_price: end,
            duration_secs: duration,
        }
        .data(),
    }
}

fn take_ix(
    c: &Ctx,
    id: u64,
    mint_b: Pubkey,
    taker_ata_b: Pubkey,
    max_price: u64,
) -> solana_instruction::Instruction {
    let offer = offer_pda(&c.maker.pubkey(), id);
    solana_instruction::Instruction {
        program_id: escrow::ID,
        accounts: escrow::accounts::TakeOffer {
            taker: c.taker.pubkey(),
            maker: c.maker.pubkey(),
            mint_a: c.mint_a,
            mint_b,
            taker_ata_a: get_associated_token_address(&c.taker.pubkey(), &c.mint_a),
            taker_ata_b,
            maker_ata_b: get_associated_token_address(&c.maker.pubkey(), &mint_b),
            offer,
            vault: get_associated_token_address(&offer, &c.mint_a),
            associated_token_program: associated_token::ID,
            token_program: token::ID,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
        data: escrow::instruction::TakeOffer { max_price }.data(),
    }
}

fn refund_ix(c: &Ctx, id: u64, signer: &Pubkey) -> solana_instruction::Instruction {
    let offer = offer_pda(&c.maker.pubkey(), id);
    solana_instruction::Instruction {
        program_id: escrow::ID,
        accounts: escrow::accounts::Refund {
            maker: *signer,
            mint_a: c.mint_a,
            maker_ata_a: c.maker_ata_a,
            offer,
            vault: get_associated_token_address(&offer, &c.mint_a),
            token_program: token::ID,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
        data: escrow::instruction::Refund {}.data(),
    }
}

fn send(svm: &mut LiteSVM, ix: solana_instruction::Instruction, signer: &Keypair) -> Result<(), String> {
    let tx = Transaction::new(
        &[signer],
        Message::new(&[ix], Some(&signer.pubkey())),
        svm.latest_blockhash(),
    );
    svm.send_transaction(tx).map(|_| ()).map_err(|e| format!("{:?}", e))
}

fn balance(svm: &LiteSVM, ata: &Pubkey) -> u64 {
    let acc = svm.get_account(ata).expect("token account missing");
    spl_token::state::Account::unpack(&acc.data).unwrap().amount
}

/// Move the cluster clock forward by `secs`.
fn warp(svm: &mut LiteSVM, secs: i64) {
    let mut clock = svm.get_sysvar::<Clock>();
    clock.unix_timestamp += secs;
    svm.set_sysvar(&clock);
    svm.expire_blockhash();
}

/// Make offer `id` as the default maker, then return the context.
fn with_offer(id: u64) -> Ctx {
    let mut c = setup();
    let maker = c.maker.insecure_clone();
    let ix = make_ix(&c, id, OFFERED, START, END, DURATION, c.mint_b);
    send(&mut c.svm, ix, &maker).unwrap();
    c
}

fn take_and_check(c: &mut Ctx, id: u64, max_price: u64, expected_price: u64) {
    let taker = c.taker.insecure_clone();
    let ix = take_ix(c, id, c.mint_b, c.taker_ata_b, max_price);
    send(&mut c.svm, ix, &taker).unwrap();

    let offer = offer_pda(&c.maker.pubkey(), id);
    let vault = get_associated_token_address(&offer, &c.mint_a);
    let taker_a = get_associated_token_address(&c.taker.pubkey(), &c.mint_a);
    let maker_b = get_associated_token_address(&c.maker.pubkey(), &c.mint_b);
    assert_eq!(balance(&c.svm, &taker_a), OFFERED);
    assert_eq!(balance(&c.svm, &maker_b), expected_price);
    assert_eq!(balance(&c.svm, &c.taker_ata_b), START * 2 - expected_price);
    assert!(c.svm.get_account(&vault).is_none(), "vault should be closed");
    assert!(c.svm.get_account(&offer).is_none(), "offer should be closed");
}

#[test]
fn take_at_start_pays_start_price() {
    let mut c = with_offer(1);
    take_and_check(&mut c, 1, START, START);
}

#[test]
fn price_decays_linearly() {
    let mut c = with_offer(2);
    warp(&mut c.svm, DURATION / 2);
    // 500_000 - (400_000 * 500 / 1000) = 300_000
    take_and_check(&mut c, 2, START, 300_000);
}

#[test]
fn price_floors_after_end() {
    let mut c = with_offer(3);
    warp(&mut c.svm, DURATION * 10);
    take_and_check(&mut c, 3, START, END);
}

#[test]
fn take_fails_when_price_exceeds_max_price() {
    let mut c = with_offer(4);
    let taker = c.taker.insecure_clone();
    let ix = take_ix(&c, 4, c.mint_b, c.taker_ata_b, START - 1);
    assert!(send(&mut c.svm, ix, &taker).is_err());
}

#[test]
fn refund_returns_tokens_to_maker() {
    let mut c = with_offer(5);
    let maker = c.maker.insecure_clone();
    send(&mut c.svm, refund_ix(&c, 5, &c.maker.pubkey()), &maker).unwrap();
    assert_eq!(balance(&c.svm, &c.maker_ata_a), OFFERED * 2);
    assert!(c.svm.get_account(&offer_pda(&c.maker.pubkey(), 5)).is_none());
}

#[test]
fn make_rejects_zero_offered_amount() {
    let mut c = setup();
    let maker = c.maker.insecure_clone();
    let ix = make_ix(&c, 6, 0, START, END, DURATION, c.mint_b);
    assert!(send(&mut c.svm, ix, &maker).is_err());
}

#[test]
fn make_rejects_bad_price_range() {
    let mut c = setup();
    let maker = c.maker.insecure_clone();
    // start < end
    let ix = make_ix(&c, 7, OFFERED, END, START, DURATION, c.mint_b);
    assert!(send(&mut c.svm, ix, &maker).is_err());
    // zero floor
    let ix = make_ix(&c, 8, OFFERED, START, 0, DURATION, c.mint_b);
    assert!(send(&mut c.svm, ix, &maker).is_err());
}

#[test]
fn make_rejects_non_positive_duration() {
    let mut c = setup();
    let maker = c.maker.insecure_clone();
    let ix = make_ix(&c, 9, OFFERED, START, END, 0, c.mint_b);
    assert!(send(&mut c.svm, ix, &maker).is_err());
    let ix = make_ix(&c, 10, OFFERED, START, END, -5, c.mint_b);
    assert!(send(&mut c.svm, ix, &maker).is_err());
}

#[test]
fn make_rejects_same_mint() {
    let mut c = setup();
    let maker = c.maker.insecure_clone();
    let ix = make_ix(&c, 11, OFFERED, START, END, DURATION, c.mint_a);
    assert!(send(&mut c.svm, ix, &maker).is_err());
}

#[test]
fn refund_by_non_maker_fails() {
    let mut c = with_offer(12);
    let taker = c.taker.insecure_clone();
    // Taker signs as the "maker" account: seeds/has_one/ATA constraints must reject.
    assert!(send(&mut c.svm, refund_ix(&c, 12, &taker.pubkey()), &taker).is_err());
}

#[test]
fn take_with_wrong_mint_b_fails() {
    let mut c = with_offer(13);
    let maker = c.maker.insecure_clone();
    let taker = c.taker.insecure_clone();

    let other_mint = CreateMint::new(&mut c.svm, &maker).decimals(DECIMALS).send().unwrap();
    let other_ata = CreateAssociatedTokenAccount::new(&mut c.svm, &taker, &other_mint)
        .owner(&taker.pubkey())
        .send()
        .unwrap();
    MintTo::new(&mut c.svm, &maker, &other_mint, &other_ata, START).send().unwrap();

    // has_one = mint_b on the offer must reject a different mint.
    assert!(send(&mut c.svm, take_ix(&c, 13, other_mint, other_ata, START), &taker).is_err());
}

#[test]
fn take_twice_fails_after_offer_closed() {
    let mut c = with_offer(14);
    take_and_check(&mut c, 14, START, START);
    warp(&mut c.svm, 1);
    let taker = c.taker.insecure_clone();
    let ix = take_ix(&c, 14, c.mint_b, c.taker_ata_b, START);
    assert!(send(&mut c.svm, ix, &taker).is_err());
}
