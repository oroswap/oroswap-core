# Changelog

All notable changes to this project will be documented in this file.

## [1.1.0] – 2025-09-24

### Added

- Initial comprehensive documentation for contracts, deployment, and transactions.
- GitHub Pages workflow for documentation.
- Pool initializer contract for atomic pool creation and liquidity provision
- Enhanced instantiation attributes for better traceability and monitoring
- Keeper authorization system to maker contract with comprehensive testing
- Keeper bridge management with critical token protection in maker contract

### Changed

- Convert token factory message fields from camelCase to snake_case for ZIGChain compatibility

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
