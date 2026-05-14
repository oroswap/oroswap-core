# Revenue Router (Zigchain → Ethereum, via Noble CCTP)

## Overview

CosmWasm contract that holds USDC on Zigchain and lets an authorised keeper
trigger a bridge to a destination that is **pre-configured in `Config` by the
owner** — never specified by the caller. The contract is environment-agnostic:
the same code runs on testnet and mainnet; only the `Config` values differ.

The mainnet route is **Zigchain → Ethereum**. Zigchain does not speak CCTP
directly, so the transfer routes through Noble (Circle's native USDC hub):
the contract fires a single IBC `MsgTransfer` to Noble carrying a memo, and
Noble's **Orbiter** module reads that memo and forwards the USDC to Ethereum
over CCTP. The Noble hop is transparent — it is not a destination, just the
CCTP gateway. (The testnet route used a different intermediary purely for
validation; only the `Config` values differ.)

The memo is **fully static** — set once by the owner, used verbatim on every
bridge, never needs refreshing. See "Example `forward_memo`" below.

## Trust model

The destination is owner-set policy. The keeper holds an **execution trigger**,
not a **payout authorisation**.

| Threat | Outcome |
|---|---|
| Keeper key hijacked | Attacker can only trigger transfers to the **configured** treasury, capped at `max_bridge_per_tx` (if set) per call. Funds do not go to the attacker. |
| Owner key hijacked | Owner is the ultimate trust anchor; mitigate at that layer (multisig + timelock). |
| Wrong `forward_memo` configured | Owner runs `PreviewBridge` before unpausing; basic JSON shape is also validated on `UpdateConfig`. |

`BridgeUsdc` accepts only `amount`. The keeper cannot pass a memo or a
receiver. The first-hop receiver and the forward memo come from `Config`
and are used verbatim. A per-tx cap bounds how much can move per call even
when the destination is the configured treasury.

## Flow

```
USDC accumulates in contract (Zigchain)
            │
            ▼
  Keeper calls ExecuteMsg::BridgeUsdc { amount? }
            │
            ▼
  Contract fires ICS-20 MsgTransfer (Stargate) with:
    receiver = Config.first_hop_receiver       ← owner-set, immutable per call
                                                 (Noble Orbiter module address)
    memo     = Config.forward_memo             ← owner-set, immutable per call
                                                 (Orbiter CCTP forwarding memo)
            │
            ▼
  Noble — Orbiter module reads the memo, burns USDC via CCTP
            │
            ▼
  Ethereum — USDC minted to Config-defined treasury (mint_recipient in memo)
```

## Config

| Field | Notes |
|---|---|
| `owner` | Address — only role that can change destination / pause / withdraw |
| `usdc_denom` | USDC IBC denom on Zigchain |
| `ibc_channel` | Zigchain outbound channel to Noble (mainnet: `channel-3`) |
| `ibc_timeout_secs` | Packet timeout; ≤ 86400 |
| `first_hop_receiver` | IBC `MsgTransfer.receiver` — the **Noble Orbiter module address**, not the treasury. The treasury is encoded inside `forward_memo`. |
| `forward_memo` | Orbiter CCTP forwarding memo, inserted into `MsgTransfer.memo` verbatim. Static — set once. |
| `max_bridge_per_tx` | `Option<Uint128>` — max USDC per single `BridgeUsdc` call. `None` = uncapped. Bounds hijacked-keeper blast radius. |
| `paused` | If true, `BridgeUsdc` returns `Paused` |

### Example `forward_memo` — mainnet, Noble Orbiter CCTP to Ethereum

Noble's forwarding is handled by its **Orbiter** module. The contract treats
this memo as opaque JSON and validates only its basic shape — the structure
below is interpreted entirely by Noble.

```json
{
  "orbiter": {
    "forwarding": {
      "protocol_id": "PROTOCOL_CCTP",
      "attributes": {
        "@type": "/noble.orbiter.controller.forwarding.v1.CCTPAttributes",
        "destination_domain": 0,
        "mint_recipient": "<base64 of the treasury's 32-byte left-padded ETH address>",
        "destination_caller": ""
      },
      "passthrough_payload": ""
    }
  }
}
```

- `destination_domain: 0` is Ethereum (CCTP domain id).
- `mint_recipient` is the treasury's Ethereum address, left-padded to 32 bytes
  and base64-encoded — this is where the USDC lands.
- `destination_caller` empty = anyone may relay the CCTP message on Ethereum.
- This memo is **fully static**: nothing in it expires, so it is set once at
  instantiate (or via `UpdateConfig`) and never touched again.

> **Smart-relay note.** Skip's routing API can also produce an Orbiter memo
> with a `pre_actions` → `ACTION_FEE` block that pays a relayer to auto-deliver
> on Ethereum. That fee quote **expires (~15 min)** and is therefore unsuitable
> for a static `Config` memo — do not use that variant here. With the static
> memo above, the CCTP message is delivered manually: after the burn on Noble,
> treasury ops submits one `receiveMessage` call on Ethereum to mint. The CCTP
> attestation itself is free.

## Messages

### InstantiateMsg

| Field | Type | Description |
|---|---|---|
| `owner` | String | Contract owner |
| `usdc_denom` | String | USDC IBC denom on Zigchain |
| `ibc_channel` | String | Outbound IBC channel on Zigchain to Noble (mainnet: `channel-3`) |
| `ibc_timeout_secs` | u64? | Packet timeout (default: 600) |
| `first_hop_receiver` | String | Noble Orbiter module address (not the treasury) |
| `forward_memo` | String | Orbiter CCTP forwarding memo (static JSON) |
| `max_bridge_per_tx` | Uint128? | Per-call cap. Omit for uncapped (not recommended for prod). |

### ExecuteMsg

- `BridgeUsdc { amount? }` — keeper or owner. The destination is **not** an
  argument; it comes from `Config`. `amount: None` bridges the full balance.
- `UpdateConfig { usdc_denom?, ibc_channel?, ibc_timeout_secs?, first_hop_receiver?, forward_memo?, max_bridge_per_tx? }`
  — owner only. `usdc_denom` cannot be changed while the contract holds a
  non-zero balance of the current denom (`StrandedBalance`). `forward_memo`
  must parse as a JSON object. `max_bridge_per_tx` uses a double-`Option`
  so that the owner can distinguish *"leave alone"* (`None`), *"set to a
  new value"* (`Some(Some(x))`), and *"clear the cap"* (`Some(None)`).
  A zero cap is rejected — use `SetPaused` to halt bridging.
- `SetPaused { paused }` — owner only.
- `AddKeeper { keeper }` / `RemoveKeeper { keeper }` — owner only.
- `EmergencyWithdraw { denom, amount?, recipient? }` — owner only. Bypasses
  the IBC path; used to recover any token from the contract.
- `ProposeNewOwner { owner, expires_in }` / `DropOwnershipProposal {}` /
  `ClaimOwnership {}` — two-step ownership transfer.

### QueryMsg

- `Config {}` → `ConfigResponse` — full configuration including destination.
- `UsdcBalance {}` → `BalanceResponse` — contract's current USDC balance.
- `Keepers {}` → `KeepersResponse` — list of registered keepers.
- `PreviewBridge {}` → `PreviewBridgeResponse` — exactly what the next
  `BridgeUsdc` call would put on chain. Use this after `UpdateConfig` to
  sanity-check the destination policy before unpausing.

## Operational notes

1. Deploy with `--admin <owner>` so the owner controls migrations.
2. Set `ibc_channel` (`channel-3` on mainnet), `first_hop_receiver` (Noble
   Orbiter module address), and `forward_memo` (Orbiter CCTP memo with the
   treasury's ETH address as `mint_recipient`).
3. Call `PreviewBridge` and verify the response matches the intended route.
4. `SetPaused { paused: false }`, then have the keeper call `BridgeUsdc`.
5. After the burn lands on Noble, treasury ops submits one `receiveMessage`
   call on Ethereum to mint the USDC (manual CCTP delivery — see the
   smart-relay note under "Example `forward_memo`").

When rotating the keeper key, removing the compromised keeper with
`RemoveKeeper` is enough to lock the attacker out — they have no destination
authority to abuse in the meantime.

## Op-risk: wrong-denom stranding

`BridgeUsdc` operates exclusively on `Config.usdc_denom`. If a counterparty
or buggy upstream sends a **different** denom to the contract address (for
example, the wrong IBC trace, or LP tokens from a misrouted reward stream),
those funds are not automatically recoverable through the bridge path.

The contract is not vulnerable in any value-loss sense — `EmergencyWithdraw`
(owner-only) can sweep any denom out of the contract — but the recovery
relies on the owner noticing and acting. Recommended operational hygiene:

- Monitor `query bank balances <contract>` and alert on any denom other
  than `Config.usdc_denom`.
- Document the runbook for `EmergencyWithdraw` with the relevant cleanup
  destination(s) so an on-call operator doesn't have to figure it out in
  the moment.
- The on-chain `StrandedBalance` error specifically protects against an
  `UpdateConfig.usdc_denom` rotation that would leave the old denom's
  balance unreferenced by the bridge; do not work around it without
  withdrawing the old balance first.

## IBC failure modes — what reply tells you (and what it doesn't)

`bridge_usdc` wraps the outbound `MsgTransfer` in a `SubMsg::reply_always`.
The reply handler emits attributes for both success and failure:

| Attribute | Value | Meaning |
|---|---|---|
| `bridge_reply.ibc_transfer_status` | `queued` | The IBC module accepted the packet and queued it for relay. |
| `bridge_reply.ibc_transfer_status` | `failed` | The IBC module **rejected the send** (channel closed, insufficient fee, etc.). Tx still succeeds. |
| `bridge_reply.ibc_transfer_error` | string | Reason from the chain when status is `failed`. |
| `bridge_reply.bridge_{amount, receiver, channel, denom}` | strings | Echoed from the call context for indexers. |

What reply does **NOT** see:

- **Asynchronous ack failure** — Noble accepts the packet but the Orbiter
  module rejects the memo / forwarding on `OnRecvPacket`. Funds are refunded
  via the standard IBC ack flow, but this contract gets no callback.
- **Packet timeout** — the relayer fails to deliver before
  `timeout_timestamp` and the funds are refunded by IBC's timeout
  machinery. Again, no callback to this contract.

Treat `ibc_transfer_status=queued` as "packet in flight", not "delivered".
Downstream monitoring should follow the packet by sequence number on the
destination chains.
