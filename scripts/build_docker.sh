#!/usr/bin/env bash
set -euo pipefail

# ── 0) Locate script & project root ───────────────────────────────────────
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
project_root="$(cd "$script_dir/.." && pwd)"
OUTPUT_DIR="$project_root/opt-artifacts"

# Check if we're in the right directory
if [[ ! -f "$project_root/Dockerfile.build" ]]; then
  echo "❌ Error: Dockerfile.build not found in project root" >&2
  echo "   Please run this script from the project root directory:" >&2
  echo "   cd $project_root" >&2
  echo "   ./scripts/build_docker.sh" >&2
  exit 1
fi

mkdir -p "$OUTPUT_DIR"

echo "🐳 Building with Docker for reproducible builds..."
echo "   Project root: $project_root"
echo "   Output dir: $OUTPUT_DIR"

# Build the Docker image
docker build -f "$project_root/Dockerfile.build" -t oroswap-builder "$project_root"

# Run the build in Docker
docker run --rm \
  -v "$project_root:/code" \
  -v "$OUTPUT_DIR:/code/opt-artifacts" \
  oroswap-builder

echo "✅ Docker build completed! Artifacts in $OUTPUT_DIR" 