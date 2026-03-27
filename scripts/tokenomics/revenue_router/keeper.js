/**
 * Revenue Router Keeper
 *
 * Polls the RevenueRouter contract on Zigchain. When USDC balance exceeds
 * MIN_BRIDGE_AMOUNT, calls Skip API to get the IBC route + PFM memo, then
 * executes BridgeUsdc on the contract.
 *
 * Flow:
 *   1. Query contract USDC balance
 *   2. POST /v2/fungible/route  → get multi-hop route
 *   3. POST /v2/fungible/msgs   → get MsgTransfer memo + receiver
 *   4. Execute BridgeUsdc { memo, receiver, amount }
 *   5. Poll /v2/tx/status until terminal state
 */

import { SigningCosmWasmClient } from "@cosmjs/cosmwasm-stargate";
import { DirectSecp256k1HdWallet } from "@cosmjs/proto-signing";
import { GasPrice } from "@cosmjs/stargate";
import { readFileSync } from "fs";
import { fileURLToPath } from "url";
import { dirname, join } from "path";

// Load .env
const __dirname = dirname(fileURLToPath(import.meta.url));
const envPath = join(__dirname, ".env");
try {
  const envFile = readFileSync(envPath, "utf8");
  for (const line of envFile.split("\n")) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith("#")) continue;
    const idx = trimmed.indexOf("=");
    if (idx === -1) continue;
    const key = trimmed.slice(0, idx).trim();
    const val = trimmed.slice(idx + 1).trim().replace(/^["']|["']$/g, "");
    if (!process.env[key]) process.env[key] = val;
  }
} catch {
  console.error("❌ .env file not found. Copy .env.example to .env and fill in values.");
  process.exit(1);
}

// ── Config ────────────────────────────────────────────────────────────────────

const cfg = {
  rpcUrl:          required("RPC_URL"),
  chainId:         required("CHAIN_ID"),
  mnemonic:        required("MNEMONIC"),
  contractAddress: required("CONTRACT_ADDRESS"),
  skipApiUrl:      process.env.SKIP_API_URL || "https://go.skip.build/api/skip",
  skipApiKey:      process.env.SKIP_API_KEY || "",
  usdcDenom:       required("USDC_DENOM"),
  sourceChainId:   required("SOURCE_CHAIN_ID"),
  destChainId:     required("DEST_CHAIN_ID"),
  destUsdcDenom:   required("DEST_USDC_DENOM"),
  destReceiver:    required("DEST_RECEIVER"),
  minBridgeAmount: BigInt(process.env.MIN_BRIDGE_AMOUNT || "1000000"),
  pollIntervalMs:  parseInt(process.env.POLL_INTERVAL_SECS || "60") * 1000,
  gasPrices:       process.env.GAS_PRICES || "0.25uzig",
  gasAdjustment:   parseFloat(process.env.GAS_ADJUSTMENT || "1.3"),
};

function required(key) {
  const v = process.env[key];
  if (!v) { console.error(`❌ Missing required env var: ${key}`); process.exit(1); }
  return v;
}

// ── Skip API helpers ──────────────────────────────────────────────────────────

async function skipPost(path, body) {
  const headers = { "Content-Type": "application/json" };
  if (cfg.skipApiKey) headers["authorization"] = cfg.skipApiKey;

  const res = await fetch(`${cfg.skipApiUrl}${path}`, {
    method: "POST",
    headers,
    body: JSON.stringify(body),
  });
  const data = await res.json();
  if (!res.ok) throw new Error(`Skip API ${path} failed: ${data.message || data.error || JSON.stringify(data)}`);
  return data;
}

async function skipGet(path) {
  const headers = {};
  if (cfg.skipApiKey) headers["authorization"] = cfg.skipApiKey;
  const res = await fetch(`${cfg.skipApiUrl}${path}`, { headers });
  const data = await res.json();
  if (!res.ok) throw new Error(`Skip API GET ${path} failed: ${data.message || JSON.stringify(data)}`);
  return data;
}

/**
 * Get the Skip route for USDC: Zigchain → Ethereum.
 */
async function getRoute(amountIn) {
  console.log("  📡 Fetching route from Skip API...");
  return skipPost("/v2/fungible/route", {
    source_asset_denom: cfg.usdcDenom,
    source_asset_chain_id: cfg.sourceChainId,
    dest_asset_denom: cfg.destUsdcDenom,
    dest_asset_chain_id: cfg.destChainId,
    amount_in: amountIn.toString(),
    smart_relay: true,
    allow_multi_tx: true,
    allow_unsafe: true,
  });
}

/**
 * Get the IBC MsgTransfer details (memo + receiver) from Skip API.
 *
 * Skip's required_chain_addresses differs by route type:
 *
 *   Testnet  (Cosmos→Cosmos via PFM):
 *     ["zig-test-2", "grand-1", "osmo-test-5"]
 *     Noble IS in the list — needs a real noble1... address.
 *     Skip encodes the Noble→Osmosis hop in the PFM memo.
 *
 *   Mainnet (Cosmos→EVM via CCTP/Orbiter):
 *     ["zigchain-1", "1"]
 *     Noble is NOT in the list — Skip handles it internally via Orbiter.
 *     Only source (zig) and final EVM destination are needed.
 *
 * The contract and keeper code is identical for both — Skip abstracts the
 * difference entirely inside the memo it generates. We just pass the right
 * address for each chain ID in required_chain_addresses.
 */
async function getMsgs(route, contractAddress) {
  console.log("  📡 Fetching msgs from Skip API...");

  const addressList = route.required_chain_addresses.map((chainId) => {
    if (chainId === cfg.sourceChainId) return contractAddress;
    if (chainId === cfg.destChainId)   return cfg.destReceiver;
    // Intermediate Cosmos chain (e.g. Noble testnet in grand-1 route).
    // We must provide a valid address for this chain's bech32 prefix.
    // Read from INTERMEDIATE_RECEIVER_<chainId> env var if set,
    // otherwise fall back to the keeper's own address (re-derived below).
    const envKey = `INTERMEDIATE_RECEIVER_${chainId.replace(/-/g, "_").toUpperCase()}`;
    return process.env[envKey] || contractAddress;
  });

  console.log(`  address_list: ${JSON.stringify(addressList)}`);

  return skipPost("/v2/fungible/msgs", {
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
  });
}

/**
 * Poll /v2/tx/status until terminal state. Returns final status.
 */
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

// ── Contract helpers ──────────────────────────────────────────────────────────

/**
 * Query the contract's USDC balance.
 */
async function queryUsdcBalance(client) {
  const res = await client.queryContractSmart(cfg.contractAddress, { usdc_balance: {} });
  return BigInt(res.amount);
}

/**
 * Execute BridgeUsdc on the contract.
 */
async function executeBridgeUsdc(client, senderAddress, amount, memo, receiver) {
  const msg = {
    bridge_usdc: {
      amount: amount.toString(),
      memo,
      receiver,
    },
  };

  console.log(`  🚀 Executing BridgeUsdc: amount=${amount}, receiver=${receiver}`);

  const result = await client.execute(
    senderAddress,
    cfg.contractAddress,
    msg,
    "auto",
  );

  return result;
}

// ── Main loop ─────────────────────────────────────────────────────────────────

async function run() {
  console.log("🔑 Setting up wallet...");
  const wallet = await DirectSecp256k1HdWallet.fromMnemonic(cfg.mnemonic, {
    prefix: "zig",
  });
  const [account] = await wallet.getAccounts();
  console.log(`   Keeper address: ${account.address}`);

  const client = await SigningCosmWasmClient.connectWithSigner(cfg.rpcUrl, wallet, {
    gasPrice: GasPrice.fromString(cfg.gasPrices),
  });

  console.log(`✅ Connected to ${cfg.rpcUrl} (${cfg.chainId})`);
  console.log(`📋 Contract: ${cfg.contractAddress}`);
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
  console.log(`[${now}] Checking USDC balance...`);

  const balance = await queryUsdcBalance(client);
  console.log(`  Balance: ${balance} uusdc`);

  if (balance < cfg.minBridgeAmount) {
    console.log(`  Below minimum (${cfg.minBridgeAmount}), skipping.\n`);
    return;
  }

  console.log(`  Balance meets threshold, initiating bridge...`);

  // 1. Get route
  const route = await getRoute(balance);
  console.log(`  Route: ${route.source_asset_chain_id} → ${route.dest_asset_chain_id}, amount_out=${route.amount_out}`);

  // 2. Get msgs
  const msgsResponse = await getMsgs(route, cfg.contractAddress);
  const cosmosTx = msgsResponse.txs?.[0]?.cosmos_tx;
  if (!cosmosTx) throw new Error("No cosmos_tx in Skip msgs response");

  const ibcMsg = cosmosTx.msgs?.[0];
  if (!ibcMsg) throw new Error("No msg in cosmos_tx");

  // Skip returns msgs[0].msg as a JSON string of the MsgTransfer fields.
  // Both Cosmos→Cosmos (PFM) and Cosmos→EVM (CCTP/Orbiter) routes produce
  // the same structure — only the memo content differs, which Skip generates.
  // We extract receiver and memo and pass them straight to the contract.
  let memo, receiver;
  try {
    const msgValue = typeof ibcMsg.msg === "string" ? JSON.parse(ibcMsg.msg) : ibcMsg.msg;
    receiver = msgValue.receiver;
    memo     = msgValue.memo ?? "";
  } catch {
    throw new Error(`Could not parse Skip msg: ${JSON.stringify(ibcMsg)}`);
  }

  if (!receiver) throw new Error("Could not extract receiver from Skip msg");
  console.log(`  memo type: ${memo.startsWith('{"forward"') ? "PFM (Cosmos→Cosmos)" : memo.startsWith('{"orbiter"') ? "Orbiter/CCTP (Cosmos→EVM)" : "unknown"}`);

  // 3. Execute BridgeUsdc on the contract
  const result = await executeBridgeUsdc(client, keeperAddress, balance, memo, receiver);
  console.log(`  ✅ BridgeUsdc tx: ${result.transactionHash}`);

  // 4. Track the IBC transfer
  const finalStatus = await trackTransaction(cfg.sourceChainId, result.transactionHash);
  const state = finalStatus.state || finalStatus.status;

  if (state === "STATE_COMPLETED_SUCCESS") {
    console.log(`  ✅ Bridge completed successfully!\n`);
  } else {
    console.error(`  ❌ Bridge ended with state: ${state}`);
    if (finalStatus.error) console.error(`     Error: ${JSON.stringify(finalStatus.error)}`);
    console.log();
  }
}

function sleep(ms) {
  return new Promise((r) => setTimeout(r, ms));
}

// ── Entry point ───────────────────────────────────────────────────────────────

run().catch((e) => {
  console.error("Fatal:", e);
  process.exit(1);
});
