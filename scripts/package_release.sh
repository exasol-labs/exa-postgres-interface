#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION="${1:-$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -n1)}"
TARGET_TRIPLE="${TARGET_TRIPLE:-$(uname -m)-unknown-linux-gnu}"

case "$TARGET_TRIPLE" in
  x86_64-unknown-linux-gnu)
    BINARY_NAME="exa-postgres-interface-v${VERSION}-linux-x86_64"
    ;;
  aarch64-unknown-linux-gnu)
    BINARY_NAME="exa-postgres-interface-v${VERSION}-linux-aarch64"
    ;;
  *)
    BINARY_NAME="exa-postgres-interface-v${VERSION}-${TARGET_TRIPLE}"
    ;;
esac

DIST_DIR="$ROOT/dist"
STAGE_DIR="$DIST_DIR/$BINARY_NAME"

cargo build --release --bins

rm -rf "$STAGE_DIR"
mkdir -p "$STAGE_DIR/bin" "$STAGE_DIR/config" "$STAGE_DIR/packaging" "$STAGE_DIR/sql" "$STAGE_DIR/docs"

install -m 0755 "$ROOT/target/release/exa-postgres-interface" "$STAGE_DIR/bin/exa-postgres-interface"
install -m 0755 "$ROOT/target/release/exasol_exec" "$STAGE_DIR/bin/exasol_exec"
install -m 0644 "$ROOT/config/example.toml" "$STAGE_DIR/config/example.toml"
install -m 0644 "$ROOT/packaging/exa-postgres-interface.service" "$STAGE_DIR/packaging/exa-postgres-interface.service"
install -m 0644 "$ROOT/sql/postgres_catalog_compatibility.sql" "$STAGE_DIR/sql/postgres_catalog_compatibility.sql"
install -m 0644 "$ROOT/sql/exasol_sql_preprocessor.sql" "$STAGE_DIR/sql/exasol_sql_preprocessor.sql"
install -m 0644 "$ROOT/README.md" "$STAGE_DIR/README.md"
install -m 0644 "$ROOT/docs/compatibility-matrix.md" "$STAGE_DIR/docs/compatibility-matrix.md"
install -m 0644 "$ROOT/docs/postgres-metadata-compatibility.md" "$STAGE_DIR/docs/postgres-metadata-compatibility.md"
install -m 0644 "$ROOT/docs/smoke-test.md" "$STAGE_DIR/docs/smoke-test.md"

(
  cd "$DIST_DIR"
  tar -czf "$BINARY_NAME.tar.gz" "$BINARY_NAME"
  sha256sum "$BINARY_NAME.tar.gz" > "$BINARY_NAME.tar.gz.sha256"
)

echo "$DIST_DIR/$BINARY_NAME.tar.gz"
echo "$DIST_DIR/$BINARY_NAME.tar.gz.sha256"
