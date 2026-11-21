//! LiteSVM tests for the escrow program.
//! Run `NO_DNA=1 anchor build` first so `target/deploy/escrow.so` exists.
//! NOTE: written without a local toolchain; crate versions/APIs may need small tweaks.

use anchor_lang::{system_program, InstructionData, ToAccountMetas};
use anchor_spl::{
    associated_token::{self, get_associated_token_address},
    token,
};
use litesvm::LiteSVM;
use litesvm_token::{spl_token, CreateAssociatedTokenAccount, CreateMint, MintTo};
use solana_keypair::Keypair;
use solana_message::Message;
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use solana_transaction::Transaction;

const OFFERED: u64 = 1_000_000;
const WANTED: u64 = 250_000;
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
    MintTo::new(&mut svm, &maker, &mint_b, &taker_ata_b, WANTED * 2).send().unwrap();

    Ctx { svm, maker, taker, mint_a, mint_b, maker_ata_a, taker_ata_b }
}

fn make_ix(c: &Ctx, id: u64, offered: u64, wanted: u64, mint_b: Pubkey) -> solana_instruction::Instruction {
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
        data: escrow::instruction::MakeOffer { id, offered_amount: offered, wanted_amount: wanted }.data(),
    }
}

fn take_ix(c: &Ctx, id: u64, mint_b: Pubkey, taker_ata_b: Pubkey) -> solana_instruction::Instruction {
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
        data: escrow::instruction::TakeOffer {}.data(),
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

fn send(
    svm: &mut LiteSVM,
    ix: solana_instruction::Instruction,
    signer: &Keypair,
) -> Result<(), String> {
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

use solana_program_pack::Pack;

#[test]
fn make_then_take_swaps_tokens_and_closes_accounts() {
    let mut c = setup();
    let ix = make_ix(&c, 1, OFFERED, WANTED, c.mint_b);
    let maker = c.maker.insecure_clone();
    send(&mut c.svm, ix, &maker).unwrap();

    let offer = offer_pda(&c.maker.pubkey(), 1);
    let vault = get_associated_token_address(&offer, &c.mint_a);
    assert_eq!(balance(&c.svm, &vault), OFFERED);
    assert_eq!(balance(&c.svm, &c.maker_ata_a), OFFERED); // minted 2x, deposited 1x

    let ix = take_ix(&c, 1, c.mint_b, c.taker_ata_b);
    let taker = c.taker.insecure_clone();
    send(&mut c.svm, ix, &taker).unwrap();

    let taker_a = get_associated_token_address(&c.taker.pubkey(), &c.mint_a);
    let maker_b = get_associated_token_address(&c.maker.pubkey(), &c.mint_b);
    assert_eq!(balance(&c.svm, &taker_a), OFFERED);
    assert_eq!(balance(&c.svm, &maker_b), WANTED);
    assert!(c.svm.get_account(&vault).is_none(), "vault should be closed");
    assert!(c.svm.get_account(&offer).is_none(), "offer should be closed");
}

#[test]
fn refund_returns_tokens_to_maker() {
    let mut c = setup();
    let maker = c.maker.insecure_clone();
    send(&mut c.svm, make_ix(&c, 2, OFFERED, WANTED, c.mint_b), &maker).unwrap();
    send(&mut c.svm, refund_ix(&c, 2, &c.maker.pubkey()), &maker).unwrap();

    assert_eq!(balance(&c.svm, &c.maker_ata_a), OFFERED * 2);
    assert!(c.svm.get_account(&offer_pda(&c.maker.pubkey(), 2)).is_none());
}

#[test]
fn make_rejects_zero_amounts() {
    let mut c = setup();
    let maker = c.maker.insecure_clone();
    assert!(send(&mut c.svm, make_ix(&c, 3, 0, WANTED, c.mint_b), &maker).is_err());
    assert!(send(&mut c.svm, make_ix(&c, 4, OFFERED, 0, c.mint_b), &maker).is_err());
}

#[test]
fn make_rejects_same_mint() {
    let mut c = setup();
    let maker = c.maker.insecure_clone();
    assert!(send(&mut c.svm, make_ix(&c, 5, OFFERED, WANTED, c.mint_a), &maker).is_err());
}

#[test]
fn refund_by_non_maker_fails() {
    let mut c = setup();
    let maker = c.maker.insecure_clone();
    let taker = c.taker.insecure_clone();
    send(&mut c.svm, make_ix(&c, 6, OFFERED, WANTED, c.mint_b), &maker).unwrap();
    // Taker signs as the "maker" account: seeds/has_one/ATA constraints must reject.
    assert!(send(&mut c.svm, refund_ix(&c, 6, &taker.pubkey()), &taker).is_err());
}

#[test]
fn take_with_wrong_mint_b_fails() {
    let mut c = setup();
    let maker = c.maker.insecure_clone();
    let taker = c.taker.insecure_clone();
    send(&mut c.svm, make_ix(&c, 7, OFFERED, WANTED, c.mint_b), &maker).unwrap();

    let other_mint = CreateMint::new(&mut c.svm, &maker).decimals(DECIMALS).send().unwrap();
    let other_ata = CreateAssociatedTokenAccount::new(&mut c.svm, &taker, &other_mint)
        .owner(&taker.pubkey())
        .send()
        .unwrap();
    MintTo::new(&mut c.svm, &maker, &other_mint, &other_ata, WANTED).send().unwrap();

    // has_one = mint_b on the offer must reject a different mint.
    assert!(send(&mut c.svm, take_ix(&c, 7, other_mint, other_ata), &taker).is_err());
}

#[test]
fn take_twice_fails_after_offer_closed() {
    let mut c = setup();
    let maker = c.maker.insecure_clone();
    let taker = c.taker.insecure_clone();
    send(&mut c.svm, make_ix(&c, 8, OFFERED, WANTED, c.mint_b), &maker).unwrap();
    send(&mut c.svm, take_ix(&c, 8, c.mint_b, c.taker_ata_b), &taker).unwrap();
    c.svm.expire_blockhash();
    assert!(send(&mut c.svm, take_ix(&c, 8, c.mint_b, c.taker_ata_b), &taker).is_err());
}
