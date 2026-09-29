/**
 * precompute.js
 *
 * One-shot helper. The contract now stores `first_hop_receiver` and
 * `forward_memo` as static destination policy in `Config` (see audit
 * fixes — keeper can no longer redirect funds). This script computes
 * both values once, using the same mnemonic as `test_transfer.js`:
 *
 *   1. Derive zig / noble / osmo addresses from the same MNEMONIC.
 *   2. Call Skip API for the route + msgs (Cosmos→Cosmos via PFM).
 *   3. Print env-var lines for FIRST_HOP_RECEIVER + FORWARD_MEMO that
 *      can be pasted into .env.testnet (or appended via `>> .env.testnet`).
 *
 * Run from this directory:
 *
 *   node precompute.js                       # uses .env (or .env.testnet)
 *   node precompute.js >> .env.testnet       # append directly
 *
 * Inputs come from env vars (same names as the keeper):
 *   MNEMONIC, USDC_DENOM, SOURCE_CHAIN_ID, DEST_CHAIN_ID, DEST_USDC_DENOM
 *
 * For the Osmosis testnet route this means:
 *   SOURCE_CHAIN_ID=zig-test-2
 *   DEST_CHAIN_ID=osmo-test-5
 *   DEST_USDC_DENOM=<USDC IBC denom as it lands on osmo-test-5>
 */

import { DirectSecp256k1HdWallet } from "@cosmjs/proto-signing";
import { readFileSync } from "fs";
import { fileURLToPath } from "url";
import { dirname, join } from "path";

// ── Load .env (preferring .env.testnet if .env doesn't exist) ────────────────
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

function required(name) {
  const v = process.env[name];
  if (!v) {
    console.error(`❌ Missing env var: ${name}`);
    process.exit(1);
  }
  return v;
}

const MNEMONIC        = required("MNEMONIC");
const USDC_DENOM      = required("USDC_DENOM");
const SOURCE_CHAIN_ID = required("SOURCE_CHAIN_ID");
const DEST_CHAIN_ID   = required("DEST_CHAIN_ID");
const DEST_USDC_DENOM = required("DEST_USDC_DENOM");
const SKIP_API_URL    = process.env.SKIP_API_URL || "https://go.skip.build/api/skip";
const SKIP_API_KEY    = process.env.SKIP_API_KEY || "";

// 1 USDC is enough to derive a route — Skip doesn't care about the
// exact amount for the *shape* of the memo. The actual bridge amount at
// runtime is decided by the keeper and capped by Config.max_bridge_per_tx.
const ROUTE_PROBE_AMOUNT = "1000000";

async function deriveAddress(prefix) {
  const wallet = await DirectSecp256k1HdWallet.fromMnemonic(MNEMONIC, { prefix });
  const [acc] = await wallet.getAccounts();
  return acc.address;
}

async function skipPost(path, body) {
  const headers = { "Content-Type": "application/json" };
  if (SKIP_API_KEY) headers["authorization"] = SKIP_API_KEY;
  const r = await fetch(`${SKIP_API_URL}${path}`, {
    method: "POST",
    headers,
    body: JSON.stringify(body),
  });
  const data = await r.json();
  if (!r.ok) {
    throw new Error(`Skip ${path} failed: ${data.message || JSON.stringify(data)}`);
  }
  return data;
}

(async () => {
  // ── 1. Addresses ──────────────────────────────────────────────────────────
  const zig   = await deriveAddress("zig");
  const noble = await deriveAddress("noble");
  const osmo  = await deriveAddress("osmo");

  console.error(`# derived from MNEMONIC:`);
  console.error(`#   zig:   ${zig}`);
  console.error(`#   noble: ${noble}`);
  console.error(`#   osmo:  ${osmo}`);

  // ── 2. Skip /v2/fungible/route ────────────────────────────────────────────
  console.error(`# fetching Skip route...`);
  const route = await skipPost("/v2/fungible/route", {
    source_asset_denom:    USDC_DENOM,
    source_asset_chain_id: SOURCE_CHAIN_ID,
    dest_asset_denom:      DEST_USDC_DENOM,
    dest_asset_chain_id:   DEST_CHAIN_ID,
    amount_in:             ROUTE_PROBE_AMOUNT,
    smart_relay:           true,
    allow_multi_tx:        true,
    allow_unsafe:          true,
  });
  console.error(`# route chain_ids: ${(route.chain_ids || []).join(" → ")}`);

  // ── 3. Skip /v2/fungible/msgs ─────────────────────────────────────────────
  const addressMap = {
    [SOURCE_CHAIN_ID]: zig,
    [DEST_CHAIN_ID]:   osmo,
    "grand-1":         noble,   // Noble testnet
    "noble-1":         noble,   // Noble mainnet (just in case)
  };
  const addressList = (route.required_chain_addresses || []).map((id) => {
    const a = addressMap[id];
    if (!a) {
      throw new Error(`No address for required chain ${id} — add a prefix to deriveAddress() or override via env`);
    }
    return a;
  });
  console.error(`# address_list: ${JSON.stringify(addressList)}`);

  const msgs = await skipPost("/v2/fungible/msgs", {
    source_asset_denom:    route.source_asset_denom,
    source_asset_chain_id: route.source_asset_chain_id,
    dest_asset_denom:      route.dest_asset_denom,
    dest_asset_chain_id:   route.dest_asset_chain_id,
    amount_in:             route.amount_in,
    amount_out:            route.amount_out,
    address_list:          addressList,
    operations:            route.operations,
    slippage_tolerance_percent: "1",
    chain_ids_to_affiliates: {},
  });

  const cosmosTx = msgs.txs?.[0]?.cosmos_tx;
  if (!cosmosTx) throw new Error("No cosmos_tx in Skip msgs response");
  const ibcMsg = cosmosTx.msgs?.[0];
  if (!ibcMsg) throw new Error("No msg in cosmos_tx");
  const msgValue =
    typeof ibcMsg.msg === "string" ? JSON.parse(ibcMsg.msg) : ibcMsg.msg;

  const firstHopReceiver = msgValue.receiver;
  const sourceChannel    = msgValue.source_channel;
  let forwardMemo        = msgValue.memo ?? "";

  // Normalize the PFM memo so it survives long-term static use:
  // Skip generates `forward.timeout` as a fixed unix-nano timestamp that's
  // only valid for a few minutes from "now". For a static config that lives
  // on chain for weeks/months this would silently break. Replace it with
  // a duration string ("10m") which PFM re-evaluates relative to each hop.
  try {
    const memoObj = JSON.parse(forwardMemo);
    if (memoObj?.forward && typeof memoObj.forward.timeout === "number") {
      const orig = memoObj.forward.timeout;
      memoObj.forward.timeout = "10m";
      console.error(`# normalized forward.timeout: ${orig} → "10m"`);
    }
    forwardMemo = JSON.stringify(memoObj);
  } catch {
    console.error(`# warning: could not parse memo as JSON, using verbatim`);
  }

  if (!firstHopReceiver) throw new Error("Skip returned no receiver");
  if (!forwardMemo)      throw new Error("Skip returned no memo (Cosmos→Cosmos route should have one)");

  // ── 4. Emit env-style output ──────────────────────────────────────────────
  // Single-quoted so the JSON-with-double-quotes inside FORWARD_MEMO stays
  // intact when the shell sources this. Comment lines start with `#`.
  console.error(`# source_channel: ${sourceChannel}  (this should match IBC_CHANNEL in .env)`);
  console.error(`# OK — paste/append below:`);
  console.log("");
  console.log(`# precomputed by scripts/tokenomics/revenue_router/precompute.js`);
  console.log(`FIRST_HOP_RECEIVER=${firstHopReceiver}`);
  console.log(`FORWARD_MEMO='${forwardMemo}'`);
  console.log(`# Sanity: derived addresses`);
  console.log(`#   ZIG_ADDR=${zig}`);
  console.log(`#   NOBLE_ADDR=${noble}`);
  console.log(`#   OSMO_ADDR=${osmo}`);
})().catch((e) => {
  console.error("Fatal:", e.message);
  process.exit(1);
});
