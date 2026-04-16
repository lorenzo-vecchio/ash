#!/usr/bin/env python3
"""
token_count.py
Counts source tokens in equivalent todo CRUD backends written in
Ash, Python, Go, and Java — and reports how they compare.

What we measure: raw source tokens (identifiers, keywords, operators,
string literals, punctuation). Comments are excluded. Whitespace is not
a token. This is a close proxy for the number of tokens an LLM would
consume generating the same program.

Usage:
    python3 benchmark/token_count.py
"""

import re
import os
import textwrap

# ── Source programs ──────────────────────────────────────────────────────────

ASH_SRC = """\
# Todo backend — Ash

type Todo
    id:    int
    title: str
    done:  bool

mut todos   = []
mut next_id = 1

fn create(title)
    todos = todos.push(Todo { id: next_id, title: title, done: false })
    id = next_id
    next_id = next_id + 1
    "created {id}"

fn get_by_id(id)
    matches = todos.filter(t => t.id == id)
    if matches.len() > 0
        t = matches[0] ?? Todo { id: 0, title: "", done: false }
        "{t.id}: {t.title} [{t.done}]"
    else
        "not found: {id}"

fn complete(id)
    mut found = false
    mut new_todos = []
    for t in todos
        if t.id == id
            t.done = true
            found = true
        new_todos = new_todos.push(t)
    todos = new_todos
    if found
        "completed {id}"
    else
        "not found: {id}"

fn delete_todo(id)
    before = todos.len()
    todos  = todos.filter(t => t.id != id)
    if todos.len() < before
        "deleted {id}"
    else
        "not found: {id}"

fn list_all()
    for t in todos
        println("{t.id}: {t.title} [{t.done}]")
    "listed {todos.len()} todos"

fn list_pending()
    pending = todos.filter(t => t.done == false)
    for t in pending
        println("{t.id}: {t.title}")
    "{pending.len()} pending"

fn count_todos()
    total = todos.len()
    done  = todos.filter(t => t.done == true).len()
    pend  = total - done
    "total:{total} done:{done} pending:{pend}"

create("Buy groceries")
create("Write tests")
create("Deploy to production")
create("Review PR")
create("Update docs")
list_all()
complete(1)
complete(3)
list_pending()
delete_todo(2)
println(count_todos())
println(get_by_id(3))
println(get_by_id(99))
"""

PYTHON_SRC = """\
from dataclasses import dataclass
from typing import Optional

@dataclass
class Todo:
    id:    int
    title: str
    done:  bool = False

todos:   list[Todo] = []
next_id: int = 1

def create(title: str) -> str:
    global next_id
    todo = Todo(id=next_id, title=title)
    todos.append(todo)
    next_id += 1
    return f"created {todo.id}"

def get_by_id(id: int) -> str:
    match = next((t for t in todos if t.id == id), None)
    if match:
        return f"{match.id}: {match.title} [{match.done}]"
    return f"not found: {id}"

def complete(id: int) -> str:
    for t in todos:
        if t.id == id:
            t.done = True
            return f"completed {id}"
    return f"not found: {id}"

def delete(id: int) -> str:
    global todos
    before = len(todos)
    todos = [t for t in todos if t.id != id]
    if len(todos) < before:
        return f"deleted {id}"
    return f"not found: {id}"

def list_all() -> str:
    for t in todos:
        print(f"{t.id}: {t.title} [{t.done}]")
    return f"listed {len(todos)} todos"

def list_pending() -> str:
    pending = [t for t in todos if not t.done]
    for t in pending:
        print(f"{t.id}: {t.title}")
    return f"{len(pending)} pending"

def count() -> str:
    total   = len(todos)
    done    = sum(1 for t in todos if t.done)
    pending = total - done
    return f"total:{total} done:{done} pending:{pending}"

create("Buy groceries")
create("Write tests")
create("Deploy to production")
create("Review PR")
create("Update docs")
list_all()
complete(1)
complete(3)
list_pending()
delete(2)
print(count())
print(get_by_id(3))
print(get_by_id(99))
"""

GO_SRC = """\
package main

import "fmt"

type Todo struct {
    ID    int
    Title string
    Done  bool
}

var todos  []Todo
var nextID = 1

func create(title string) string {
    todo := Todo{ID: nextID, Title: title, Done: false}
    todos = append(todos, todo)
    nextID++
    return fmt.Sprintf("created %d", todo.ID)
}

func getByID(id int) string {
    for _, t := range todos {
        if t.ID == id {
            return fmt.Sprintf("%d: %s [%v]", t.ID, t.Title, t.Done)
        }
    }
    return fmt.Sprintf("not found: %d", id)
}

func complete(id int) string {
    for i := range todos {
        if todos[i].ID == id {
            todos[i].Done = true
            return fmt.Sprintf("completed %d", id)
        }
    }
    return fmt.Sprintf("not found: %d", id)
}

func deleteTodo(id int) string {
    before   := len(todos)
    filtered := todos[:0]
    for _, t := range todos {
        if t.ID != id {
            filtered = append(filtered, t)
        }
    }
    todos = filtered
    if len(todos) < before {
        return fmt.Sprintf("deleted %d", id)
    }
    return fmt.Sprintf("not found: %d", id)
}

func listAll() string {
    for _, t := range todos {
        fmt.Printf("%d: %s [%v]\\n", t.ID, t.Title, t.Done)
    }
    return fmt.Sprintf("listed %d todos", len(todos))
}

func listPending() string {
    count := 0
    for _, t := range todos {
        if !t.Done {
            fmt.Printf("%d: %s\\n", t.ID, t.Title)
            count++
        }
    }
    return fmt.Sprintf("%d pending", count)
}

func count() string {
    total, done := len(todos), 0
    for _, t := range todos {
        if t.Done { done++ }
    }
    return fmt.Sprintf("total:%d done:%d pending:%d", total, done, total-done)
}

func main() {
    create("Buy groceries")
    create("Write tests")
    create("Deploy to production")
    create("Review PR")
    create("Update docs")
    listAll()
    complete(1)
    complete(3)
    listPending()
    deleteTodo(2)
    fmt.Println(count())
    fmt.Println(getByID(3))
    fmt.Println(getByID(99))
}
"""

JAVA_SRC = """\
import java.util.ArrayList;
import java.util.List;
import java.util.Optional;
import java.util.stream.Collectors;

public class todo {

    static class Todo {
        int     id;
        String  title;
        boolean done;
        Todo(int id, String title) {
            this.id    = id;
            this.title = title;
            this.done  = false;
        }
        public String toString() {
            return id + ": " + title + " [" + done + "]";
        }
    }

    static List<Todo> todos  = new ArrayList<>();
    static int        nextId = 1;

    static String create(String title) {
        Todo todo = new Todo(nextId++, title);
        todos.add(todo);
        return "created " + todo.id;
    }

    static String getById(int id) {
        Optional<Todo> match = todos.stream()
            .filter(t -> t.id == id).findFirst();
        return match.map(t -> t.id + ": " + t.title + " [" + t.done + "]")
                    .orElse("not found: " + id);
    }

    static String complete(int id) {
        for (Todo t : todos) {
            if (t.id == id) { t.done = true; return "completed " + id; }
        }
        return "not found: " + id;
    }

    static String delete(int id) {
        int before = todos.size();
        todos.removeIf(t -> t.id == id);
        if (todos.size() < before) return "deleted " + id;
        return "not found: " + id;
    }

    static String listAll() {
        todos.forEach(System.out::println);
        return "listed " + todos.size() + " todos";
    }

    static String listPending() {
        List<Todo> pending = todos.stream()
            .filter(t -> !t.done).collect(Collectors.toList());
        pending.forEach(t -> System.out.println(t.id + ": " + t.title));
        return pending.size() + " pending";
    }

    static String count() {
        long done = todos.stream().filter(t -> t.done).count();
        return "total:" + todos.size() + " done:" + done +
               " pending:" + (todos.size() - done);
    }

    public static void main(String[] args) {
        create("Buy groceries");
        create("Write tests");
        create("Deploy to production");
        create("Review PR");
        create("Update docs");
        listAll();
        complete(1);
        complete(3);
        listPending();
        delete(2);
        System.out.println(count());
        System.out.println(getById(3));
        System.out.println(getById(99));
    }
}
"""

# ── Tokenizer ────────────────────────────────────────────────────────────────

def strip_comments(src: str, style: str) -> str:
    """Remove comments based on language style."""
    if style == "hash":
        # Remove # comments (Ash, Python)
        src = re.sub(r'#.*$', '', src, flags=re.MULTILINE)
    elif style == "slash":
        # Remove // and /* */ comments (Go, Java)
        src = re.sub(r'//.*$', '', src, flags=re.MULTILINE)
        src = re.sub(r'/\*.*?\*/', '', src, flags=re.DOTALL)
    return src

def count_tokens(src: str, comment_style: str) -> dict:
    """Count tokens and return statistics."""
    src = strip_comments(src, comment_style)

    # Tokenise: identifiers/keywords, numbers, string literals, operators/punct
    tokens = re.findall(
        r'"(?:[^"\\]|\\.)*"'     # double-quoted strings
        r"|'(?:[^'\\]|\\.)*'"    # single-quoted strings
        r'|[a-zA-Z_]\w*'         # identifiers and keywords
        r'|[0-9]+(?:\.[0-9]+)?'  # numbers
        r'|[^\s\w]',             # any single non-whitespace non-word char
        src
    )

    # Count non-blank lines
    lines = [l for l in src.split('\n') if l.strip()]

    # Count raw characters (no whitespace)
    chars = len(re.sub(r'\s+', ' ', src).strip())

    return {
        "tokens": len(tokens),
        "lines":  len(lines),
        "chars":  chars,
        "token_list": tokens,
    }

# ── Run ──────────────────────────────────────────────────────────────────────

def main():
    programs = [
        ("Ash",    ASH_SRC,    "hash"),
        ("Python", PYTHON_SRC, "hash"),
        ("Go",     GO_SRC,     "slash"),
        ("Java",   JAVA_SRC,   "slash"),
    ]

    results = []
    for name, src, style in programs:
        stats = count_tokens(src, style)
        results.append((name, stats))

    # Baseline is Python
    baseline_tokens = next(s["tokens"] for n, s in results if n == "Python")

    # ── Summary table ────────────────────────────────────────────────────────
    print()
    print("Ash Language — Token Count Benchmark")
    print("By AI, for AI")
    print()
    print("Program: identical todo CRUD backend (create/read/update/delete/list/filter)")
    print("Metric:  source tokens with comments excluded")
    print()
    print(f"{'Language':<12} {'Tokens':>8} {'Lines':>7} {'Chars':>8} {'vs Python':>12}")
    print("-" * 52)

    for name, stats in results:
        t = stats["tokens"]
        l = stats["lines"]
        c = stats["chars"]
        pct = (t - baseline_tokens) / baseline_tokens * 100
        sign = "+" if pct > 0 else ""
        pct_str = f"{sign}{pct:.1f}%"
        marker = " <-- BASELINE" if name == "Python" else ""
        print(f"{name:<12} {t:>8} {l:>7} {c:>8} {pct_str:>12}{marker}")

    print()

    # ── Detailed breakdown ───────────────────────────────────────────────────
    print("Top token categories (Ash vs Python)")
    print()

    ash_tokens    = results[0][1]["token_list"]
    python_tokens = results[1][1]["token_list"]

    def categorize(tokens):
        keywords  = set(["fn","let","mut","if","else","return","while","for",
                          "in","match","type","true","false","move","await",
                          # Python
                          "def","class","import","from","global","pass",
                          "lambda","yield","with","as","and","or","not",
                          "is","in","None","True","False","self",
                          # Go
                          "func","var","package","struct","interface",
                          "range","go","defer","make","append","len",
                          # Java
                          "public","private","static","void","class","new",
                          "return","import","extends","implements",
                          "this","super","null","instanceof"])
        kw    = sum(1 for t in tokens if t in keywords)
        ident = sum(1 for t in tokens if re.match(r'^[a-zA-Z_]\w*$', t) and t not in keywords)
        ops   = sum(1 for t in tokens if re.match(r'^[^\w\s"\']+$', t))
        strs  = sum(1 for t in tokens if t.startswith('"') or t.startswith("'"))
        nums  = sum(1 for t in tokens if re.match(r'^\d', t))
        return {"keywords": kw, "identifiers": ident, "operators": ops,
                "strings": strs, "numbers": nums}

    ash_cats = categorize(ash_tokens)
    py_cats  = categorize(python_tokens)

    print(f"{'Category':<15} {'Ash':>8} {'Python':>8} {'Difference':>12}")
    print("-" * 46)
    for cat in ["keywords", "identifiers", "operators", "strings", "numbers"]:
        a, p = ash_cats[cat], py_cats[cat]
        diff = a - p
        sign = "+" if diff > 0 else ""
        print(f"{cat:<15} {a:>8} {p:>8} {sign+str(diff):>12}")

    print()

    # ── Projection ───────────────────────────────────────────────────────────
    ash_t = results[0][1]["tokens"]
    py_t  = results[1][1]["tokens"]
    go_t  = results[2][1]["tokens"]
    ja_t  = results[3][1]["tokens"]

    print("Token savings at scale")
    print()
    for n_calls in [1_000, 100_000, 10_000_000]:
        ash_cost  = ash_t * n_calls
        py_cost   = py_t  * n_calls
        go_cost   = go_t  * n_calls
        java_cost = ja_t  * n_calls
        saved_vs_py   = py_cost   - ash_cost
        saved_vs_java = java_cost - ash_cost
        print(f"  At {n_calls:>12,} generations:")
        print(f"    Ash saves {saved_vs_py:>14,} tokens vs Python")
        print(f"    Ash saves {saved_vs_java:>14,} tokens vs Java")
        # Rough cost estimate: $0.30 per 1M output tokens (GPT-4o pricing)
        cost_saved_usd = saved_vs_java / 1_000_000 * 0.30
        print(f"    Estimated cost saving vs Java: ${cost_saved_usd:,.2f}")
        print()

    print("Note: token counts vary by tokenizer. These use a simple")
    print("regex-based counter. LLM tokenizers (BPE) produce different")
    print("absolute numbers but the relative ratios hold.")
    print()

if __name__ == "__main__":
    main()
