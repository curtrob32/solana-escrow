# Dutch Auction Escrow

**Let the market find the price, not a guess.**

A trustless, on-chain Dutch-auction swap for Solana. You lock up a token, name a starting price and a floor, and the price falls on its own until a buyer decides it's worth it. No order book. No intermediary. No one holding your funds but code that can't run off with them.

Built with Anchor 1.x. Works with both SPL Token and Token-2022.

> **Status: early.** The program and tests are written and the security linter is clean, but the code has **not yet been compiled or run** (see [Status](#status-and-honesty)). Treat it as a working draft, not audited software.

---

## Why this exists

### The problem: nobody knows what a token is worth

Selling a token you hold sounds simple. It isn't. Pick the price too high and nothing happens for days. Pick it too low and you hand a stranger free money, and you'll never know how much. For thin or new tokens there's often no market price at all to check against.

The usual fixes are all uncomfortable:

- **Fixed-price swaps** force you to guess. Every guess is wrong in one direction or the other.
- **Order books** need liquidity, market makers and constant attention.
- **AMM pools** need you to seed and expose capital to impermanent loss, and they price off a formula, not off what buyers actually will pay.
- **OTC deals** need trust, a middleman, or both.

### The idea: let time do the price discovery

A Dutch auction flips the question. Instead of asking *"what should I charge?"*, it asks *"how long until someone says yes?"*

Start high. Let the price fall steadily. The first buyer who thinks the current price is a bargain takes it, and that moment *is* the market's answer. The seller never has to guess. The buyer never has to bid against anyone. The price they pay is the first price at which anyone was willing to buy.

That one mechanism gives you things that are hard to get any other way:

- **Honest price discovery.** The sale happens at the first price a real buyer accepts, not at a number someone invented.
- **No bidding wars.** A buyer can't be outbid, because the first taker wins. Waiting is the only risk, and the buyer chooses how much of it to take.
- **A built-in floor.** The seller sets the lowest price they'll accept, so the downside is capped before the auction starts.
- **No capital sitting idle in a pool.** You lock one token, once, for one purpose.

### Why on-chain, and why Solana

The whole point of an escrow is that you don't have to trust the other side. A program that holds the tokens, checks the price and settles the swap in a single atomic transaction removes the trust problem entirely: either both sides get paid, or nothing moves. There's no "I sent mine, where's yours?"

Solana makes this cheap and fast enough to be practical. The clock is available to every program, transactions settle in seconds, and fees are small enough that a one-off OTC-style trade doesn't cost more in fees than it's worth.

### Why I built it

Because the first decent solution to "how do I sell this fairly?" shouldn't require a trading desk. A tool this small and this honest should exist as a primitive anyone can read in an afternoon, audit in a week, and build on: auctions, token launches, treasury sales, NFT-style drops for fungible tokens, or just a better way for two people to trade.

It's also a deliberately compact example of doing Solana programs *properly*: every account validated by constraints, checked math, PDA-owned vaults, events for every state change, and tests for every way it should fail.

---

## How it works

**Example.** Alice sells 1,000,000 units of token A. She sets a starting price of 500,000 token B, falling to a floor of 100,000 over 1,000 seconds.

| When a buyer shows up | They pay |
|---|---|
| Second 0 | 500,000 |
| Second 500 | 300,000 |
| Second 1,000 or later | 100,000 |

The price falls in a straight line:

```
price = start_price - (start_price - end_price) * elapsed / duration
```

The decay is floored, so the price is rounded **up** (in the seller's favor). The price never drops below `end_price`.

### The three instructions

**`make_offer(id, offered_amount, start_price, end_price, duration_secs)`**
The maker creates the auction. Their token A moves into a **vault** that only the program can spend from, and an **offer account** records the terms. The offer's address is derived from the maker's key and an `id` they choose, so one maker can run many auctions at once without collisions.

**`take_offer(max_price)`**
A buyer completes the swap. The program reads the clock and computes the current price. If it's above the buyer's `max_price`, the transaction **fails**, so a buyer can never pay more than they saw. Otherwise, in one transaction: the buyer pays the maker in token B, receives the vault's token A, and both temporary accounts are closed with their rent returned to the maker.

**`refund`**
The maker cancels and reclaims their token A. Only the maker can do this.

---

## Security design

| Concern | How it's handled |
|---|---|
| Who can spend the vault? | Only the program. The vault's owner is the offer's PDA, so there is no private key for anyone to steal. |
| Fake accounts | The maker, both mints and the vault are pinned by Anchor constraints (`has_one`, `seeds`, `bump`, associated-token checks). A buyer can't slip in a cheaper mint. |
| Taking an offer twice | The offer account is closed on take, so there's nothing left to spend. |
| Overpaying | `max_price` is slippage protection for the buyer. |
| Arithmetic | `checked_*` math with `u128` intermediates, mapped to custom errors. Rounding favors the seller. |
| Token-2022 transfer fees | The program pays out the vault's *actual* balance, so fees taken on deposit can't strand dust and block closing the vault. |
| Bad inputs | Rejects zero amounts, an inverted price range (`start < end`), a zero floor, a non-positive duration, and identical mints. |
| Observability | `OfferMade`, `OfferTaken` and `OfferRefunded` events are emitted for every state change. |

One note for reviewers: `take_offer` uses `init_if_needed`, but only to create the taker's token-A account and the maker's token-B account if they don't exist. That is idempotent and holds no program state, but it's the kind of thing worth a second look.

---

## Project layout

```
Anchor.toml, Cargo.toml
programs/escrow/
  src/
    lib.rs                      program entrypoint
    constants.rs, errors.rs, events.rs
    state/offer.rs              the Offer account + current_price()
    instructions/
      make_offer.rs
      take_offer.rs
      refund.rs
  tests/escrow.rs               LiteSVM integration tests
```

## Build and test

Requires Rust, the Solana CLI (Agave), and Anchor 1.x. The easiest path is a Linux environment (WSL or a GitHub Codespace).

```bash
# install the toolchain
curl --proto '=https' --tlsv1.2 -sSfL https://solana-install.solana.workers.dev | bash

# build, then sync the placeholder program ID, then rebuild
NO_DNA=1 anchor build
anchor keys sync
NO_DNA=1 anchor build

# run the tests
cargo test -p escrow
```

The tests cover paying the start price, mid-auction decay, the floor after the end, the `max_price` rejection, refund, every input validation, a refund by a non-maker, a wrong mint, and taking an offer twice. Two pure-Rust unit tests check the price curve without needing the built program.

## Status and honesty

- The security linter reported no issues, but the program has **not been compiled or run yet**. Expect small fixes on the first build, especially around the test crate versions.
- The program ID in `lib.rs` is a **placeholder**. Run `anchor keys sync`.
- **Not done yet:** a generated TypeScript client, compute-unit budget assertions, a frontend, an independent audit, and any deployment.
- **Do not deploy this to mainnet** as is.

## Roadmap

- Compile, run and fix the test suite
- Generate a Codama TypeScript client
- Compute-unit budgets per instruction
- Optional exponential or stepped decay curves
- A small frontend that shows the falling price live
- Independent security review before any real funds

## License

MIT, unless you decide otherwise.
