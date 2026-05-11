#!/usr/bin/env bash
# Verifies that the release artifact directory contains no files from the
# AGPL-isolated Metabase test corpus.
set -euo pipefail

ARTIFACT_DIR="${1:?usage: check_release_artifact_clean.sh <artifact-dir>}"

if grep -r "upstream-mined/metabase" "${ARTIFACT_DIR}" 2>/dev/null; then
    echo "ERROR: release artifact contains AGPL-isolated Metabase probe paths" >&2
    exit 1
fi

echo "OK: no AGPL-isolated Metabase probe paths found in ${ARTIFACT_DIR}"
