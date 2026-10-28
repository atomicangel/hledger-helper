# hledger-helper

A lightweight, config-driven web UI for rapid hledger transaction entry.
Built in Rust with Axum, it is a mobile-first Progressive Web App (PWA) for
quick entry, split transactions, and budget tracking. Journals stay in plain
hledger files; the app reads and appends to them through the hledger CLI.

## Features

- **Quick Entry** — one row per split (Source / Expense / Amount), payee
  autocomplete, `+ Add Line` for additional splits, and a transfer toggle
  that routes to the transfer journal.
- **Credit-card auto-cover** — when a split's source matches
  `credit_locations` (cards, lines of credit, BNPL/point-of-sale
  financing), the app offers to cover the purchase from a budget envelope.
  The cover defaults to the envelope whose name matches the purchase
  category, or Unallocated when there is no unique match, and is written
  into `transfer_journal` alongside the purchase.
- **Budget tab** — envelope cards with monthly limits, spent totals, and
  remaining balances, grouped by header (Checking, Savings, ...), plus a
  Safe-to-Spend figure for the checking group.
- **Offline queue** — submissions made while offline are stored in
  IndexedDB and synced automatically when the app comes back online. A
  failed cover is queued with its purchase and replayed the same way.
- **PWA** — installable, with static assets embedded in the binary at
  compile time (no separate frontend server).

## Prerequisites

- [hledger](https://hledger.org/install) on your `PATH` (the app shells out
  to it for balances and for validating transactions before writing).
- A Rust toolchain (`cargo`).

## Quickstart (with the included demo data)

```bash
cp config.example.yaml config.yaml
cargo run --release
```

Then open http://localhost:8080. The example config points at
`./demo_journals`, a small fictional journal set, so you can try Quick
Entry, the Budget tab, and auto-cover without touching your own books.
Try a card purchase: pick the Example Card as the source and watch the
cover row appear; submitting writes the purchase to
`demo_journals/2026/demo.journal` and the cover to
`demo_journals/2026/transfers.journal`.

To use your own journals, edit `config.yaml` (see Configuration below) and
restart the server — configuration is loaded once at startup.

## Configuration

`config.yaml` drives behavior; there are no hard-coded accounts or rules.
Keys:

- `nas_path` — directory containing your journals. Relative paths are
  resolved from the working directory the server is started in. The demo
  uses `./demo_journals`.
- `top_journal` — entry-point journal file, relative to `nas_path`
  (usually `current.journal`). Balance queries run against this file, so
  any journal you want reflected in the UI must be included from it.
- `current_year` — year used when routing journal files.
- `budget_locations` — account prefixes whose accounts appear as budget
  envelopes (the Source dropdown and the Budget tab). Envelopes live under
  these prefixes, e.g. `assets:Bank:Checking:Budget:Groceries`.
- `liability_locations` — account prefixes shown as liabilities
  (e.g. `liabilities:CC:` for credit cards).
- `expense_locations` — account prefixes shown in the Expense dropdown
  (e.g. `expenses:`).
- `payment_sources` — journal routing for source accounts. Keys are
  account prefixes; the longest match wins, and anything unmatched falls
  back to `top_journal`. Each value has a `label` (shown in the UI) and a
  `journal` (file path relative to `nas_path`):
  ```yaml
  payment_sources:
    "assets:Bank:Checking":
      label: "Checking"
      journal: "2026/checking.journal"
  ```
- `credit_locations` — prefixes for charge-now/pay-later sources (cards,
  lines of credit, BNPL/point-of-sale financing). A Quick Entry split
  whose source matches one of these shows the auto-cover UI. Paydown-only
  debts (student loans, mortgages, tax debts) are not spending sources
  and must not be listed here. If the key is absent, there is no cover UI.
- `safe_to_spend_account` — the account shown as Safe-to-Spend
  (conventionally the Unallocated envelope).
- `safe_to_spend_header` — the header group whose underwater
  (negative-balance) envelopes count against Safe-to-Spend (default
  `Checking`).
- `transfer_journal` — **required**. File (relative to `nas_path`) that
  receives transfer-mode entries and auto-covers. The server refuses to
  start if this key is missing or empty, and creates the file (with
  parent directories) at startup if it does not exist yet. Include it
  from `top_journal` so covers show up in balances.
- `server` — `bind_address` and `port` (the example binds `0.0.0.0:8080`).
- `budget_entries` — the envelope/liability cards on the Budget tab:
  `account`, `label`, `amount` (monthly limit; negative for a liability
  credit line), `header` (group), and `type` (`budget` or `liability`).

## Journal layout

A typical layout under `nas_path`:

```
current.journal
2026/
  demo.journal
  transfers.journal
```

`current.journal` includes the year files:

```
include 2026/demo.journal
include 2026/transfers.journal
```

Routing is based on source-account prefixes from `payment_sources`; the
transfer journal is used for transfer-mode entries and auto-covers.
Transactions are validated with `hledger check` before they are appended,
and amounts are aligned to column 62.

## Development

```bash
cargo build --release
cargo test
```

## License

Dual-licensed under MIT OR Apache-2.0, at your choice. See `LICENSE-MIT`
and `LICENSE-APACHE`. The `license` field in `Cargo.toml` records this as
`MIT OR Apache-2.0`.
