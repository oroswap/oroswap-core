#!/usr/bin/env bash
set -euo pipefail

# ─── Configuration ──────────────────────────────────────────────────────────
BINARY="zigchaind"
RPC_URL="https://devnet-rpc.zigchain.com"
CHAIN_ID="zig-devnet-1"
KEY_NAME="devnet-key"
KEYRING_BACKEND="test"

# The denom you're targeting (uses the `coin.` prefix):
DENOM="coin.zig18sytwc03z5j3wge5egf4rdue6gxkzzyf4658vq.ufin1"

# Off-chain metadata JSON URI (pin to IPFS beforehand)
METADATA_URI="https://blue-careful-carp-364.mypinata.cloud/ipfs/bafkreifhxwnh36zccf7nl7yb2tv57gcpcqjtrpeidhzwbeyxz23skvdnv4"

# ─── Compute SHA-256 hash of the URI string ─────────────────────────────────
URI_HASH=$(echo -n "$METADATA_URI" | shasum -a 256 | awk '{print $1}')
echo "🔑 Computed URI hash from METADATA_URI: $URI_HASH"

# ─── Build on-chain payload ─────────────────────────────────────────────────
PAYLOAD_FILE="payload.json"
cat > "$PAYLOAD_FILE" <<EOF
{
  "description": "Fin1 Token - devnet",
  "denom_units": [
    {
      "denom": "${DENOM}",
      "exponent": 0,
      "aliases": ["ufin1"]
    },
    {
      "denom": "fin1",
      "exponent": 4,
      "aliases": []
    }
  ],
  "base": "${DENOM}",
  "display": "fin1",
  "name": "fin1 Token",
  "symbol": "FIN1",
  "uri": "${METADATA_URI}",
  "uri_hash": "${URI_HASH}"
}
EOF

echo "📄 On-chain metadata payload ($PAYLOAD_FILE):"
cat "$PAYLOAD_FILE"

# ─── Submit the transaction ─────────────────────────────────────────────────
echo -e "\n🔧 Submitting set-denom-metadata for $DENOM..."
$BINARY tx factory set-denom-metadata "$PAYLOAD_FILE" \
  --from "$KEY_NAME" \
  --keyring-backend "$KEYRING_BACKEND" \
  --node "$RPC_URL" \
  --chain-id "$CHAIN_ID" \
  --fees 20000uzig --gas auto --gas-adjustment 1.3 \
  -y -o json | jq

echo "✅ Metadata updated for $DENOM"
