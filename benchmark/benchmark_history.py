#!/usr/bin/env python3
"""benchmark_history.py — Read results.json and print a comparison table."""

import json
import sys
from pathlib import Path

RESULTS_FILE = Path(__file__).parent / "results.json"

LABELS = {
    "ash_c": "Ash (compiled)",
    "ash_i": "Ash (interpreted)",
    "go": "Go (native)",
    "python": "Python",
    "java": "Java (JVM)",
    "js": "JavaScript (Node)",
    "ts": "TypeScript (Deno)",
}

COLORS = {
    "ash_c": "\033[92m",
    "ash_i": "\033[93m",
    "go": "\033[94m",
    "python": "\033[96m",
    "java": "\033[95m",
    "js": "\033[92m",
    "ts": "\033[94m",
}
RESET = "\033[0m"
BOLD = "\033[1m"

def main():
    if not RESULTS_FILE.exists():
        print(f"No results file found at {RESULTS_FILE}")
        sys.exit(1)

    with open(RESULTS_FILE) as f:
        raw = f.read()

    # Parse the custom JSON format (each line is a separate JSON object, but
    # we wrote them as one JSON object per entry without commas between entries)
    # Actually let's just read them as a JSON array
    try:
        data = json.loads(raw)
    except json.JSONDecodeError:
        # Try parsing as individual objects separated by newlines
        data = []
        for line in raw.strip().split("\n"):
            line = line.strip()
            if line:
                try:
                    data.append(json.loads(line))
                except json.JSONDecodeError:
                    pass

    if not data:
        print("No valid results found.")
        return

    # Show the latest result
    latest = data[-1] if isinstance(data, list) else data
    if isinstance(data, list):
        latest = data[-1]
    elif isinstance(data, dict) and "timestamp" in data:
        latest = data

    results = latest.get("results", latest)
    ts = latest.get("timestamp", "unknown")
    runs = latest.get("runs", 20)

    print(f"\n{BOLD}Ash Benchmark Results — {ts}{RESET}")
    print(f"Runs per test: {runs}\n")

    # Sort by speed
    sorted_results = sorted(
        [(k, v) for k, v in results.items() if isinstance(v, (int, float))],
        key=lambda x: x[1],
    )

    if not sorted_results:
        print("No timing results found.")
        return

    python_ms = results.get("python", None)
    go_ms = results.get("go", None)

    print(f"{'Language':<25} {'ms/run':>10} {'vs Python':>14} {'vs Go':>14}")
    print(f"{'-'*25} {'-'*10} {'-'*14} {'-'*14}")

    for key, ms in sorted_results:
        label = LABELS.get(key, key)
        color = COLORS.get(key, "")

        vs_py = ""
        if python_ms and python_ms > 0:
            ratio = python_ms / ms
            vs_py = f"{ratio:.1f}x faster" if ratio >= 1 else f"{1/ratio:.1f}x slower"

        vs_go = ""
        if go_ms and go_ms > 0 and key != "go":
            ratio = go_ms / ms
            vs_go = f"{ratio:.1f}x faster" if ratio >= 1 else f"{1/ratio:.1f}x slower"

        print(f"{color}{label:<25} {ms:>7}ms {vs_py:>14} {vs_go:>14}{RESET}")

    print()
    print("Summary:")
    best = sorted_results[0]
    print(f"  Fastest:  {LABELS.get(best[0], best[0])} ({best[1]}ms)")
    if python_ms:
        print(f"  Python:   {python_ms}ms")
    print()


if __name__ == "__main__":
    main()
