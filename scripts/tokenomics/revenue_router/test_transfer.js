/**
 * One-shot test: transfer 1 USDC from your Zigchain wallet to Osmosis testnet
 * via Skip API (zig-test-2 → grand-1 → osmo-test-5).
 *
 * Run: node test_transfer.js
 */

import { SigningStargateClient, defaultRegistryTypes } from "@cosmjs/stargate";
import { DirectSecp256k1HdWallet, Registry } from "@cosmjs/proto-signing";
import { GasPrice } from "@cosmjs/stargate";
import { MsgTransfer } from "cosmjs-types/ibc/applications/transfer/v1/tx";
import Long from "long";

const MNEMONIC = "begin cool flush invest sketch syrup mail relief liquid bounce urban sight";
const RPC      = "https://public-zigchain-testnet-rpc.numia.xyz";
const SKIP_API = "https://go.skip.build/api/skip";

const USDC_DENOM = "ibc/5260516290F7883EC893AADA09A6B8CEC790F2EEF3196F440037908749785BE8";
const AMOUNT     = "1000000"; // 1 USDC

const SRC_CHAIN  = "zig-test-2";
const DEST_CHAIN = "osmo-test-5";
const DEST_DENOM = "ibc/DE6792CF9E521F6AD6E9A4BDF6225C9571A3B74ACC0A529F92BC5122A39D2E58";

async function main() {
  // ── 1. Setup wallet ────────────────────────────────────────────────────────
  const zigWallet   = await DirectSecp256k1HdWallet.fromMnemonic(MNEMONIC, { prefix: "zig" });
  const nobleWallet = await DirectSecp256k1HdWallet.fromMnemonic(MNEMONIC, { prefix: "noble" });
  const osmoWallet  = await DirectSecp256k1HdWallet.fromMnemonic(MNEMONIC, { prefix: "osmo" });

  const [zigAcc]   = await zigWallet.getAccounts();
  const [nobleAcc] = await nobleWallet.getAccounts();
  const [osmoAcc]  = await osmoWallet.getAccounts();

  console.log("zig  :", zigAcc.address);
  console.log("noble:", nobleAcc.address);
  console.log("osmo :", osmoAcc.address);

  // Register IBC transfer message type
  const registry = new Registry([
    ...defaultRegistryTypes,
    ["/ibc.applications.transfer.v1.MsgTransfer", MsgTransfer],
  ]);

  const client = await SigningStargateClient.connectWithSigner(RPC, zigWallet, {
    gasPrice: GasPrice.fromString("0.025uzig"),
    registry,
  });

  // ── 2. Get route ───────────────────────────────────────────────────────────
  console.log("\n📡 Getting route from Skip...");
  const routeRes = await fetch(`${SKIP_API}/v2/fungible/route`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      source_asset_denom: USDC_DENOM,
      source_asset_chain_id: SRC_CHAIN,
      dest_asset_denom: DEST_DENOM,
      dest_asset_chain_id: DEST_CHAIN,
      amount_in: AMOUNT,
      smart_relay: true,
      allow_multi_tx: true,
      allow_unsafe: true,
    }),
  });
  const route = await routeRes.json();
  if (!routeRes.ok) throw new Error(`Route failed: ${route.message}`);
  console.log(`Route: ${route.chain_ids.join(" → ")}, amount_out=${route.amount_out}`);

  // ── 3. Get msgs ────────────────────────────────────────────────────────────
  console.log("📡 Getting msgs from Skip...");

  const addressMap = {
    "zig-test-2":  zigAcc.address,
    "grand-1":     nobleAcc.address,
    "osmo-test-5": osmoAcc.address,
  };
  const addressList = route.required_chain_addresses.map(id => addressMap[id]);
  console.log("address_list:", addressList);

  const msgsRes = await fetch(`${SKIP_API}/v2/fungible/msgs`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      source_asset_denom: route.source_asset_denom,
      source_asset_chain_id: route.source_asset_chain_id,
      dest_asset_denom: route.dest_asset_denom,
      dest_asset_chain_id: route.dest_asset_chain_id,
      amount_in: route.amount_in,
      amount_out: route.amount_out,
      address_list: addressList,
      operations: route.operations,
      slippage_tolerance_percent: "1",
      chain_ids_to_affiliates: {},
    }),
  });
  const msgsData = await msgsRes.json();
  if (!msgsRes.ok) throw new Error(`Msgs failed: ${msgsData.message}`);

  const cosmosTx = msgsData.txs?.[0]?.cosmos_tx;
  if (!cosmosTx) throw new Error("No cosmos_tx in response");

  // Skip returns msg as a JSON string of the raw MsgTransfer fields
  const ibcMsg   = cosmosTx.msgs[0];
  const msgValue = JSON.parse(ibcMsg.msg);

  console.log("\n📋 MsgTransfer details:");
  console.log("  channel  :", msgValue.source_channel);
  console.log("  receiver :", msgValue.receiver);
  console.log("  memo     :", msgValue.memo);

  // ── 4. Broadcast using cosmjs MsgTransfer type ────────────────────────────
  console.log("\n🚀 Broadcasting MsgTransfer...");

  const msg = {
    typeUrl: "/ibc.applications.transfer.v1.MsgTransfer",
    value: MsgTransfer.fromPartial({
      sourcePort:    "transfer",
      sourceChannel: msgValue.source_channel,
      token: {
        denom:  USDC_DENOM,
        amount: AMOUNT,
      },
      sender:           zigAcc.address,
      receiver:         msgValue.receiver,
      timeoutHeight:    {},
      timeoutTimestamp: Long.fromString(msgValue.timeout_timestamp.toString()),
      memo:             msgValue.memo ?? "",
    }),
  };

  const result = await client.signAndBroadcast(zigAcc.address, [msg], "auto");

  console.log("\n✅ TX broadcast!");
  console.log("  hash:", result.transactionHash);
  console.log("  code:", result.code);

  if (result.code !== 0) {
    console.error("  rawLog:", result.rawLog);
    return;
  }

  console.log(`\n  Explorer: https://testnet.mintscan.io/zigchain/tx/${result.transactionHash}`);

  // ── 5. Track via Skip ──────────────────────────────────────────────────────
  console.log("\n🔍 Tracking (polls every 6s, up to 3 min)...");
  const TERMINAL = ["STATE_COMPLETED_SUCCESS", "STATE_COMPLETED_ERROR", "STATE_ABANDONED"];

  for (let i = 0; i < 30; i++) {
    await sleep(6000);
    try {
      const statusRes = await fetch(`${SKIP_API}/v2/tx/status?chain_id=${SRC_CHAIN}&tx_hash=${result.transactionHash}`);
      const status = await statusRes.json();
      const state = status.state || status.status;
      console.log(`  [${String(i+1).padStart(2)}] ${state}`);
      if (TERMINAL.includes(state)) {
        if (state === "STATE_COMPLETED_SUCCESS") {
          console.log("\n🎉 Transfer completed successfully!");
          console.log(`  Osmosis receiver : ${osmoAcc.address}`);
          console.log(`  Explorer: https://testnet.mintscan.io/osmosis-testnet/address/${osmoAcc.address}`);
        } else {
          console.error("\n❌ Transfer failed:", JSON.stringify(status.error ?? status, null, 2));
        }
        return;
      }
    } catch (e) {
      console.warn(`  poll error: ${e.message}`);
    }
  }
  console.log("⏱️  Timed out polling — check manually:");
  console.log(`  https://testnet.mintscan.io/zigchain/tx/${result.transactionHash}`);
}

function sleep(ms) { return new Promise(r => setTimeout(r, ms)); }

main().catch(e => { console.error("Fatal:", e.message); process.exit(1); });
