#!/usr/bin/env bash

set -e
set -o pipefail

projectPath=$(cd "$(dirname "${0}")" && cd ../ && pwd)

echo "Starting interactive shell in nightly-optimizer container to inspect optimize.sh..."

docker run --rm -it \
  --entrypoint sh \
  -v "$projectPath":/code \
  --mount type=volume,source="$(basename "$projectPath")_cache",target=/target \
  --mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
  nightly-optimizer