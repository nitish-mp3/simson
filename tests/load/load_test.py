#!/usr/bin/env python3
"""Load-test harness for HA VoIP.

Simulates concurrent SIP registrations and calls against a running
voip-engine instance, measuring call-setup latency, CPU, and memory.

Usage:
    python tests/load/load_test.py --config tests/load/benchmark_config.yaml
    python tests/load/load_test.py --host 127.0.0.1 --grpc-port 50051 --users 50

Output is written as JSON (results.json) and a human-readable Markdown
table (results.md) in the current directory.
"""

from __future__ import annotations

import argparse
import asyncio
import json
import os
import platform
import statistics
import sys
import time
from dataclasses import dataclass, field, asdict
from pathlib import Path
from typing import Any

import yaml  # type: ignore[import-untyped]

# ---------------------------------------------------------------------------
# Data classes
# ---------------------------------------------------------------------------


@dataclass
class BenchmarkConfig:
    host: str = "127.0.0.1"
    grpc_port: int = 50051
    sip_port: int = 5060
    user_counts: list[int] = field(default_factory=lambda: [50, 100, 250])
    registration_timeout_sec: float = 5.0
    call_setup_timeout_sec: float = 10.0
    call_duration_sec: float = 2.0
    warmup_sec: float = 1.0
    cooldown_sec: float = 1.0
    output_json: str = "results.json"
    output_md: str = "results.md"

    @classmethod
    def from_yaml(cls, path: str) -> "BenchmarkConfig":
        raw = yaml.safe_load(Path(path).read_text())
        return cls(**{k: v for k, v in raw.items() if k in cls.__dataclass_fields__})


@dataclass
class RunResult:
    concurrency: int = 0
    registrations_attempted: int = 0
    registrations_succeeded: int = 0
    registrations_failed: int = 0
    calls_attempted: int = 0
    calls_succeeded: int = 0
    calls_failed: int = 0
    avg_registration_ms: float = 0.0
    p50_registration_ms: float = 0.0
    p95_registration_ms: float = 0.0
    p99_registration_ms: float = 0.0
    avg_call_setup_ms: float = 0.0
    p50_call_setup_ms: float = 0.0
    p95_call_setup_ms: float = 0.0
    p99_call_setup_ms: float = 0.0
    elapsed_sec: float = 0.0
    peak_cpu_pct: float = 0.0
    peak_memory_mb: float = 0.0


# ---------------------------------------------------------------------------
# Simulated SIP client (gRPC-based for the load test)
# ---------------------------------------------------------------------------


async def simulate_register(
    host: str,
    port: int,
    extension: int,
    timeout: float,
) -> tuple[bool, float]:
    """Simulate a SIP REGISTER by opening a TCP connection to the gRPC port.

    Returns (success, latency_ms).
    In a full implementation this would use the generated gRPC stubs.
    """
    start = time.monotonic()
    try:
        reader, writer = await asyncio.wait_for(
            asyncio.open_connection(host, port),
            timeout=timeout,
        )
        elapsed_ms = (time.monotonic() - start) * 1000
        writer.close()
        await writer.wait_closed()
        return True, elapsed_ms
    except (OSError, asyncio.TimeoutError):
        elapsed_ms = (time.monotonic() - start) * 1000
        return False, elapsed_ms


async def simulate_call(
    host: str,
    port: int,
    from_ext: int,
    to_ext: int,
    duration_sec: float,
    timeout: float,
) -> tuple[bool, float]:
    """Simulate a call setup.

    Connects to the gRPC endpoint, waits *duration_sec*, then disconnects.
    Returns (success, setup_latency_ms).
    """
    start = time.monotonic()
    try:
        reader, writer = await asyncio.wait_for(
            asyncio.open_connection(host, port),
            timeout=timeout,
        )
        setup_ms = (time.monotonic() - start) * 1000
        await asyncio.sleep(duration_sec)
        writer.close()
        await writer.wait_closed()
        return True, setup_ms
    except (OSError, asyncio.TimeoutError):
        setup_ms = (time.monotonic() - start) * 1000
        return False, setup_ms


# ---------------------------------------------------------------------------
# Percentile helper
# ---------------------------------------------------------------------------


def percentile(data: list[float], pct: float) -> float:
    if not data:
        return 0.0
    sorted_data = sorted(data)
    idx = int(len(sorted_data) * pct / 100)
    idx = min(idx, len(sorted_data) - 1)
    return sorted_data[idx]


# ---------------------------------------------------------------------------
# Run one scenario
# ---------------------------------------------------------------------------


async def run_scenario(cfg: BenchmarkConfig, n: int) -> RunResult:
    """Run a single concurrency scenario with *n* users."""
    result = RunResult(concurrency=n)

    # -- Warm-up --
    await asyncio.sleep(cfg.warmup_sec)
    overall_start = time.monotonic()

    # -- Registrations --
    reg_tasks = [
        simulate_register(cfg.host, cfg.grpc_port, ext, cfg.registration_timeout_sec)
        for ext in range(1000, 1000 + n)
    ]
    reg_results = await asyncio.gather(*reg_tasks)
    reg_latencies = []
    for success, lat in reg_results:
        result.registrations_attempted += 1
        if success:
            result.registrations_succeeded += 1
            reg_latencies.append(lat)
        else:
            result.registrations_failed += 1

    if reg_latencies:
        result.avg_registration_ms = statistics.mean(reg_latencies)
        result.p50_registration_ms = percentile(reg_latencies, 50)
        result.p95_registration_ms = percentile(reg_latencies, 95)
        result.p99_registration_ms = percentile(reg_latencies, 99)

    # -- Calls (pair up extensions: 1000->1001, 1002->1003, ...) --
    call_pairs = [(1000 + i, 1000 + i + 1) for i in range(0, n - 1, 2)]
    call_tasks = [
        simulate_call(
            cfg.host,
            cfg.grpc_port,
            a,
            b,
            cfg.call_duration_sec,
            cfg.call_setup_timeout_sec,
        )
        for a, b in call_pairs
    ]
    call_results = await asyncio.gather(*call_tasks)
    call_latencies = []
    for success, lat in call_results:
        result.calls_attempted += 1
        if success:
            result.calls_succeeded += 1
            call_latencies.append(lat)
        else:
            result.calls_failed += 1

    if call_latencies:
        result.avg_call_setup_ms = statistics.mean(call_latencies)
        result.p50_call_setup_ms = percentile(call_latencies, 50)
        result.p95_call_setup_ms = percentile(call_latencies, 95)
        result.p99_call_setup_ms = percentile(call_latencies, 99)

    # -- Cool-down --
    await asyncio.sleep(cfg.cooldown_sec)
    result.elapsed_sec = time.monotonic() - overall_start

    # -- Resource usage (best-effort) --
    try:
        import psutil  # type: ignore[import-untyped]

        proc = psutil.Process()
        result.peak_cpu_pct = proc.cpu_percent(interval=0.5)
        result.peak_memory_mb = proc.memory_info().rss / (1024 * 1024)
    except ImportError:
        pass

    return result


# ---------------------------------------------------------------------------
# Output formatters
# ---------------------------------------------------------------------------


def write_json(results: list[RunResult], path: str):
    with open(path, "w") as f:
        json.dump([asdict(r) for r in results], f, indent=2)
    print(f"JSON results written to {path}")


def write_markdown(results: list[RunResult], path: str):
    lines = [
        "# HA VoIP Load Test Results",
        "",
        f"Date: {time.strftime('%Y-%m-%d %H:%M:%S')}",
        f"Platform: {platform.platform()}",
        "",
        "| Concurrency | Reg OK | Reg Fail | Avg Reg (ms) | P95 Reg (ms) "
        "| Calls OK | Calls Fail | Avg Setup (ms) | P95 Setup (ms) | Elapsed (s) |",
        "|------------|--------|----------|-------------|-------------|"
        "----------|------------|----------------|----------------|-------------|",
    ]
    for r in results:
        lines.append(
            f"| {r.concurrency} | {r.registrations_succeeded} | {r.registrations_failed} "
            f"| {r.avg_registration_ms:.1f} | {r.p95_registration_ms:.1f} "
            f"| {r.calls_succeeded} | {r.calls_failed} "
            f"| {r.avg_call_setup_ms:.1f} | {r.p95_call_setup_ms:.1f} "
            f"| {r.elapsed_sec:.2f} |"
        )
    lines.append("")

    with open(path, "w") as f:
        f.write("\n".join(lines))
    print(f"Markdown results written to {path}")


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------


async def main(cfg: BenchmarkConfig):
    results: list[RunResult] = []
    for n in cfg.user_counts:
        print(f"\n=== Running scenario: {n} concurrent users ===")
        result = await run_scenario(cfg, n)
        results.append(result)
        print(
            f"  Registrations: {result.registrations_succeeded}/{result.registrations_attempted}  "
            f"Calls: {result.calls_succeeded}/{result.calls_attempted}  "
            f"Avg setup: {result.avg_call_setup_ms:.1f} ms  "
            f"Elapsed: {result.elapsed_sec:.2f} s"
        )

    write_json(results, cfg.output_json)
    write_markdown(results, cfg.output_md)


def cli():
    parser = argparse.ArgumentParser(description="HA VoIP load test harness")
    parser.add_argument("--config", type=str, help="Path to benchmark_config.yaml")
    parser.add_argument("--host", type=str, default="127.0.0.1")
    parser.add_argument("--grpc-port", type=int, default=50051)
    parser.add_argument("--users", type=int, nargs="*", default=None,
                        help="Space-separated concurrency levels (e.g. 50 100 250)")
    args = parser.parse_args()

    if args.config:
        cfg = BenchmarkConfig.from_yaml(args.config)
    else:
        cfg = BenchmarkConfig(
            host=args.host,
            grpc_port=args.grpc_port,
            user_counts=args.users or [50, 100, 250],
        )

    asyncio.run(main(cfg))


if __name__ == "__main__":
    cli()
