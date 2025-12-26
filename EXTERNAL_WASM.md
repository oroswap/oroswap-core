# External WASM Dependencies

This file tracks external wasm contracts used by Oroswap that are not built from this repository.

---

## CW721 Base (NFT Contract)

| Property | Value |
|----------|-------|
| **File** | `cw721_base.wasm` |
| **Source** | [public-awesome/cw-nfts](https://github.com/public-awesome/cw-nfts) |
| **Version** | v0.20.0 |
| **Download** | https://github.com/public-awesome/cw-nfts/releases/tag/v0.20.0 |
| **SHA256** | `5eab0b1d002a97103c41c24ef3ebb2a4cbd1b6344748a3f0743d780d21b0e6ea` |

### Verification

```bash
shasum -a 256 artifacts/cw721_base.wasm
# Expected: 5eab0b1d002a97103c41c24ef3ebb2a4cbd1b6344748a3f0743d780d21b0e6ea
```

---

## CW20 Base (Token Contract)

| Property | Value |
|----------|-------|
| **File** | `cw20_base.wasm` |
| **Source** | [CosmWasm/cw-plus](https://github.com/CosmWasm/cw-plus) |
| **Version** | v2.0.0 |
| **Build Script** | `scripts/build_cw20.sh` |

### Build from Source

```bash
./scripts/build_cw20.sh
```

---

## Notes

- CW721: Downloaded directly from official release (checksum verified)
- CW20: Built from source using `cosmwasm/optimizer` Docker image
