# Changelog

All notable changes to this project will be documented in this file.

## [2.0.0] – 2026-09-30

### Added

- Airdrop contract with claim, linear vesting with APY bonus and mandatory expiry
- Wrapper contract for 1:1 lock-and-mint of factory tokens
- Revenue router contract for bridging USDC from ZIG Chain to Ethereum via Noble CCTP
- ZIG Chain v5 denom (`uzig` → `azig`) migration for factory and XYK pairs
- `UpdatePoolCreationFeeDenom` message in factory
- Testing scripts for airdrop, staking, wrapper and revenue router

### Changed

- Staking contract updates: xORO removed, 1:1 stake registry with multi-denom rewards and an unbonding queue
- Pool creation fee denom now defaults to `azig` and pairs read it from the factory instead of default `uzig`
- XYK pair `MigrateMsg` Added `denom_from` and `denom_to`
- XYK TWAP accumulator widened to `Uint256` with 18-decimal precision (reset on migrate)
- LP token minting cap raised from `u64::MAX` to `Uint128::MAX`
- Factory, pair, pair-stable and pair-concentrated bumped to 1.2.0; staking to 2.2.0
- Removed unused `[profile.release]` block from `.cargo/config.toml`

### Fixed

- **Pairs**:
  - Fixed XYK TWAP not updating on pools with 18-decimal and 6-decimal assets
  - Fixed stable pool LP overflow with 18-decimal assets
  - Fixed stable and concentrated pool creation failing after the `azig` redenomination
- **Staking**:
  - Reward denom limit now counts only active streams
  - Pending rewards keep rounding remainders instead of dropping them
  - Added unbonding period bounds check on instantiate
- **Airdrop, Wrapper, Staking**:
  - Added forward-only migrations

## [1.1.0] – 2025-09-24

### Added

- CW721 NFT support scripts (`mainnet-scripts/cw-721-nft/`)
  - NFT minting, batch minting, transfers, burns
- Added `cw721_base.wasm` v0.20.0 from [cw-nfts](https://github.com/public-awesome/cw-nfts) (external dependency)

## [1.1.0] – 2025-09-24

### Added

- Initial comprehensive documentation for contracts, deployment, and transactions.
- GitHub Pages workflow for documentation.
- Pool initializer contract for atomic pool creation and liquidity provision
- Enhanced instantiation attributes for better traceability and monitoring
- Keeper authorization system to maker contract with comprehensive testing
- Keeper bridge management with critical token protection in maker contract

### Changed

- Convert token factory message fields from camelCase to snake_case for ZIG Chain compatibility

### Fixed

- **Security Vulnerabilities**: Comprehensive audit fixes addressing multiple critical security issues
- **Pool Initializer**: 
  - Fixed LP token receiver issue - LP tokens now go to user instead of contract
  - Added proper admin authorization and owner field to config
  - Removed redundant logic and added defensive guards
  - Added migration support
- **Incentives Contract**:
  - Prevented fund seizure when bypass_upcoming_schedules=true
  - Updated to require incentivization fee for every schedule
- **Maker Contract**:
  - Added authorization check to seize function
  - Resolved dev fund distribution vulnerability and added comprehensive validation
  - Added permissioned collect to prevent griefing attacks
- **General Security**:
  - Added DoS protection to query handlers with unbounded limits
  - Addressed audit vulnerabilities including case sensitivity bypass and robust pair key generation

## [1.0.0] – 2025-07-10

### Added

- Initial testnet release of Oroswap DEX
- Core DEX functionality with standard pairs
- Concentrated liquidity pairs for advanced trading
- Factory contract for pair creation and management
- CW20 token support and integration
- Basic swap functionality with slippage protection
- Liquidity provision and removal mechanisms
- Fee collection and distribution system
- Router contract for token swaps
- Token factory integration for custom token creation
- Incentives and staking rewards system
