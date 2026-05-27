#!/usr/bin/env bash
# Compare websocket vs arrow Exasol transport throughput across result-set sizes.
# Usage: scripts/transport_bench.sh <dsn> <user> <password>
set -euo pipefail

DSN="${1:?dsn}"; USER="${2:?user}"; PASS="${3:?password}"
BIN="./target/release/exasol_exec"

# Digit CTE; cross-joining k aliases yields 10^k rows. Same 4-column projection
# (int, decimal, varchar, int) regardless of size so per-row payload is fixed.
DIGITS="WITH d(n) AS (SELECT 0 UNION ALL SELECT 1 UNION ALL SELECT 2 UNION ALL SELECT 3 UNION ALL SELECT 4 UNION ALL SELECT 5 UNION ALL SELECT 6 UNION ALL SELECT 7 UNION ALL SELECT 8 UNION ALL SELECT 9)"
PROJ="SELECT (a.n + b.n*10 + c.n*100) AS id, (a.n*7 + b.n)*1.25 AS val, 'row_' || (a.n + b.n*10) AS label, MOD(c.n,2) AS flag"

# name | repeat | FROM clause (alias count sets row volume)
CASES=(
  "1_row|30|SELECT 1 AS id, 1.0 AS val, 'x' AS label, 0 AS flag"
  "1k_rows|20|$DIGITS $PROJ FROM d a, d b, d c"
  "100k_rows|10|$DIGITS $PROJ FROM d a, d b, d c, d e, d f"
  "1M_rows|6|$DIGITS $PROJ FROM d a, d b, d c, d e, d f, d g"
)

run() {  # transport, repeat, sql -> echoes "rows median_ms mean_ms"
  $BIN --dsn "$DSN" --user "$USER" --password "$PASS" --no-verify \
       --transport "$1" --repeat "$2" --sql "$3" 2>/dev/null \
    | awk -F= '/^rows=/{r=$2} /^min_ms=/{print} END{}' \
    | sed -n "s/.*median_ms=\([0-9.]*\).*mean_ms=\([0-9.]*\).*/\1 \2/p" \
    | { read med mean; echo "$med $mean"; }
}

rows_of() { $BIN --dsn "$DSN" --user "$USER" --password "$PASS" --no-verify --transport websocket --repeat 2 --sql "$1" 2>/dev/null | sed -n 's/^rows=//p'; }

printf '%-10s %10s %12s %12s %10s\n' "case" "rows" "ws_median" "arrow_median" "speedup"
printf '%-10s %10s %12s %12s %10s\n' "----" "----" "---------" "------------" "-------"
for c in "${CASES[@]}"; do
  IFS='|' read -r name rep sql <<<"$c"
  rows=$(rows_of "$sql")
  read ws_med ws_mean < <(run websocket "$rep" "$sql")
  read ar_med ar_mean < <(run arrow "$rep" "$sql")
  speedup=$(awk -v w="$ws_med" -v a="$ar_med" 'BEGIN{ if(a>0) printf "%.2fx", w/a; else print "n/a" }')
  printf '%-10s %10s %10s ms %10s ms %10s\n' "$name" "$rows" "$ws_med" "$ar_med" "$speedup"
done
