#!/usr/bin/env bash

# Usage: ./scripts/coverage.sh <package_name>
# Example: ./scripts/coverage.sh oroswap-pair

cargo tarpaulin --target-dir target/tarpaulin_build --skip-clean --exclude-files *tests*.rs --exclude-files target*.rs \
  -p "$1" --out Html
