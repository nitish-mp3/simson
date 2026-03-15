#!/usr/bin/env bash
# =============================================================================
# run-integration-test.sh
# Starts the full stack via Docker Compose, waits for health checks, curls
# health endpoints, runs the Python integration test, captures the exit code,
# tears down with docker compose down, and exits with the test code.
# =============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
COMPOSE_FILE="${PROJECT_ROOT}/ops/docker-compose.demo.yml"
RESULTS_DIR="${PROJECT_ROOT}/test-results"
MAX_WAIT=180  # seconds to wait for services to become healthy
TEST_EXIT=0

mkdir -p "${RESULTS_DIR}"

# ---------------------------------------------------------------------------
# Cleanup handler - docker compose down
# ---------------------------------------------------------------------------
cleanup() {
    echo "==> Collecting logs before teardown..."
    docker compose -f "${COMPOSE_FILE}" logs --no-color > "${RESULTS_DIR}/docker-compose.log" 2>&1 || true

    echo "==> Tearing down (docker compose down)..."
    docker compose -f "${COMPOSE_FILE}" --profile test down -v --remove-orphans 2>/dev/null || true
}
trap cleanup EXIT

# ---------------------------------------------------------------------------
# 1. Start compose
# ---------------------------------------------------------------------------
echo "==> Step 1/5: Starting services with docker compose..."
docker compose -f "${COMPOSE_FILE}" build --parallel 2>/dev/null || true
docker compose -f "${COMPOSE_FILE}" up -d

# ---------------------------------------------------------------------------
# 2. wait_for_healthy function with timeout
# ---------------------------------------------------------------------------
echo "==> Step 2/5: Waiting for services to become healthy (timeout: ${MAX_WAIT}s)..."

wait_for_healthy() {
    local service=$1
    local elapsed=0

    while [ $elapsed -lt $MAX_WAIT ]; do
        status=$(docker inspect --format='{{.State.Health.Status}}' "${service}" 2>/dev/null || echo "missing")
        case "${status}" in
            healthy)
                echo "    ${service}: healthy (${elapsed}s)"
                return 0
                ;;
            unhealthy)
                echo "    ${service}: unhealthy after ${elapsed}s"
                docker logs --tail 20 "${service}" 2>&1 | sed 's/^/      /'
                return 1
                ;;
            *)
                sleep 3
                elapsed=$((elapsed + 3))
                ;;
        esac
    done

    echo "    ${service}: timed out after ${MAX_WAIT}s (last status: ${status})"
    return 1
}

wait_for_healthy "demo-voip-engine"
wait_for_healthy "demo-homeassistant"

echo "    All services healthy."

# ---------------------------------------------------------------------------
# 3. Curl health endpoints
# ---------------------------------------------------------------------------
echo "==> Step 3/5: Verifying health endpoints..."

echo "    Checking voip-engine health..."
curl -sf http://localhost:9090/health/live && echo "    -> voip-engine /health/live OK" || echo "    -> voip-engine /health/live FAILED"
curl -sf http://localhost:9090/health/ready && echo "    -> voip-engine /health/ready OK" || echo "    -> voip-engine /health/ready FAILED"

echo "    Checking Home Assistant health..."
curl -sf http://localhost:8123/api/ && echo "    -> HA /api/ OK" || echo "    -> HA /api/ FAILED"

# ---------------------------------------------------------------------------
# 4. Run Python integration test
# ---------------------------------------------------------------------------
echo "==> Step 4/5: Running Python integration tests..."

if [ -d "${PROJECT_ROOT}/tests/integration" ]; then
    python -m pytest "${PROJECT_ROOT}/tests/integration/" \
        --junitxml="${RESULTS_DIR}/integration-results.xml" \
        -v 2>&1 | tee "${RESULTS_DIR}/test-runner.log" || true
    TEST_EXIT=${PIPESTATUS[0]}
elif [ -d "${PROJECT_ROOT}/tests/python" ]; then
    python -m pytest "${PROJECT_ROOT}/tests/python/" \
        --junitxml="${RESULTS_DIR}/integration-results.xml" \
        -v 2>&1 | tee "${RESULTS_DIR}/test-runner.log" || true
    TEST_EXIT=${PIPESTATUS[0]}
else
    echo "    No integration test directory found, running compose test profile..."
    docker compose -f "${COMPOSE_FILE}" --profile test run \
        --rm \
        -e CI=true \
        -e BASE_URL=http://homeassistant:8123 \
        -e ENGINE_URL=http://demo-voip-engine:9090 \
        test-runner 2>&1 | tee "${RESULTS_DIR}/test-runner.log" || true
    TEST_EXIT=${PIPESTATUS[0]}
fi

# ---------------------------------------------------------------------------
# 5. Capture exit code and report
# ---------------------------------------------------------------------------
echo "==> Step 5/5: Test result summary"

cat > "${RESULTS_DIR}/summary.json" << EOF
{
  "timestamp": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "exit_code": ${TEST_EXIT},
  "passed": $([ ${TEST_EXIT} -eq 0 ] && echo true || echo false),
  "compose_file": "${COMPOSE_FILE}",
  "git_sha": "$(git -C "${PROJECT_ROOT}" rev-parse HEAD 2>/dev/null || echo 'unknown')"
}
EOF

if [ ${TEST_EXIT} -eq 0 ]; then
    echo "    PASSED: Integration tests succeeded."
else
    echo "    FAILED: Integration tests failed (exit code ${TEST_EXIT})."
    echo "    See ${RESULTS_DIR}/test-runner.log for details."
fi

# docker compose down happens in the cleanup trap
exit ${TEST_EXIT}
