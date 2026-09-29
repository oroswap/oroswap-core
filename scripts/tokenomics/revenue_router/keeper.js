/**
 * Revenue Router Keeper (post-audit)
 *
 * The contract now holds the full destination policy in Config
 * (`first_hop_receiver` + `forward_memo`). A hijacked keeper key can
 * only trigger transfers to that configured destination — it cannot
 * pass a memo or receiver. So this keeper has one job:
 *
 *   1. Query the contract's USDC balance.
 *   2. If balance >= MIN_BRIDGE_AMOUNT (and contract isn't paused),
 *      execute BridgeUsdc { amount } and let Config supply the route.
 *   3. Optionally track the IBC packet via Skip's tx-status endpoint
 *      (off-chain observability only — doesn't affect contract logic).
 *
 * Compare with the pre-audit keeper, which called Skip at every tick
 * and supplied memo+receiver per call.
 */

import { SigningCosmWasmClient } from "@cosmjs/cosmwasm-stargate";
import { DirectSecp256k1HdWallet } from "@cosmjs/proto-signing";
import { GasPrice } from "@cosmjs/stargate";
import { readFileSync } from "fs";
import { fileURLToPath } from "url";
import { dirname, join } from "path";

// ── Load .env ────────────────────────────────────────────────────────────────
const __dirname = dirname(fileURLToPath(import.meta.url));
for (const name of [".env", ".env.testnet"]) {
  try {
    const file = readFileSync(join(__dirname, name), "utf8");
    for (const line of file.split("\n")) {
      const t = line.trim();
      if (!t || t.startsWith("#")) continue;
      const i = t.indexOf("=");
      if (i === -1) continue;
      const k = t.slice(0, i).trim();
      const v = t.slice(i + 1).trim().replace(/^["']|["']$/g, "");
      if (!process.env[k]) process.env[k] = v;
    }
    break;
  } catch {
    // try next candidate
  }
}

function required(k) {
  const v = process.env[k];
  if (!v) { console.error(`❌ Missing required env var: ${k}`); process.exit(1); }
  return v;
}

const cfg = {
  rpcUrl:          required("RPC_URL"),
  chainId:         required("CHAIN_ID"),
  mnemonic:        required("MNEMONIC"),
  contractAddress: required("CONTRACT_ADDRESS"),
  sourceChainId:   required("SOURCE_CHAIN_ID"),
  minBridgeAmount: BigInt(process.env.MIN_BRIDGE_AMOUNT || "1000000"),
  pollIntervalMs:  parseInt(process.env.POLL_INTERVAL_SECS || "60") * 1000,
  gasPrices:       process.env.GAS_PRICES || "0.25uzig",
  // Skip is only used for off-chain tx-status polling now. Optional.
  skipApiUrl:      process.env.SKIP_API_URL || "https://go.skip.build/api/skip",
  skipApiKey:      process.env.SKIP_API_KEY || "",
  trackWithSkip:   (process.env.TRACK_WITH_SKIP || "true").toLowerCase() === "true",
};

// ── Contract helpers ─────────────────────────────────────────────────────────

async function queryUsdcBalance(client) {
  const res = await client.queryContractSmart(cfg.contractAddress, { usdc_balance: {} });
  return BigInt(res.amount);
}

async function queryConfig(client) {
  return client.queryContractSmart(cfg.contractAddress, { config: {} });
}

async function queryPreview(client) {
  return client.queryContractSmart(cfg.contractAddress, { preview_bridge: {} });
}

/**
 * Execute BridgeUsdc on the contract. The destination is owner-controlled
 * policy in Config — this caller can only choose how much.
 */
async function executeBridgeUsdc(client, senderAddress, amount) {
  const msg = { bridge_usdc: { amount: amount.toString() } };
  console.log(`  🚀 Executing BridgeUsdc: amount=${amount}`);
  return client.execute(senderAddress, cfg.contractAddress, msg, "auto");
}

// ── Optional Skip tx-status polling (observability only) ─────────────────────

async function skipGet(path) {
  const headers = {};
  if (cfg.skipApiKey) headers["authorization"] = cfg.skipApiKey;
  const r = await fetch(`${cfg.skipApiUrl}${path}`, { headers });
  const data = await r.json();
  if (!r.ok) throw new Error(`Skip GET ${path} failed: ${data.message || JSON.stringify(data)}`);
  return data;
}

async function trackTransaction(chainId, txHash) {
  console.log(`  🔍 Tracking tx ${txHash} on chain ${chainId}...`);
  const TERMINAL = ["STATE_COMPLETED_SUCCESS", "STATE_COMPLETED_ERROR", "STATE_ABANDONED"];
  for (let attempt = 0; attempt < 60; attempt++) {
    await sleep(5000);
    try {
      const status = await skipGet(`/v2/tx/status?chain_id=${chainId}&tx_hash=${txHash}`);
      const state = status.state || status.status;
      console.log(`  ⏳ Status: ${state}`);
      if (TERMINAL.includes(state)) return status;
    } catch (e) {
      console.warn(`  ⚠️  Status poll error: ${e.message}`);
    }
  }
  throw new Error("Timed out waiting for transaction to reach terminal state");
}

// ── Main loop ────────────────────────────────────────────────────────────────

async function run() {
  console.log("🔑 Setting up wallet...");
  const wallet = await DirectSecp256k1HdWallet.fromMnemonic(cfg.mnemonic, { prefix: "zig" });
  const [account] = await wallet.getAccounts();
  console.log(`   Keeper address: ${account.address}`);

  const client = await SigningCosmWasmClient.connectWithSigner(cfg.rpcUrl, wallet, {
    gasPrice: GasPrice.fromString(cfg.gasPrices),
  });

  console.log(`✅ Connected to ${cfg.rpcUrl} (${cfg.chainId})`);
  console.log(`📋 Contract: ${cfg.contractAddress}`);

  // One-time eyeball of the destination policy the contract will use.
  try {
    const preview = await queryPreview(client);
    console.log(`📦 Destination policy (Config):`);
    console.log(`   channel        : ${preview.channel}`);
    console.log(`   first_hop      : ${preview.receiver}`);
    console.log(`   memo           : ${String(preview.memo).slice(0, 120)}...`);
    console.log(`   max_per_tx     : ${preview.max_bridge_per_tx ?? "<uncapped>"}`);
  } catch (e) {
    console.warn(`⚠️  Could not query PreviewBridge: ${e.message}`);
  }

  console.log(`💰 Min bridge amount: ${cfg.minBridgeAmount} (uusdc)`);
  console.log(`⏱️  Poll interval: ${cfg.pollIntervalMs / 1000}s\n`);

  while (true) {
    try {
      await tick(client, account.address);
    } catch (e) {
      console.error(`❌ Tick error: ${e.message}`);
    }
    await sleep(cfg.pollIntervalMs);
  }
}

async function tick(client, keeperAddress) {
  const now = new Date().toISOString();
  console.log(`[${now}] Checking USDC balance + pause state...`);

  // Skip the bridge entirely if paused — saves gas + avoids a guaranteed
  // ContractError::Paused. The contract would reject anyway, this is a UX nicety.
  const conf = await queryConfig(client);
  if (conf.paused) {
    console.log(`  Contract paused, skipping.\n`);
    return;
  }

  const balance = await queryUsdcBalance(client);
  console.log(`  Balance: ${balance} uusdc`);

  if (balance < cfg.minBridgeAmount) {
    console.log(`  Below minimum (${cfg.minBridgeAmount}), skipping.\n`);
    return;
  }

  // Respect the on-chain cap so we don't trip BridgeCapExceeded.
  let bridgeAmount = balance;
  if (conf.max_bridge_per_tx) {
    const cap = BigInt(conf.max_bridge_per_tx);
    if (bridgeAmount > cap) {
      console.log(`  Capping bridge amount: ${bridgeAmount} → ${cap} (Config.max_bridge_per_tx)`);
      bridgeAmount = cap;
    }
  }

  const result = await executeBridgeUsdc(client, keeperAddress, bridgeAmount);
  console.log(`  ✅ BridgeUsdc tx: ${result.transactionHash}`);

  if (!cfg.trackWithSkip) {
    console.log();
    return;
  }
  try {
    const finalStatus = await trackTransaction(cfg.sourceChainId, result.transactionHash);
    const state = finalStatus.state || finalStatus.status;
    if (state === "STATE_COMPLETED_SUCCESS") {
      console.log(`  ✅ Bridge completed successfully!\n`);
    } else {
      console.error(`  ❌ Bridge ended with state: ${state}`);
      if (finalStatus.error) console.error(`     Error: ${JSON.stringify(finalStatus.error)}`);
      console.log();
    }
  } catch (e) {
    console.warn(`  ⚠️  Tracking failed: ${e.message}\n`);
  }
}

function sleep(ms) { return new Promise(r => setTimeout(r, ms)); }

run().catch((e) => {
  console.error("Fatal:", e);
  process.exit(1);
});
