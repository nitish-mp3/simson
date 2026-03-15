#!/usr/bin/env bash
# =============================================================================
# build-engine.sh
# Compiles the Rust voip-engine binary, strips debug symbols, creates a
# tarball with the binary and required configs, generates sha256 checksum.
# =============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
ENGINE_DIR="${PROJECT_ROOT}/voip-engine"
DIST_DIR="${PROJECT_ROOT}/dist"
VERSION="${TAG_NAME:-$(git -C "${PROJECT_ROOT}" describe --tags --always 2>/dev/null || echo 'dev')}"
TARGET="${BUILD_TARGET:-x86_64-unknown-linux-gnu}"
BINARY_NAME="voip-engine"

echo "==> Building voip-engine ${VERSION} for ${TARGET}"

# ---------------------------------------------------------------------------
# 1. Compile
# ---------------------------------------------------------------------------
echo "==> Step 1/4: Compiling Rust binary..."
cd "${ENGINE_DIR}"

if [ "${TARGET}" != "$(rustc -vV | sed -n 's/host: //p')" ]; then
    rustup target add "${TARGET}"
fi

cargo build --release --target "${TARGET}" 2>/dev/null || cargo build --release

BINARY_PATH="${ENGINE_DIR}/target/${TARGET}/release/${BINARY_NAME}"
if [ ! -f "${BINARY_PATH}" ]; then
    BINARY_PATH="${ENGINE_DIR}/target/release/${BINARY_NAME}"
fi

if [ ! -f "${BINARY_PATH}" ]; then
    echo "ERROR: Binary not found after build"
    exit 1
fi

echo "    Binary size (before strip): $(du -h "${BINARY_PATH}" | cut -f1)"

# ---------------------------------------------------------------------------
# 2. Strip debug symbols
# ---------------------------------------------------------------------------
echo "==> Step 2/4: Stripping debug symbols..."
strip "${BINARY_PATH}"
echo "    Binary size (after strip): $(du -h "${BINARY_PATH}" | cut -f1)"

# ---------------------------------------------------------------------------
# 3. Create tarball with binary and required configs
# ---------------------------------------------------------------------------
echo "==> Step 3/4: Packaging tarball..."
mkdir -p "${DIST_DIR}"

STAGING_DIR=$(mktemp -d)
ARCHIVE_NAME="${BINARY_NAME}-${VERSION}-${TARGET}.tar.gz"

# Copy binary
cp "${BINARY_PATH}" "${STAGING_DIR}/${BINARY_NAME}"

# Copy required config files if they exist
if [ -d "${PROJECT_ROOT}/ops/k8s" ]; then
    mkdir -p "${STAGING_DIR}/config"
    cp "${PROJECT_ROOT}/ops/k8s/values.yaml" "${STAGING_DIR}/config/" 2>/dev/null || true
fi

if [ -d "${PROJECT_ROOT}/monitoring" ]; then
    mkdir -p "${STAGING_DIR}/monitoring"
    cp -r "${PROJECT_ROOT}/monitoring/prometheus" "${STAGING_DIR}/monitoring/" 2>/dev/null || true
fi

if [ -d "${PROJECT_ROOT}/migrations" ]; then
    mkdir -p "${STAGING_DIR}/migrations"
    cp "${PROJECT_ROOT}/migrations/"*.sql "${STAGING_DIR}/migrations/" 2>/dev/null || true
fi

# Create a wrapper script
cat > "${STAGING_DIR}/run.sh" << 'WRAPPER'
#!/usr/bin/env bash
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
exec "${SCRIPT_DIR}/voip-engine" "$@"
WRAPPER
chmod +x "${STAGING_DIR}/run.sh"

tar -czf "${DIST_DIR}/${ARCHIVE_NAME}" -C "${STAGING_DIR}" .
echo "    Archive: ${DIST_DIR}/${ARCHIVE_NAME}"
echo "    Archive size: $(du -h "${DIST_DIR}/${ARCHIVE_NAME}" | cut -f1)"

# ---------------------------------------------------------------------------
# 4. Generate sha256 checksum
# ---------------------------------------------------------------------------
echo "==> Step 4/4: Generating sha256 checksum..."
cd "${DIST_DIR}"
sha256sum "${ARCHIVE_NAME}" > "${ARCHIVE_NAME}.sha256"
echo "    SHA-256: $(cat "${ARCHIVE_NAME}.sha256")"

# Clean up staging directory
rm -rf "${STAGING_DIR}"

echo "==> Build complete: ${DIST_DIR}/${ARCHIVE_NAME}"
