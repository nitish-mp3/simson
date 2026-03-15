#!/usr/bin/with-contenv bash
# shellcheck shell=bash
set -euo pipefail

DATA_DIR="/data/voip"
LOG_LEVEL="info"
LOG_FORMAT="json"

# Create required data directories
mkdir -p "${DATA_DIR}" "${DATA_DIR}/recordings" "${DATA_DIR}/certs"

# Parse options.json written by HA supervisor (requires jq, installed in Dockerfile)
OPTIONS_FILE="/data/options.json"
if [ -f "${OPTIONS_FILE}" ] && command -v jq >/dev/null 2>&1; then
    _val=$(jq -r '.log_level // empty' "${OPTIONS_FILE}" 2>/dev/null || true)
    [ -n "${_val}" ] && LOG_LEVEL="${_val}"

    _val=$(jq -r '.log_format // empty' "${OPTIONS_FILE}" 2>/dev/null || true)
    [ -n "${_val}" ] && LOG_FORMAT="${_val}"
fi

echo "[ha-voip] Starting voip-engine (log-level=${LOG_LEVEL}, log-format=${LOG_FORMAT})"

exec /usr/local/bin/voip-engine \
    --data-dir "${DATA_DIR}" \
    --log-level "${LOG_LEVEL}" \
    --log-format "${LOG_FORMAT}"
