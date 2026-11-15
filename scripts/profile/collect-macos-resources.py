#!/usr/bin/env python3
"""Read-only native process-family diagnostic; never launches or signs an app.

Sampling can miss short-lived processes and peaks. This is not a release receipt
or network/GPU attribution trace. Pair it with native scenarios and profiling.
"""

import argparse
import ctypes
import hashlib
import json
import math
from pathlib import Path
import platform
import subprocess
import sys
import time


FIELDS = (
    "user_time", "system_time", "pkg_idle_wkups", "interrupt_wkups", "pageins",
    "wired_size", "resident_size", "phys_footprint", "proc_start_abstime",
    "proc_exit_abstime", "child_user_time", "child_system_time",
    "child_pkg_idle_wkups", "child_interrupt_wkups", "child_pageins",
    "child_elapsed_abstime", "diskio_bytesread", "diskio_byteswritten",
)


class Usage(ctypes.Structure):
    _fields_ = [("uuid", ctypes.c_uint8 * 16)] + [
        (name, ctypes.c_uint64) for name in FIELDS
    ]


def process_table():
    table = {}
    output = subprocess.check_output(["ps", "-axo", "pid=,ppid=,comm="], text=True)
    for line in output.splitlines():
        parts = line.strip().split(None, 2)
        if len(parts) == 3:
            table[int(parts[0])] = (int(parts[1]), parts[2])
    return table


def family(table, bundle, root_pid):
    """Bundle executables and descendants, excluding similarly named installs."""
    prefix = str(bundle) + "/"
    if root_pid not in table or not table[root_pid][1].startswith(prefix):
        raise ValueError("Root PID no longer belongs to the requested bundle")
    selected = {pid for pid, (_, path) in table.items() if path.startswith(prefix)}
    while True:
        children = {pid for pid, (parent, _) in table.items() if parent in selected}
        if children <= selected:
            return selected
        selected.update(children)


def summarize(samples, memory_limit, cpu_nanoseconds_per_tick=1):
    totals = [sum(p["phys_footprint"] for p in row["processes"]) for row in samples]
    previous = {}
    counters = {name: 0 for name in (
        "user_time", "system_time", "pkg_idle_wkups", "interrupt_wkups",
        "diskio_bytesread", "diskio_byteswritten",
    )}
    for row in samples:
        for process in row["processes"]:
            identity = process["identity"]
            before = previous.get(identity)
            if before is not None:
                for name in counters:
                    counters[name] += max(0, process[name] - before[name])
            previous[identity] = process
    elapsed = samples[-1]["elapsed_seconds"] - samples[0]["elapsed_seconds"] if len(samples) > 1 else 0
    return {
        "samples": len(samples), "elapsed_seconds": elapsed,
        "peak_bytes": max(totals) if totals else None,
        "last_bytes": totals[-1] if totals else None,
        "observed_memory_budget_pass": bool(totals) and max(totals) < memory_limit,
        "cpu_percent_one_core": ((counters["user_time"] + counters["system_time"]) * cpu_nanoseconds_per_tick / 1e9 / elapsed * 100) if elapsed else None,
        "interrupt_wakeups_per_second": counters["interrupt_wkups"] / elapsed if elapsed else None,
        "idle_wakeups_per_second": counters["pkg_idle_wkups"] / elapsed if elapsed else None,
        "disk_read_bytes_per_second": counters["diskio_bytesread"] / elapsed if elapsed else None,
        "disk_write_bytes_per_second": counters["diskio_byteswritten"] / elapsed if elapsed else None,
        "network_bytes_per_second": None,
        "counter_deltas": counters,
    }


def positive_number(value):
    number = float(value)
    if not math.isfinite(number) or number <= 0:
        raise argparse.ArgumentTypeError("must be finite and positive")
    return number


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--pid", required=True, type=int)
    parser.add_argument("--bundle", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--duration", type=positive_number, default=120)
    parser.add_argument("--interval", type=positive_number, default=0.25)
    parser.add_argument("--scenario", required=True)
    parser.add_argument("--source-receipt", required=True, type=Path)
    parser.add_argument("--artifact-receipt", required=True, type=Path)
    args = parser.parse_args()
    if sys.platform != "darwin":
        parser.error("requires a native macOS host")
    if args.duration < args.interval or args.output.exists():
        parser.error("duration must cover an interval; output must be a new file")
    bundle = args.bundle.resolve(strict=True)
    identity = {}
    for name, path in [("source_receipt", args.source_receipt), ("artifact_receipt", args.artifact_receipt)]:
        identity[name] = {"path": str(path.resolve(strict=True)), "sha256": hashlib.sha256(path.read_bytes()).hexdigest()}
    budgets_path = Path(__file__).resolve().parents[2] / "config/runtime-resource-budgets.json"
    budgets = json.loads(budgets_path.read_text())
    memory_limit = budgets["platforms"]["macos"]["ordinary_memory_limit_bytes"]
    libc = ctypes.CDLL("/usr/lib/libSystem.B.dylib", use_errno=True)
    class Timebase(ctypes.Structure):
        _fields_ = [("numer", ctypes.c_uint32), ("denom", ctypes.c_uint32)]
    timebase = Timebase()
    if libc.mach_timebase_info(ctypes.byref(timebase)) or not timebase.denom:
        parser.error("cannot establish native CPU counter timebase")
    libc.proc_pid_rusage.argtypes = [ctypes.c_int, ctypes.c_int, ctypes.c_void_p]
    libc.proc_pid_rusage.restype = ctypes.c_int
    assert ctypes.sizeof(Usage) == 160
    samples, errors, identities = [], [], {}
    started, wall_started = time.monotonic(), time.time()
    root_identity = None
    try:
        while True:
            tick = time.monotonic()
            table = process_table()
            selected = family(table, bundle, args.pid)
            # Retain observed descendants after reparenting, but never PID reuse.
            selected.update(pid for pid in identities if pid in table)
            row = {"elapsed_seconds": tick - started, "processes": []}
            for pid in sorted(selected):
                usage = Usage()
                if libc.proc_pid_rusage(pid, 2, ctypes.byref(usage)):
                    errors.append({"pid": pid, "errno": ctypes.get_errno(), "elapsed_seconds": tick-started})
                    continue
                process_identity = f"{pid}:{usage.proc_start_abstime}"
                if pid in identities and identities[pid] != process_identity:
                    raise ValueError("Observed PID was reused; restart the diagnostic")
                if pid == args.pid:
                    if root_identity is not None and process_identity != root_identity:
                        raise ValueError("Root process identity changed")
                    root_identity = process_identity
                identities[pid] = process_identity
                row["processes"].append({
                    "pid": pid, "identity": process_identity,
                    "role": "app" if pid == args.pid else "daemon" if Path(table[pid][1]).name == "copypaste-daemon" else "worker",
                    **{name: getattr(usage, name) for name in FIELDS},
                })
            samples.append(row)
            if tick-started >= args.duration:
                break
            time.sleep(max(0, min(args.interval, args.duration-(tick-started)) - (time.monotonic()-tick)))
    except (OSError, ValueError, subprocess.SubprocessError, KeyboardInterrupt) as error:
        errors.append({"error": type(error).__name__, "message": str(error)})
    summary = summarize(samples, memory_limit, timebase.numer / timebase.denom)
    collection_complete = not errors and len(samples) >= 2
    result = {
        "schema_version": 1, "platform": "macos", "os": platform.platform(),
        "bundle": str(bundle), "root_identity": root_identity, "scenario": args.scenario,
        "started_unix_seconds": wall_started, "sample_interval_seconds": args.interval,
        "memory_metric": "physical_footprint_bytes", "memory_limit_bytes": memory_limit,
        "cpu_timebase": {"numer": timebase.numer, "denom": timebase.denom},
        "budget_version": budgets["budget_version"], "identity": identity,
        "release_qualified": False,
        "collection_complete": collection_complete,
        "resource_check_pass": collection_complete and summary["observed_memory_budget_pass"],
        "limitations": ["Sampling misses sub-interval peaks and short-lived workers", "Counters omit work before first and after final observation of each process", "Network and separate GPU/shared attribution require native tracing", "Receipt references are not verified release provenance"],
        "errors": errors, "summary": summary, "samples": samples,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open("x") as output:
        json.dump(result, output, indent=2)
        output.write("\n")
    print(json.dumps({"output": str(args.output), "collection_complete": collection_complete,
                      "resource_check_pass": result["resource_check_pass"],
                      "summary": summary, "errors": errors}))
    return 2 if errors or len(samples) < 2 else 0 if summary["observed_memory_budget_pass"] else 1


if __name__ == "__main__":
    sys.exit(main())
