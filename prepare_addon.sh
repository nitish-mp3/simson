#!/usr/bin/env bash
# prepare_addon.sh
#
# Copies the voip-engine Rust source into ha_voip_engine/ so it is accessible
# to the Docker build context when building the HA add-on image.
#
# Run once before any `docker build` or HA builder invocation:
#   bash prepare_addon.sh
#
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ADDON_DIR="${REPO_ROOT}/ha_voip_engine"
SRC_DIR="${REPO_ROOT}/voip-engine"
DEST_DIR="${ADDON_DIR}/voip-engine"

echo "Preparing ha_voip_engine build context..."

if [ ! -d "${SRC_DIR}" ]; then
    echo "ERROR: ${SRC_DIR} not found. Run from the repo root." >&2
    exit 1
fi

# Remove stale copy if present
rm -rf "${DEST_DIR}"

# Copy Rust source (exclude build artifacts)
cp -r "${SRC_DIR}" "${DEST_DIR}"
rm -rf "${DEST_DIR}/target"

echo "Copied voip-engine source  →  ${DEST_DIR}"
echo ""
echo "Build the add-on image:"
echo "  cd ha_voip_engine"
echo "  docker build --build-arg BUILD_FROM=ghcr.io/home-assistant/amd64-base-debian:bookworm -t ha-voip-engine ."
echo ""
echo "Or use the HA builder for multi-arch:"
echo "  docker run --rm --privileged \\"
echo "    -v /var/run/docker.sock:/var/run/docker.sock \\"
echo "    -v \"\$(pwd)\":/data \\"
echo "    ghcr.io/home-assistant/builder --all --target /data/ha_voip_engine"
