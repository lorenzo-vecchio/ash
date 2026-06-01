# Ash Benchmark

## Usage

```bash
# Run all benchmarks (requires: cargo, python3, go, node, javac, clang)
./benchmark/run_benchmark.sh

# View historical results
python3 benchmark/benchmark_history.py

# Token count comparison
python3 benchmark/token_count.py
```

## What's measured

**Speed benchmark:** `fib(25) + sum_range(10000) + collatz(27) + count_primes(1000)`
- Ash (compiled, via `ash build`)
- Ash (interpreted, via `ash run`)
- Go (native)
- Python (CPython)
- Java (JVM)
- JavaScript (Node.js)
- TypeScript (Deno)

**Token count:** Identical CRUD backend implementation in each language,
comparing source tokens (comments excluded).

## Results history

Results are saved to `benchmark/results.json` with timestamps.
View trends with `python3 benchmark/benchmark_history.py`.
