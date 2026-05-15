#!/usr/bin/env python3
"""Stress test the gateway with parallel sessions and compare against direct Exasol.

Each worker opens its own connection, runs a fixed mix of three query shapes for
N iterations, then closes. Reports total wall time, throughput, per-query
latency percentiles, and any errors encountered.

Requires:
    pip install 'psycopg[binary]' pyexasol

Usage:
    scripts/stress_parallel_sessions.py \\
        --gateway-host 127.0.0.1 --gateway-port 15432 \\
        --gateway-user sys --gateway-password EXASOL_PASSWORD \\
        --exasol-dsn 127.0.0.1:8563 \\
        --exasol-user sys --exasol-password EXASOL_PASSWORD \\
        --schema CORE_DB_2026_1_DEMOS --table SHIPMENTS \\
        --sessions 50 --iterations 20
"""
from __future__ import annotations

import argparse
import ssl
import statistics
import sys
import time
import traceback
from concurrent.futures import ThreadPoolExecutor, as_completed
from dataclasses import dataclass, field
from typing import Callable

try:
    import psycopg
except ImportError as exc:
    sys.exit(f"psycopg is required: pip install 'psycopg[binary]' ({exc})")

try:
    import pyexasol
except ImportError as exc:
    sys.exit(f"pyexasol is required: pip install pyexasol ({exc})")


@dataclass
class WorkerResult:
    worker_id: int
    latencies_ms: list[float] = field(default_factory=list)
    errors: list[str] = field(default_factory=list)
    connect_ms: float = 0.0


@dataclass
class RunSummary:
    label: str
    sessions: int
    iterations: int
    wall_ms: float
    latencies_ms: list[float]
    connect_latencies_ms: list[float]
    errors: list[str]

    @property
    def total_queries(self) -> int:
        return len(self.latencies_ms)

    @property
    def qps(self) -> float:
        return self.total_queries / (self.wall_ms / 1000.0) if self.wall_ms else 0.0


def percentile(values: list[float], pct: float) -> float:
    if not values:
        return 0.0
    ordered = sorted(values)
    idx = max(0, min(len(ordered) - 1, int(round(pct / 100.0 * (len(ordered) - 1)))))
    return ordered[idx]


def run_gateway_worker(
    worker_id: int,
    iterations: int,
    queries: list[str],
    conn_kwargs: dict,
) -> WorkerResult:
    result = WorkerResult(worker_id=worker_id)
    connect_start = time.perf_counter()
    try:
        conn = psycopg.connect(**conn_kwargs)
    except Exception as exc:
        result.errors.append(f"connect: {exc}")
        return result
    result.connect_ms = (time.perf_counter() - connect_start) * 1000.0

    try:
        with conn.cursor() as cur:
            for i in range(iterations):
                sql = queries[i % len(queries)]
                started = time.perf_counter()
                try:
                    cur.execute(sql)
                    if cur.description is not None:
                        cur.fetchall()
                    elapsed_ms = (time.perf_counter() - started) * 1000.0
                    result.latencies_ms.append(elapsed_ms)
                except Exception as exc:
                    result.errors.append(f"iter {i} `{sql[:40]}...`: {exc}")
    finally:
        try:
            conn.close()
        except Exception:
            pass
    return result


def run_exasol_worker(
    worker_id: int,
    iterations: int,
    queries: list[str],
    conn_kwargs: dict,
) -> WorkerResult:
    result = WorkerResult(worker_id=worker_id)
    connect_start = time.perf_counter()
    try:
        conn = pyexasol.connect(**conn_kwargs)
    except Exception as exc:
        result.errors.append(f"connect: {exc}")
        return result
    result.connect_ms = (time.perf_counter() - connect_start) * 1000.0

    try:
        for i in range(iterations):
            sql = queries[i % len(queries)]
            started = time.perf_counter()
            try:
                stmt = conn.execute(sql)
                if stmt.columns():
                    stmt.fetchall()
                elapsed_ms = (time.perf_counter() - started) * 1000.0
                result.latencies_ms.append(elapsed_ms)
            except Exception as exc:
                result.errors.append(f"iter {i} `{sql[:40]}...`: {exc}")
    finally:
        try:
            conn.close()
        except Exception:
            pass
    return result


def run_pool(
    label: str,
    sessions: int,
    iterations: int,
    queries: list[str],
    worker_fn: Callable[..., WorkerResult],
    conn_kwargs: dict,
) -> RunSummary:
    print(f"\n=== {label}: {sessions} sessions x {iterations} iterations ===", flush=True)
    started = time.perf_counter()
    results: list[WorkerResult] = []
    with ThreadPoolExecutor(max_workers=sessions) as pool:
        futures = [
            pool.submit(worker_fn, wid, iterations, queries, conn_kwargs)
            for wid in range(sessions)
        ]
        for future in as_completed(futures):
            try:
                results.append(future.result())
            except Exception as exc:
                # Should not happen — workers swallow their own exceptions.
                err = WorkerResult(worker_id=-1)
                err.errors.append(f"worker crashed: {exc}\n{traceback.format_exc()}")
                results.append(err)
    wall_ms = (time.perf_counter() - started) * 1000.0

    latencies = [lat for r in results for lat in r.latencies_ms]
    connect_latencies = [r.connect_ms for r in results if r.connect_ms > 0]
    errors = [err for r in results for err in r.errors]
    return RunSummary(
        label=label,
        sessions=sessions,
        iterations=iterations,
        wall_ms=wall_ms,
        latencies_ms=latencies,
        connect_latencies_ms=connect_latencies,
        errors=errors,
    )


def warmup(label: str, worker_fn, queries: list[str], conn_kwargs: dict) -> None:
    print(f"warmup {label}...", flush=True)
    res = worker_fn(-1, len(queries), queries, conn_kwargs)
    if res.errors:
        print(f"  warmup errors on {label}:", flush=True)
        for err in res.errors:
            print(f"    {err}", flush=True)


def print_summary(summary: RunSummary) -> None:
    print(f"\n--- {summary.label} ---", flush=True)
    print(f"  wall time         : {summary.wall_ms:>10.1f} ms", flush=True)
    print(f"  queries executed  : {summary.total_queries:>10d}", flush=True)
    print(f"  throughput        : {summary.qps:>10.1f} qps", flush=True)
    print(f"  errors            : {len(summary.errors):>10d}", flush=True)
    if summary.latencies_ms:
        print(
            "  query latency ms  : "
            f"p50={percentile(summary.latencies_ms, 50):.1f} "
            f"p95={percentile(summary.latencies_ms, 95):.1f} "
            f"p99={percentile(summary.latencies_ms, 99):.1f} "
            f"max={max(summary.latencies_ms):.1f} "
            f"mean={statistics.fmean(summary.latencies_ms):.1f}",
            flush=True,
        )
    if summary.connect_latencies_ms:
        print(
            "  connect latency ms: "
            f"p50={percentile(summary.connect_latencies_ms, 50):.1f} "
            f"p95={percentile(summary.connect_latencies_ms, 95):.1f} "
            f"max={max(summary.connect_latencies_ms):.1f}",
            flush=True,
        )
    if summary.errors:
        print("  first 5 errors:", flush=True)
        for err in summary.errors[:5]:
            print(f"    - {err}", flush=True)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--gateway-host", default="127.0.0.1")
    parser.add_argument("--gateway-port", type=int, default=15432)
    parser.add_argument("--gateway-user", required=True)
    parser.add_argument("--gateway-password", required=True)
    parser.add_argument("--gateway-database", default="exasol")
    parser.add_argument("--exasol-dsn", required=True, help="host:port of the Exasol cluster")
    parser.add_argument("--exasol-user", required=True)
    parser.add_argument("--exasol-password", required=True)
    parser.add_argument("--schema", required=True, help="schema containing the COUNT(*) target")
    parser.add_argument("--table", required=True, help="table for the COUNT(*) probe")
    parser.add_argument("--sessions", type=int, default=50)
    parser.add_argument("--iterations", type=int, default=20)
    parser.add_argument("--skip-direct", action="store_true", help="only run the gateway side")
    parser.add_argument("--skip-gateway", action="store_true", help="only run the direct-Exasol side")
    args = parser.parse_args()

    fully_qualified = f'"{args.schema}"."{args.table}"'
    queries = [
        "SELECT 1",
        "SELECT table_name FROM information_schema.tables WHERE table_schema = '"
        + args.schema.replace("'", "''")
        + "' LIMIT 10",
        f"SELECT COUNT(*) FROM {fully_qualified}",
    ]

    summaries: list[RunSummary] = []

    if not args.skip_gateway:
        gateway_conn_kwargs = dict(
            host=args.gateway_host,
            port=args.gateway_port,
            user=args.gateway_user,
            password=args.gateway_password,
            dbname=args.gateway_database,
            autocommit=True,
        )
        warmup("gateway", run_gateway_worker, queries, gateway_conn_kwargs)
        summaries.append(
            run_pool(
                "gateway (PG wire)",
                args.sessions,
                args.iterations,
                queries,
                run_gateway_worker,
                gateway_conn_kwargs,
            )
        )

    if not args.skip_direct:
        exasol_conn_kwargs = dict(
            dsn=args.exasol_dsn,
            user=args.exasol_user,
            password=args.exasol_password,
            encryption=True,
            websocket_sslopt={"cert_reqs": ssl.CERT_NONE},
            client_name="exa-postgres-interface-stress",
        )
        warmup("direct exasol", run_exasol_worker, queries, exasol_conn_kwargs)
        summaries.append(
            run_pool(
                "direct exasol (websocket)",
                args.sessions,
                args.iterations,
                queries,
                run_exasol_worker,
                exasol_conn_kwargs,
            )
        )

    print("\n========== results ==========", flush=True)
    for summary in summaries:
        print_summary(summary)

    if len(summaries) == 2 and summaries[0].latencies_ms and summaries[1].latencies_ms:
        gateway, direct = summaries
        ratio = statistics.fmean(gateway.latencies_ms) / statistics.fmean(direct.latencies_ms)
        wall_ratio = gateway.wall_ms / direct.wall_ms if direct.wall_ms else float("inf")
        print(
            f"\ngateway vs direct: mean latency {ratio:.2f}x, "
            f"wall time {wall_ratio:.2f}x",
            flush=True,
        )

    return 0 if all(not s.errors for s in summaries) else 1


if __name__ == "__main__":
    sys.exit(main())
