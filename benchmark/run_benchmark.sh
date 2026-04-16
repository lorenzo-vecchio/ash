#!/usr/bin/env bash
# run_benchmark.sh
# Compares execution speed: Ash compiled, Ash interpreted, Go, Python, Java
# All run identical logic: fib(25), count_primes(1000), collatz(27), sum_range(10000)
#
# Requirements:
#   - cargo (to build Ash)
#   - python3
#   - go
#   - java + javac
#   - clang or clang-20 (for ash build)
#
# Usage:
#   ./benchmark/run_benchmark.sh

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
TMP_DIR="$(mktemp -d)"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
BOLD='\033[1m'
NC='\033[0m'

echo ""
echo -e "${BOLD}Ash Language Benchmark${NC}"
echo -e "${BLUE}By AI, for AI${NC}"
echo ""
echo "Workload: fib(25) + count_primes(1000) + collatz(27) + sum_range(10000)"
echo ""

# ── Build Ash ────────────────────────────────────────────────────────────────

echo "Building Ash toolchain..."
cd "$ROOT_DIR" && cargo build --release -q 2>/dev/null
ASH="$ROOT_DIR/target/release/ash"
echo "Done."
echo ""

# ── Write source files ───────────────────────────────────────────────────────

cat > "$TMP_DIR/compute.ash" << 'ASH_EOF'
fn fib(n)
    if n <= 1
        n
    else
        fib(n - 1) + fib(n - 2)

fn sum_range(n)
    mut total = 0
    mut i = 0
    while i <= n
        total = total + i
        i = i + 1
    total

fn collatz(n)
    mut steps = 0
    mut x = n
    while x != 1
        if x % 2 == 0
            x = x / 2
        else
            x = x * 3 + 1
        steps = steps + 1
    steps

fn is_prime(n)
    if n < 2
        false
    else
        mut i = 2
        mut prime = true
        while i * i <= n
            if n % i == 0
                prime = false
            i = i + 1
        prime

fn count_primes(limit)
    mut count = 0
    mut n = 2
    while n <= limit
        if is_prime(n)
            count = count + 1
        n = n + 1
    count

println(fib(25))
println(sum_range(10000))
println(collatz(27))
println(count_primes(1000))
ASH_EOF

cat > "$TMP_DIR/compute.py" << 'PY_EOF'
import sys

def fib(n):
    if n <= 1: return n
    return fib(n-1) + fib(n-2)

def sum_range(n):
    total = 0
    for i in range(n+1): total += i
    return total

def collatz(n):
    steps = 0
    while n != 1:
        n = n // 2 if n % 2 == 0 else n * 3 + 1
        steps += 1
    return steps

def is_prime(n):
    if n < 2: return False
    i = 2
    while i*i <= n:
        if n % i == 0: return False
        i += 1
    return True

def count_primes(limit):
    return sum(1 for n in range(2, limit+1) if is_prime(n))

sys.setrecursionlimit(10000)
print(fib(25))
print(sum_range(10000))
print(collatz(27))
print(count_primes(1000))
PY_EOF

cat > "$TMP_DIR/compute.go" << 'GO_EOF'
package main

import "fmt"

func fib(n int) int {
    if n <= 1 { return n }
    return fib(n-1) + fib(n-2)
}

func sumRange(n int) int {
    total := 0
    for i := 0; i <= n; i++ { total += i }
    return total
}

func collatz(n int) int {
    steps := 0
    for n != 1 {
        if n%2 == 0 { n /= 2 } else { n = n*3 + 1 }
        steps++
    }
    return steps
}

func isPrime(n int) bool {
    if n < 2 { return false }
    for i := 2; i*i <= n; i++ {
        if n%i == 0 { return false }
    }
    return true
}

func countPrimes(limit int) int {
    count := 0
    for n := 2; n <= limit; n++ {
        if isPrime(n) { count++ }
    }
    return count
}

func main() {
    fmt.Println(fib(25))
    fmt.Println(sumRange(10000))
    fmt.Println(collatz(27))
    fmt.Println(countPrimes(1000))
}
GO_EOF

cat > "$TMP_DIR/Compute.java" << 'JAVA_EOF'
public class Compute {
    static int fib(int n) {
        return n <= 1 ? n : fib(n-1) + fib(n-2);
    }
    static int sumRange(int n) {
        int total = 0;
        for (int i = 0; i <= n; i++) total += i;
        return total;
    }
    static int collatz(int n) {
        int steps = 0;
        while (n != 1) { n = n%2==0 ? n/2 : n*3+1; steps++; }
        return steps;
    }
    static boolean isPrime(int n) {
        if (n < 2) return false;
        for (int i = 2; i*i <= n; i++)
            if (n%i == 0) return false;
        return true;
    }
    static int countPrimes(int limit) {
        int count = 0;
        for (int n = 2; n <= limit; n++)
            if (isPrime(n)) count++;
        return count;
    }
    public static void main(String[] args) {
        System.out.println(fib(25));
        System.out.println(sumRange(10000));
        System.out.println(collatz(27));
        System.out.println(countPrimes(1000));
    }
}
JAVA_EOF

# ── Compile native variants ──────────────────────────────────────────────────

SKIP_GO=false; SKIP_JAVA=false; SKIP_ASH_C=false

if ! command -v go &>/dev/null; then
    echo -e "${RED}Warning: go not found — skipping Go benchmark${NC}"
    SKIP_GO=true
else
    echo -n "Compiling Go... "
    cd "$TMP_DIR" && go build -o compute_go compute.go && echo "done"
fi

if ! command -v javac &>/dev/null; then
    echo -e "${RED}Warning: javac not found — skipping Java benchmark${NC}"
    SKIP_JAVA=true
else
    echo -n "Compiling Java... "
    cd "$TMP_DIR" && javac Compute.java && echo "done"
fi

echo -n "Compiling Ash to native... "
CLANG_CMD=""
for c in clang-20 clang-18 clang; do
    if command -v "$c" &>/dev/null; then CLANG_CMD="$c"; break; fi
done

if [ -z "$CLANG_CMD" ]; then
    echo -e "${RED}no clang found — skipping compiled Ash${NC}"
    SKIP_ASH_C=true
else
    if "$ASH" build "$TMP_DIR/compute.ash" -o "$TMP_DIR/compute_ash" 2>/dev/null; then
        echo "done (via $CLANG_CMD)"
    else
        echo -e "${RED}compilation failed — skipping compiled Ash${NC}"
        SKIP_ASH_C=true
    fi
fi

echo ""

# ── Verify outputs match ─────────────────────────────────────────────────────

echo "Verifying all outputs produce the same result..."
EXPECTED=$("$ASH" run "$TMP_DIR/compute.ash" 2>/dev/null)
MISMATCH=false

check_output() {
    local name="$1" output="$2"
    if [ "$output" != "$EXPECTED" ]; then
        echo -e "  ${RED}MISMATCH: $name${NC}"
        echo "    Expected: $(echo "$EXPECTED" | tr '\n' ' ')"
        echo "    Got:      $(echo "$output"   | tr '\n' ' ')"
        MISMATCH=true
    else
        echo -e "  ${GREEN}OK${NC} $name: $(echo "$output" | tr '\n' ' ')"
    fi
}

check_output "Ash interpreted" "$EXPECTED"
$SKIP_ASH_C  || check_output "Ash compiled"   "$("$TMP_DIR/compute_ash" 2>/dev/null)"
$SKIP_GO     || check_output "Go"              "$("$TMP_DIR/compute_go"  2>/dev/null)"
check_output "Python"          "$(python3 "$TMP_DIR/compute.py" 2>/dev/null)"
$SKIP_JAVA   || check_output "Java"            "$(cd "$TMP_DIR" && java Compute 2>/dev/null)"

if $MISMATCH; then
    echo ""
    echo -e "${RED}Output mismatch detected — benchmark results may be unreliable.${NC}"
fi

echo ""

# ── Timing ───────────────────────────────────────────────────────────────────

RUNS=20

time_cmd() {
    local cmd="$1" total=0
    for i in $(seq 1 $RUNS); do
        local start=$(date +%s%N)
        eval "$cmd" > /dev/null 2>&1
        local end=$(date +%s%N)
        total=$(( total + (end - start) / 1000000 ))
    done
    echo $(( total / RUNS ))
}

echo "Timing ($RUNS runs each)..."
echo ""

declare -A TIMES LABELS ORDER

idx=0

if ! $SKIP_ASH_C; then
    echo -n "  Ash compiled...    "
    TIMES[ash_c]=$(time_cmd "$TMP_DIR/compute_ash")
    LABELS[ash_c]="Ash (compiled)"
    echo "${TIMES[ash_c]}ms"
fi

if ! $SKIP_GO; then
    echo -n "  Go native...       "
    TIMES[go]=$(time_cmd "$TMP_DIR/compute_go")
    LABELS[go]="Go (native)"
    echo "${TIMES[go]}ms"
fi

echo -n "  Python...          "
TIMES[python]=$(time_cmd "python3 $TMP_DIR/compute.py")
LABELS[python]="Python (CPython)"
echo "${TIMES[python]}ms"

if ! $SKIP_JAVA; then
    echo -n "  Java...            "
    TIMES[java]=$(time_cmd "cd $TMP_DIR && java Compute")
    LABELS[java]="Java (JVM)"
    echo "${TIMES[java]}ms"
fi

echo -n "  Ash interpreted... "
TIMES[ash_i]=$(time_cmd "$ASH run $TMP_DIR/compute.ash")
LABELS[ash_i]="Ash (interpreted)"
echo "${TIMES[ash_i]}ms"

# ── Results table ────────────────────────────────────────────────────────────

echo ""
echo -e "${BOLD}Results${NC} — $RUNS runs each, cold start per run"
echo ""
printf "%-28s %10s %14s\n" "Language" "ms/run" "vs Python"
printf "%-28s %10s %14s\n" "--------" "------" "---------"

PY=${TIMES[python]}

print_row() {
    local key="$1"
    [ -z "${TIMES[$key]}" ] && return
    local ms="${TIMES[$key]}"
    local label="${LABELS[$key]}"
    if [ "$ms" -lt "$PY" ]; then
        local ratio=$(echo "scale=1; $PY / $ms" | bc 2>/dev/null || echo "?")
        printf "${GREEN}%-28s %10s %14s${NC}\n" "$label" "${ms}ms" "${ratio}x faster"
    elif [ "$ms" -eq "$PY" ]; then
        printf "%-28s %10s %14s\n" "$label" "${ms}ms" "baseline"
    else
        local ratio=$(echo "scale=1; $ms / $PY" | bc 2>/dev/null || echo "?")
        printf "${RED}%-28s %10s %14s${NC}\n" "$label" "${ms}ms" "${ratio}x slower"
    fi
}

print_row ash_c
print_row go
print_row python
print_row java
print_row ash_i

echo ""

# ── Cleanup ──────────────────────────────────────────────────────────────────
rm -rf "$TMP_DIR"

echo -e "${BOLD}Done.${NC}"
echo ""
echo "Notes:"
echo "  - Timing includes process startup, which favors compiled binaries"
echo "  - Java timing is dominated by JVM startup (~100-150ms baseline)"
echo "  - Ash interpreted is slow on deep recursion; use ash build for compute"
echo "  - Token count benchmark: python3 benchmark/token_count.py"
echo ""
