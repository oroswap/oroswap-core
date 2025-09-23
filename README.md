# Oroswap Core

Multi pool type automated market-maker (AMM) protocol with multiple fee tiers, powered by smart contracts on ZIGChain blockchain.

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

## Contracts


| Contract                                                          | Description                                                       |
| ----------------------------------------------------------------- | ----------------------------------------------------------------- |
| [Factory](contracts/factory/)                                     | Core factory contract that creates and manages all trading pairs  |
| [Pair](contracts/pair/)                                           | Standard XYK (constant product) automated market maker pair       |
| [Pair Concentrated](contracts/pair_concentrated/)                 | Concentrated liquidity pair with adaptive price ranges            |
| [Pair Stable](contracts/pair_stable/)                             | Stable swap pair optimized for assets with similar values         |
| [Router](contracts/router/)                                       | Aggregation router for optimal swap routing across multiple pairs |
| [Maker](contracts/tokenomics/maker/)                              | Fee collection and distribution contract for protocol revenue     |
| [Incentives](contracts/tokenomics/incentives/)                    | Liquidity mining and reward distribution system                   |
| [Native Coin Registry](contracts/periphery/native_coin_registry/) | Registry for native token metadata and configurations             |
| [Pool Initializer](contracts/periphery/pool_initializer/)         | Helper contract for initializing new trading pools                |

## Deployment

Deployed contract addresses and deployment information can be found in the [Oroswap Deployment Repository](https://github.com/oroswap/oroswap-deployments).

## Docs

Docs can be generated using `cargo doc --no-deps`
