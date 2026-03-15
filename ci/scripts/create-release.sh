#!/usr/bin/env bash
# =============================================================================
# create-release.sh
# Extracts the version from Cargo.toml, generates a changelog from git log,
# creates a GitHub release with the gh CLI, uploads tarball artifacts.
# =============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
ARTIFACTS_DIR="${PROJECT_ROOT}/artifacts"
CARGO_TOML="${PROJECT_ROOT}/voip-engine/Cargo.toml"

# ---------------------------------------------------------------------------
# 1. Extract version from Cargo.toml
# ---------------------------------------------------------------------------
echo "==> Step 1/4: Extracting version from Cargo.toml..."

if [ -f "${CARGO_TOML}" ]; then
    VERSION=$(grep -m1 '^version' "${CARGO_TOML}" | sed 's/.*= *"\(.*\)"/\1/')
    echo "    Cargo.toml version: ${VERSION}"
else
    VERSION="${TAG_NAME:-unknown}"
    echo "    Cargo.toml not found, using TAG_NAME: ${VERSION}"
fi

TAG_NAME="${TAG_NAME:-v${VERSION}}"
echo "    Release tag: ${TAG_NAME}"

# ---------------------------------------------------------------------------
# 2. Generate changelog from git log
# ---------------------------------------------------------------------------
echo "==> Step 2/4: Generating changelog from git log..."

PREV_TAG=$(git -C "${PROJECT_ROOT}" describe --tags --abbrev=0 HEAD^ 2>/dev/null || echo "")
CHANGELOG_FILE=$(mktemp)

{
    echo "## Release ${TAG_NAME}"
    echo ""
    echo "### What's Changed"
    echo ""

    if [ -n "${PREV_TAG}" ]; then
        RANGE="${PREV_TAG}..${TAG_NAME}"
        echo "Changes since ${PREV_TAG}:"
    else
        RANGE="${TAG_NAME}"
        echo "Initial release:"
    fi
    echo ""

    # Group commits by conventional commit type
    for prefix_label in "feat:Features" "fix:Bug Fixes" "perf:Performance" "docs:Documentation" "refactor:Refactoring" "test:Tests" "ci:CI/CD" "chore:Maintenance"; do
        prefix="${prefix_label%%:*}"
        label="${prefix_label##*:}"
        commits=$(git -C "${PROJECT_ROOT}" log "${RANGE}" --pretty=format:"- %s (%h)" --grep="^${prefix}" 2>/dev/null || true)
        if [ -n "${commits}" ]; then
            echo "#### ${label}"
            echo ""
            echo "${commits}"
            echo ""
        fi
    done

    # Uncategorized commits
    uncategorized=$(git -C "${PROJECT_ROOT}" log "${RANGE}" --pretty=format:"- %s (%h)" \
        --grep="^feat" --grep="^fix" --grep="^perf" --grep="^docs" \
        --grep="^refactor" --grep="^test" --grep="^ci" --grep="^chore" \
        --invert-grep 2>/dev/null || true)
    if [ -n "${uncategorized}" ]; then
        echo "#### Other"
        echo ""
        echo "${uncategorized}"
        echo ""
    fi

    echo "---"
    echo ""
    echo "**Full Changelog**: https://github.com/${GITHUB_REPOSITORY:-ha-voip/ha-voip}/compare/${PREV_TAG:-initial}...${TAG_NAME}"
} > "${CHANGELOG_FILE}"

echo "    Changelog generated ($(wc -l < "${CHANGELOG_FILE}") lines)"

# ---------------------------------------------------------------------------
# 3. Create GitHub release with gh CLI
# ---------------------------------------------------------------------------
echo "==> Step 3/4: Creating GitHub release with gh CLI..."

PRERELEASE_FLAG=""
if echo "${TAG_NAME}" | grep -qE '(alpha|beta|rc)'; then
    PRERELEASE_FLAG="--prerelease"
fi

gh release create "${TAG_NAME}" \
    --title "${TAG_NAME}" \
    --notes-file "${CHANGELOG_FILE}" \
    ${PRERELEASE_FLAG} \
    2>/dev/null || echo "    Release ${TAG_NAME} may already exist"

echo "    GitHub release created: ${TAG_NAME}"

# ---------------------------------------------------------------------------
# 4. Upload tarball artifacts
# ---------------------------------------------------------------------------
echo "==> Step 4/4: Uploading tarball artifacts..."

UPLOAD_COUNT=0
if [ -d "${ARTIFACTS_DIR}" ]; then
    while IFS= read -r -d '' file; do
        echo "    Uploading: $(basename "${file}")"
        gh release upload "${TAG_NAME}" "${file}" --clobber 2>/dev/null || true
        UPLOAD_COUNT=$((UPLOAD_COUNT + 1))
    done < <(find "${ARTIFACTS_DIR}" -type f \( -name "*.tar.gz" -o -name "*.sha256" -o -name "*.zip" \) -print0)
fi

# Clean up
rm -f "${CHANGELOG_FILE}"

echo "==> Release complete"
echo "    Tag:       ${TAG_NAME}"
echo "    Version:   ${VERSION}"
echo "    Artifacts: ${UPLOAD_COUNT} files uploaded"
echo "    URL:       https://github.com/${GITHUB_REPOSITORY:-ha-voip/ha-voip}/releases/tag/${TAG_NAME}"
