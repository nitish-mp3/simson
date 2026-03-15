#!/usr/bin/with-contenv sh
set -eu

DATA_DIR="/data/voip"
LOG_LEVEL="info"

mkdir -p "${DATA_DIR}"

if [ -f /data/options.json ]; then
    # Extract configured log level without requiring jq.
    LOG_LEVEL_FROM_OPTIONS=$(grep -o '"log_level"[[:space:]]*:[[:space:]]*"[^"]*"' /data/options.json | head -n1 | sed -E 's/.*"([^"]+)"/\1/' || true)
    if [ -n "${LOG_LEVEL_FROM_OPTIONS}" ]; then
        LOG_LEVEL="${LOG_LEVEL_FROM_OPTIONS}"
    fi
fi

exec /usr/local/bin/voip-engine --data-dir "${DATA_DIR}" --log-level "${LOG_LEVEL}"
