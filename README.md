# Oroswap Core

Multi pool type automated market-maker (AMM) protocol powered by smart contracts on ZIGChain blockchain.

## Building Contracts

You will need Rust 1.68.0+ with wasm32-unknown-unknown target installed.

### You can compile each contract:

Go to contract directory and run

```
cargo wasm
cp ../../target/wasm32-unknown-unknown/release/oroswap_factory.wasm .
ls -l oroswap_factory.wasm
sha256sum oroswap_factory.wasm
```

### You can run tests for all contracts

Run the following from the repository root

```
cargo test
```

### For a production-ready (compressed) build:

Run the following from the repository root

```
./scripts/build_release.sh
```

The optimized contracts are generated in the artifacts/ directory.

## Docs

Docs can be generated using `cargo doc --no-deps`
