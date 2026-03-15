#!/usr/bin/env bash
# =============================================================================
# build-frontend.sh
# Installs Node dependencies, builds the frontend, and copies the resulting
# ha-voip-card.js into the integration custom_components directory.
# =============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
FRONTEND_DIR="${PROJECT_ROOT}/frontend"
OUTPUT_DIR="${FRONTEND_DIR}/dist"
INTEGRATION_STATIC_DIR="${PROJECT_ROOT}/custom_components/ha_voip/frontend"

echo "==> Building HA-VoIP frontend"

# ---------------------------------------------------------------------------
# 1. Change to frontend directory
# ---------------------------------------------------------------------------
echo "==> Step 1/4: Entering frontend directory..."
cd "${FRONTEND_DIR}"

if [ ! -f "package.json" ]; then
    echo "ERROR: package.json not found in ${FRONTEND_DIR}"
    exit 1
fi

# ---------------------------------------------------------------------------
# 2. Install dependencies
# ---------------------------------------------------------------------------
echo "==> Step 2/4: Installing dependencies (npm ci)..."
npm ci --no-audit --no-fund

# ---------------------------------------------------------------------------
# 3. Build
# ---------------------------------------------------------------------------
echo "==> Step 3/4: Building production bundle (npm run build)..."
NODE_ENV=production npm run build

if [ ! -d "${OUTPUT_DIR}" ]; then
    echo "ERROR: Build output directory not found at ${OUTPUT_DIR}"
    exit 1
fi

BUNDLE_SIZE=$(du -sh "${OUTPUT_DIR}" | cut -f1)
echo "    Output: ${OUTPUT_DIR} (${BUNDLE_SIZE})"

# ---------------------------------------------------------------------------
# 4. Copy ha-voip-card.js to integration custom_components dir
# ---------------------------------------------------------------------------
echo "==> Step 4/4: Copying ha-voip-card.js to custom_components..."
mkdir -p "${INTEGRATION_STATIC_DIR}"

if [ -f "${OUTPUT_DIR}/ha-voip-card.js" ]; then
    cp "${OUTPUT_DIR}/ha-voip-card.js" "${INTEGRATION_STATIC_DIR}/ha-voip-card.js"
    echo "    Copied: ha-voip-card.js -> ${INTEGRATION_STATIC_DIR}/ha-voip-card.js"
else
    # Fallback: copy all build output
    echo "    ha-voip-card.js not found; copying all build output..."
    cp -r "${OUTPUT_DIR}/." "${INTEGRATION_STATIC_DIR}/"
    echo "    Copied all output to: ${INTEGRATION_STATIC_DIR}"
fi

echo "==> Frontend build complete"
