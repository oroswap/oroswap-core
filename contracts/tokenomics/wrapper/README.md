# Oroswap Token Wrapper

The Wrapper contract lets users lock a native, IBC, or tokenfactory denom in
exchange for an equal amount of a freshly-minted ZigChain tokenfactory token
(e.g. lock ibc/233..23deef → receive coin.{contract_address}.ORO). It enforces a strict 1:1 invariant between
the locked balance and the minted supply.

## Features

- **1:1 Token Mapping** — every minted token is backed by exactly one locked
  native token. The contract balance of `lock_denom` always equals (or exceeds,
  via accidental direct sends) the total supply of `mint_denom`.
- **TokenFactory Integration** — creates the minted denom at instantiate time
  and writes ZigChain `Metadata` with a caller-supplied name, symbol,
  description, and URI.
- **Two-step ownership transfer** — `ProposeNewOwner` + `ClaimOwnership` with
  an expiry, so a typo or compromised key doesn't permanently strand the
  contract.
- **Pausable** — owner-driven plus an optional set of pause-authority wallets
  that can trigger emergency pauses but cannot unpause. Modelled on the
  `factory` contract.
- **Receiver support** — Lock/Unlock both accept an optional `receiver`, so
  one address can pay and another can receive.
- **Excess recovery** — the owner can sweep `lock_denom` tokens that were
  bank-sent directly to the contract (bypassing `Lock`), restoring the 1:1
  invariant.
- **Retirement-ready** — `ProposeDenomAdmin` lets the owner hand off the
  ZigChain bank-admin and metadata-admin roles of the minted denom to a
  successor, completing the chain-level two-step admin handover.

## Three independent admin roles

These are commonly conflated; the wrapper keeps them separate by design.

| Role | Stored where | Governs | Changed via |
|---|---|---|---|
| **wasm contract admin** | wasmd `ContractInfo.admin` (set at instantiate via `--admin`) | `MsgMigrateContract` and `MsgUpdateAdmin` itself | `wasmd MsgUpdateAdmin` / `MsgClearAdmin` directly — not exposed by the wrapper |
| **wrapper `config.owner`** | this contract's storage | Pause, AddPauseAuthorities, UpdateDenomMetadata, ProposeDenomAdmin, RecoverExcess, ProposeNewOwner | `ProposeNewOwner` + `ClaimOwnership` |
| **denom bank/metadata admin** | ZigChain factory module storage | `mint`, `burn`, `set-metadata` for the minted denom | `ProposeDenomAdmin` (wrapper) + `MsgClaimDenomAdmin` (signed by the new admin themselves) |

Multisig addresses work natively in every role — the contract only does
bech32 equality checks on `info.sender`, and the chain treats multisig
accounts like any other signer.

## Deploying

The wrapper creates a new tokenfactory denom in its `instantiate` reply,
which costs **1000uzig** under the ZigChain factory module. The fee is paid
by the contract account, so the contract must be funded at instantiate time
via `--amount 1000uzig`:

```bash
zigchaind tx wasm instantiate $CODE_ID '{
  "lock_denom": "ibc/F1073E5895A25347EF10A7B50783D3048882726895AEE1176C498C45D0576F2B",
  "mint_subdenom": "oro",
  "mint_decimals": 6,
  "mint_name": "Oroswap ORO",
  "mint_symbol": "ORO",
  "mint_description": "Oroswap ORO token",
  "mint_uri": "https://example.com/oro.json",
  "owner": "zig1..."
}' --label "wrapper" --admin zig1... --amount 1000uzig \
   --from $KEY --keyring-backend test --node $RPC --chain-id $CHAIN \
   --gas-prices 0.25uzig --gas auto --gas-adjustment 1.5 -y
```

After instantiate, the minted denom will be
`coin.<contract_address>.oro`.

`mint_name`, `mint_symbol`, `mint_description`, and `mint_uri` are
optional. When omitted the contract falls back to `"Oroswap <subdenom>"`,
`"<subdenom>"`, and `"Oroswap <subdenom> Token"` respectively
(`mint_uri` stays empty).

`mint_decimals` is **required** — no default. It must match the
`lock_denom`'s source-token decimals (e.g. 6 for USDC, 18 for most
ERC-20s) so that the 1:1 wrap-mint displays correctly in wallets.
The chain bakes this exponent into the denom's `Metadata.denom_units`
at create-time and the contract refuses to change it later, so getting
it right here is critical.

## ExecuteMsg

### `Lock { receiver }`
Sends `mint_denom` 1:1 for the `lock_denom` you attach. If `receiver` is
omitted the minted tokens go to the sender.

```json
{ "lock": { "receiver": "zig1..." } }
```

Attach the locked amount via `--amount <n><lock_denom>`.

### `Unlock { receiver }`
Burns the attached `mint_denom` and sends the equivalent `lock_denom` back.
If `receiver` is omitted the unlocked tokens go to the sender.

```json
{ "unlock": { "receiver": "zig1..." } }
```

Attach the burn amount via `--amount <n><mint_denom>`.

### `Pause {}` / `Unpause {}`
- `Pause` — callable by the owner or any address in `PauseAuthorities`.
- `Unpause` — owner only. A compromised pause-authority can't grief by
  toggling pause state.

While paused, `Lock` and `Unlock` return `ContractPaused`. All other
operations remain available.

### `AddPauseAuthorities { authorities }` / `RemovePauseAuthorities { authorities }`
Owner only. Manages the set of addresses (in addition to the owner) that
can pause the contract.

```json
{ "add_pause_authorities": { "authorities": ["zig1...", "zig1..."] } }
```

### `ProposeNewOwner { owner, expires_in }` / `DropOwnershipProposal {}` / `ClaimOwnership {}`
Standard two-step ownership transfer (shared with other Oroswap contracts).
`expires_in` is seconds from the proposal's block time.

### `RecoverExcess {}`
Owner only. Returns to the owner any `lock_denom` whose balance exceeds
`MintSupply` — typically because someone bank-sent the denom directly
to the contract without going through `Lock`. Errors with
`"No excess tokens to recover"` when the invariant already holds.

### `UpdateDenomMetadata { name, symbol, description, uri }`
Owner only. **Patch semantics**: any field left as `null` keeps the
value currently stored on-chain; only `Some(_)` fields are overwritten.
So `{ "uri": "https://new/logo.png" }` updates the URI alone and leaves
name/symbol/description untouched — even if they were customised at
instantiate.

Under the hood the ZigChain factory module rewrites Metadata as a whole
on each `MsgSetDenomMetadata`, so the contract keeps an authoritative
copy in storage, overlays the supplied fields onto it, and re-encodes
the full Metadata on every call. `decimals` is sourced from
`config.mint_decimals` (set once at instantiate, immutable) — never a
hardcoded value.

### `ProposeDenomAdmin { bank_admin, metadata_admin }`
Owner only. Begins the two-step transfer of the minted denom's bank-admin
and metadata-admin roles to new addresses.

> ⚠️ **Run this AFTER mint supply has been fully drained.** Once the new
> admin claims, this wrapper can no longer mint or burn the denom, so any
> outstanding `mint_denom` held by users becomes **unredeemable through
> this contract** — `Unlock` calls will fail at the chain layer with
> `incorrect admin for denom`. The correct order is:
>
> 1. `Pause` — stop new locks.
> 2. **Wait for `MintSupply` to drain to zero** (users unwind their
>    positions, or pre-arrange unlock with affected holders).
> 3. `ProposeDenomAdmin` — only now hand off the admin roles.
>
> Skipping step 2 bricks every remaining holder. See "Retiring a wrapper"
> below for the full runbook.

The proposed `bank_admin` and `metadata_admin` must then sign
`MsgClaimDenomAdmin` directly on chain from their own addresses — not
through this contract. This is intentional: the chain enforces that the
signer of the claim is the one who becomes the admin, which is what makes
the handover safe.

```bash
# Step 1 (wrapper owner):
zigchaind tx wasm execute $WRAPPER '{
  "propose_denom_admin": {
    "bank_admin":     "zig1...",
    "metadata_admin": "zig1..."
  }
}' --from $OWNER_KEY ...

# Step 2 (new admin, from their own address):
zigchaind tx factory claim-denom-admin $MINT_DENOM \
  --from $NEW_ADMIN_KEY ...
```

After the claim, the wrapper loses mint/burn/metadata authority. `Lock`
calls will fail at the chain level with `incorrect admin for denom`.

## QueryMsg

### `Config {}`
Returns the full `Config { lock_denom, mint_denom, mint_decimals, owner, paused }`.

### `TotalLocked {}`
Returns the wrapper's current balance of `lock_denom` as `Uint128`. If
the 1:1 invariant is intact this equals `MintSupply`; otherwise the
difference is exactly the amount recoverable via `RecoverExcess`.

### `MintSupply {}`
Returns the total circulating supply of `mint_denom` as `Uint128`.

### `PauseAuthorities {}`
Returns the list of addresses currently authorised to pause the contract.
The owner has implicit pause authority and is NOT included in this list.

## Migration

Forward-only via `cw2::ensure_from_older_version`: `migrate` succeeds when
the stored cw2 name is `oroswap-wrapper` AND the stored version is `<=
CONTRACT_VERSION`. A name mismatch or a downgrade returns a generic
`StdError` wrapped into `ContractError::Std`; an equal stored version is a
silent no-op. Wasm-level migrate rights are gated by the wasm contract
admin (see the admin-roles table above), not by `config.owner`.

## Retiring a wrapper

End-to-end retirement is three separate handovers. **Order matters**: the
denom-admin handover must come last, after circulating `mint_denom` has
been fully redeemed, otherwise any remaining holder is permanently locked
out of their underlying `lock_denom`.

Operational sequence:

1. **`Pause`** — stops new `Lock` calls so circulating supply can only
   shrink from here.
2. **Drain `MintSupply`** — wait for / facilitate users to `Unlock` their
   positions. Confirm with `query MintSupply {}` that it has reached
   zero (or the residual you're willing to write off, e.g. lost keys).
3. **Denom admins** — `ProposeDenomAdmin` (wrapper owner) + external
   `MsgClaimDenomAdmin` (new admin). Required if the successor needs to
   keep minting/burning the denom. After this step the wrapper can no
   longer process `Unlock`.
4. **Wrapper internal owner** — `ProposeNewOwner` + `ClaimOwnership`.
   Optional; only matters if the wrapper will remain administratively
   useful (e.g. `RecoverExcess`, future `Unpause`).
5. **Wasm contract admin** — direct `wasmd MsgUpdateAdmin` (transfer) or
   `MsgClearAdmin` (freeze migrations permanently). One-way once cleared.

If you must hand off the denom admin while `MintSupply > 0` (e.g. emergency
migration), the successor admin is responsible for honouring the
outstanding 1:1 claims — there is no on-chain mechanism to compel them.
