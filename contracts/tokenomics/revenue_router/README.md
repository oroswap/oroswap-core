# Revenue Router (Zigchain → Ethereum)

## Overview

CosmWasm contract that holds USDC on Zigchain and lets an authorized keeper bridge it to Ethereum via [Skip API](https://docs.skip.build) IBC routing.

Because Zigchain does not support CCTP, bridging uses IBC with Skip's Packet Forward Middleware (PFM) memo: **Zigchain → Noble → Ethereum**.

## Flow

```
USDC accumulates in contract (Zigchain)
            │
            ▼
  Keeper queries Skip API off-chain
  POST /v2/fungible/route  → get route
  POST /v2/fungible/msgs   → get PFM memo + receiver
            │
            ▼
  Keeper calls ExecuteMsg::BridgeUsdc { memo, receiver, amount? }
            │
            ▼
  Contract fires ICS-20 MsgTransfer (stargate) with PFM memo
            │
            ▼
  Noble (USDC native chain)
            │
            ▼
  Ethereum (final receiver)
```

## Keeper Responsibilities

1. Check balance: `QueryMsg::UsdcBalance`
2. Get route from Skip API:
   ```json
   POST https://go.skip.build/api/skip/v2/fungible/route
   {
     "source_asset_denom": "<usdc_ibc_denom_on_zigchain>",
     "source_asset_chain_id": "zigchain-1",
     "dest_asset_denom": "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
     "dest_asset_chain_id": "1",
     "amount_in": "<amount>",
     "smart_relay": true
   }
   ```
3. Get messages from Skip API:
   ```json
   POST https://go.skip.build/api/skip/v2/fungible/msgs
   {
     "source_asset_denom": "...",
     "source_asset_chain_id": "zigchain-1",
     "dest_asset_denom": "...",
     "dest_asset_chain_id": "1",
     "amount_in": "...",
     "amount_out": "...",
     "address_list": ["<contract_addr>", "<noble_intermediary>", "<eth_receiver>"],
     "operations": [...],
     "slippage_tolerance_percent": "1"
   }
   ```
4. Extract `txs[0].cosmos_tx.msgs[0]` — the `memo` and `to_address` fields.
5. Call `ExecuteMsg::BridgeUsdc { memo, receiver, amount? }` on this contract.

## Messages

### InstantiateMsg

| Field | Type | Description |
|---|---|---|
| `owner` | String | Contract owner |
| `usdc_denom` | String | USDC IBC denom on Zigchain |
| `dest_chain_id` | String | Ethereum chain ID (`"1"`) |
| `dest_usdc_denom` | String | USDC address on Ethereum |
| `dest_receiver` | String | Final Ethereum receiver |
| `ibc_channel` | String | Outbound IBC channel on Zigchain |
| `ibc_timeout_secs` | u64? | Packet timeout in seconds (default: 600) |

### ExecuteMsg

- `BridgeUsdc { amount?, memo, receiver }` — bridge USDC (keeper/owner only).
- `UpdateConfig { ... }` — update config fields (owner only).
- `AddKeeper { keeper }` / `RemoveKeeper { keeper }` — manage keepers (owner only).
- `EmergencyWithdraw { denom, amount?, recipient? }` — recover funds (owner only).

### QueryMsg

- `Config {}` → `ConfigResponse`
- `UsdcBalance {}` → `BalanceResponse`
- `Keepers {}` → `KeepersResponse`
