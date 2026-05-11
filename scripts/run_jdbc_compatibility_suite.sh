#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 3 ]]; then
    cat <<'USAGE' >&2
usage: scripts/run_jdbc_compatibility_suite.sh <jdbc-url> <user> <password> [suite-args...]

Example:
  scripts/run_jdbc_compatibility_suite.sh \
    'jdbc:postgresql://127.0.0.1:15432/exasol?preferQueryMode=extended' \
    sys \
    'EXASOL_PASSWORD' \
    --personas=baseline,metabase,dbeaver,dml,ddl,transaction,session,utility
USAGE
    exit 1
fi

if ! command -v java >/dev/null 2>&1; then
    echo "java is required to run the JDBC compatibility suite" >&2
    exit 1
fi

if ! command -v javac >/dev/null 2>&1; then
    echo "javac is required to compile the JDBC compatibility suite" >&2
    exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
BUILD_DIR="${TMPDIR:-/tmp}/exa-postgres-interface-jdbc"
JDBC_JAR="${PG_JDBC_JAR:-${BUILD_DIR}/postgresql-42.7.8.jar}"

mkdir -p "${BUILD_DIR}"

if [[ ! -f "${JDBC_JAR}" ]]; then
    curl -fsSL -o "${JDBC_JAR}" https://jdbc.postgresql.org/download/postgresql-42.7.8.jar
fi

javac \
    -cp "${JDBC_JAR}" \
    -d "${BUILD_DIR}" \
    "${REPO_ROOT}"/tests/jdbc/*.java

java \
    -cp "${JDBC_JAR}:${BUILD_DIR}" \
    PgJdbcCompatibilitySuite \
    "$@"
