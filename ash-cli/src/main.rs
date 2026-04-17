//! ash — the Ash language toolchain
//!
//! Usage:
//!   ash run   <file.ash>            Interpret and execute
//!   ash build <file.ash> [-o out]   Compile to native binary via LLVM IR
//!   ash check <file.ash>            Type-check only, no execution
//!   ash fmt   <file.ash>            Format source file
//!   ash docs  [namespace]           Show stdlib documentation
//!   ash test  <file.ash>            Run all test_* functions
//!   ash repl                        Start interactive REPL
//!   ash version                     Print version info

use std::path::{Path, PathBuf};
use std::process;

const VERSION: &str = "0.1.0";

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        print_usage();
        process::exit(1);
    }

    match args[1].as_str() {
        "run" => cmd_run(&args[2..]),
        "build" => cmd_build(&args[2..]),
        "check" => cmd_check(&args[2..]),
        "fmt" => cmd_fmt(&args[2..]),
        "docs" => cmd_docs(&args[2..]),
        "test" => cmd_test(&args[2..]),
        "repl" => cmd_repl(),
        "version" | "--version" | "-V" => cmd_version(),
        "help" | "--help" | "-h" => print_usage(),
        other => {
            eprintln!("ash: unknown command '{other}'");
            eprintln!("Run 'ash help' for usage.");
            process::exit(1);
        }
    }
}

fn print_usage() {
    println!("ash {VERSION} — the Ash language toolchain");
    println!();
    println!("USAGE:");
    println!("  ash run   <file.ash>             Interpret and run");
    println!("  ash build <file.ash> [-o <out>]  Compile to native binary");
    println!("  ash check <file.ash>             Type-check only");
    println!("  ash fmt   <file.ash>             Format source file");
    println!("  ash docs  [namespace]            Show stdlib docs");
    println!("  ash test  <file.ash>             Run test_* functions");
    println!("  ash repl                         Interactive REPL");
    println!("  ash version                      Show version");
    println!();
    println!("EXAMPLES:");
    println!("  ash run   hello.ash");
    println!("  ash build hello.ash -o hello");
    println!("  ash check hello.ash");
    println!("  ash fmt   hello.ash");
    println!("  ash docs  math");
    println!("  ash repl");
}

fn cmd_version() {
    println!("ash {VERSION}");
    println!("interpreter: tree-walking over AST");
    println!("compiler:    LLVM IR text emission");
}

// ─── run ─────────────────────────────────────────────────────────────────────

fn cmd_run(args: &[String]) {
    let path = require_file(args, "run");
    let source = read_source(&path);
    let program = frontend(&source, &path);
    match ash_interp::run(&program) {
        Ok(_) => {}
        Err(e) => {
            eprintln!("{}:runtime: {}", path.display(), e.msg);
            process::exit(1);
        }
    }
}

// ─── build ───────────────────────────────────────────────────────────────────

fn cmd_build(args: &[String]) {
    let path = require_file(args, "build");
    let source = read_source(&path);
    let program = frontend(&source, &path);
    let out_path = parse_output_flag(args).unwrap_or_else(|| path.with_extension(""));

    let ir = match ash_codegen::compile(&program) {
        Ok(ir) => ir,
        Err(e) => {
            eprintln!("{}:codegen: {}", path.display(), e.msg);
            process::exit(1);
        }
    };

    let ll_path = out_path.with_extension("ll");
    if let Err(e) = std::fs::write(&ll_path, &ir) {
        eprintln!("ash: failed to write {}: {e}", ll_path.display());
        process::exit(1);
    }

    match try_compile_with_clang(&ll_path, &out_path)
        .or_else(|_| try_compile_with_llc(&ll_path, &out_path))
    {
        Ok(()) => {
            println!("ash: built {} → {}", path.display(), out_path.display());
            let _ = std::fs::remove_file(&ll_path);
        }
        Err(_) => {
            println!("ash: LLVM IR written to {}", ll_path.display());
            println!(
                "ash: compile manually with: clang {} -o {}",
                ll_path.display(),
                out_path.display()
            );
        }
    }
}

fn try_compile_with_clang(ll: &Path, out: &Path) -> Result<(), String> {
    // Try clang-20 first (avoids FastISel bugs in clang-18 with certain phi patterns)
    for clang in &["clang-20", "clang-18", "clang"] {
        let result = process::Command::new(clang)
            .args([
                "-O1",
                ll.to_str().unwrap(),
                "-o",
                out.to_str().unwrap(),
                "-lm",
            ])
            .status();
        match result {
            Ok(status) if status.success() => return Ok(()),
            Ok(_) => continue,  // clang found but compilation failed
            Err(_) => continue, // clang not found
        }
    }
    Err("no working clang found".into())
}

fn try_compile_with_llc(ll: &Path, out: &Path) -> Result<(), String> {
    let asm = ll.with_extension("s");
    let s1 = process::Command::new("llc")
        .args([ll.to_str().unwrap(), "-o", asm.to_str().unwrap()])
        .status()
        .map_err(|e| e.to_string())?;
    if !s1.success() {
        return Err("llc failed".into());
    }
    let s2 = process::Command::new("cc")
        .args([asm.to_str().unwrap(), "-o", out.to_str().unwrap(), "-lm"])
        .status()
        .map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&asm);
    if s2.success() {
        Ok(())
    } else {
        Err("cc failed".into())
    }
}

// ─── check ───────────────────────────────────────────────────────────────────

fn cmd_check(args: &[String]) {
    let path = require_file(args, "check");
    let source = read_source(&path);
    let program = frontend(&source, &path);

    // Also run the type checker
    match ash_typeck::check(&program) {
        Ok(hir) => {
            let fn_count = hir.fns.len();
            let type_count = hir.types.len();
            let stmt_count = hir.top_stmts.len();
            println!(
                "ash: {} — OK ({fn_count} fn{}, {type_count} type{}, {stmt_count} stmt{})",
                path.display(),
                if fn_count == 1 { "" } else { "s" },
                if type_count == 1 { "" } else { "s" },
                if stmt_count == 1 { "" } else { "s" },
            );
        }
        Err(e) => {
            eprintln!("{}:{e}", path.display());
            process::exit(1);
        }
    }
}

// ─── fmt ─────────────────────────────────────────────────────────────────────

fn cmd_fmt(args: &[String]) {
    let path = require_file(args, "fmt");
    let source = read_source(&path);

    // Verify it parses first
    let _ = frontend(&source, &path);

    // Format: normalize indentation to 4 spaces, trim trailing whitespace,
    // ensure single blank line between top-level definitions,
    // ensure file ends with newline
    let formatted = format_source(&source);

    let in_place = !args.contains(&"--check".to_string());
    if in_place {
        if std::fs::write(&path, &formatted).is_err() {
            eprintln!("ash: cannot write to {}", path.display());
            process::exit(1);
        }
        println!("ash: formatted {}", path.display());
    } else {
        // --check mode: exit 1 if file would change
        if formatted != source {
            eprintln!("ash: {} is not formatted", path.display());
            process::exit(1);
        }
        println!("ash: {} is already formatted", path.display());
    }
}

fn format_source(src: &str) -> String {
    let mut out = Vec::new();
    let mut prev_blank = false;
    let mut in_top_level_def = false;

    for line in src.lines() {
        let trimmed = line.trim_end();

        // Detect top-level definitions (fn, type, let at col 0)
        let is_def = trimmed.starts_with("fn ")
            || trimmed.starts_with("type ")
            || trimmed.starts_with("let ")
            || trimmed.starts_with("mut ");

        // Insert blank line before top-level defs (except first)
        if is_def && !out.is_empty() && !prev_blank {
            out.push(String::new());
        }

        // Normalize indentation: tabs → 4 spaces
        let indent_spaces = line.len() - line.trim_start().len();
        let indent_tabs = line.chars().take_while(|c| *c == '\t').count();
        let effective_indent = indent_tabs * 4 + (indent_spaces - indent_tabs);
        let normalized = format!("{}{}", " ".repeat(effective_indent), trimmed.trim_start());

        prev_blank = trimmed.is_empty();
        out.push(normalized);
        in_top_level_def = is_def;
    }

    // Ensure trailing newline
    let mut result = out.join("\n");
    if !result.ends_with('\n') {
        result.push('\n');
    }
    result
}

// ─── docs ─────────────────────────────────────────────────────────────────────

fn cmd_docs(args: &[String]) {
    let namespace = args.first().map(|s| s.as_str()).unwrap_or("");

    let all = ash_stdlib::all_functions();

    let fns: Vec<_> = if namespace.is_empty() {
        all.iter().collect()
    } else {
        all.iter().filter(|f| f.namespace == namespace).collect()
    };

    if fns.is_empty() {
        if namespace.is_empty() {
            eprintln!("ash: no stdlib functions found");
        } else {
            eprintln!("ash: unknown namespace '{namespace}'");
            eprintln!("Available namespaces: (empty), math, file, http, json, re, env, go, db, cache, queue, auth, mail, store, ai");
        }
        process::exit(1);
    }

    // Group by namespace for display
    let mut current_ns = "";
    for f in &fns {
        if f.namespace != current_ns {
            current_ns = f.namespace;
            if current_ns.is_empty() {
                println!("\n--- core ---");
            } else {
                println!("\n--- {current_ns}.* ---");
            }
        }
        let params: Vec<String> = f
            .params
            .iter()
            .map(|(name, ty)| format!("{name}:{ty}"))
            .collect();
        println!(
            "  {:20} ({}) -> {}",
            f.full_name(),
            params.join(", "),
            f.ret
        );
        println!("    {}", f.doc);
    }
    println!();
}

// ─── test ─────────────────────────────────────────────────────────────────────

fn cmd_test(args: &[String]) {
    use ash_parser::ast::StmtKind;

    let path = require_file(args, "test");
    let source = read_source(&path);
    let program = frontend(&source, &path);

    // Collect names of all top-level test_* functions
    let test_names: Vec<String> = program
        .stmts
        .iter()
        .filter_map(|s| {
            if let StmtKind::FnDef(f) = &s.kind {
                if f.name.starts_with("test_") {
                    return Some(f.name.clone());
                }
            }
            None
        })
        .collect();

    if test_names.is_empty() {
        println!("ash test: no test_* functions found in {}", path.display());
        return;
    }

    println!(
        "ash test: running {} test(s) in {}",
        test_names.len(),
        path.display()
    );
    println!();

    // Load the program into an interpreter so all definitions are available
    let mut interp = ash_interp::Interpreter::new();
    if let Err(e) = interp.run_program(&program) {
        eprintln!("{}:runtime: {}", path.display(), e.msg);
        process::exit(1);
    }

    let mut passed = 0usize;
    let mut failed = 0usize;

    for name in &test_names {
        match interp.call_by_name(name) {
            Ok(_) => {
                println!("  \x1b[32mPASS\x1b[0m {name}");
                passed += 1;
            }
            Err(e) => {
                println!("  \x1b[31mFAIL\x1b[0m {name}");
                println!("       {}", e.msg);
                failed += 1;
            }
        }
    }

    println!();
    println!("ash test: {} passed, {} failed", passed, failed);

    if failed > 0 {
        process::exit(1);
    }
}

// ─── repl ─────────────────────────────────────────────────────────────────────

fn cmd_repl() {
    use std::io::{self, BufRead, Write};

    println!("ash {VERSION} REPL — type 'exit' or Ctrl-D to quit");
    println!("Multi-line: end line with '\\' to continue");
    println!();

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut interp = ash_interp::Interpreter::new();
    let mut buffer = String::new();
    let mut continuation = false;

    loop {
        // Print prompt
        {
            let mut out = stdout.lock();
            if continuation {
                write!(out, "... ").unwrap();
            } else {
                write!(out, "ash> ").unwrap();
            }
            out.flush().unwrap();
        }

        let mut line = String::new();
        match stdin.lock().read_line(&mut line) {
            Ok(0) => {
                // EOF
                println!();
                break;
            }
            Ok(_) => {}
            Err(e) => {
                eprintln!("ash: read error: {e}");
                break;
            }
        }

        let trimmed = line.trim_end();

        if trimmed == "exit" || trimmed == "quit" {
            break;
        }

        // Handle line continuation
        if trimmed.ends_with('\\') {
            buffer.push_str(&trimmed[..trimmed.len() - 1]);
            buffer.push('\n');
            continuation = true;
            continue;
        }

        buffer.push_str(trimmed);
        continuation = false;

        if buffer.trim().is_empty() {
            buffer.clear();
            continue;
        }

        // Lex + parse
        let tokens = match ash_lexer::Lexer::new(&buffer).tokenize() {
            Ok(t) => t,
            Err(e) => {
                eprintln!("  error: {e}");
                buffer.clear();
                continue;
            }
        };

        let program = match ash_parser::parse(tokens) {
            Ok(p) => p,
            Err(e) => {
                // If it looks like an incomplete expression, allow continuation
                if e.msg.contains("EOF") || e.msg.contains("Indent") {
                    buffer.push('\n');
                    continuation = true;
                    continue;
                }
                eprintln!("  error: {}", e.msg);
                buffer.clear();
                continue;
            }
        };

        // Execute
        match interp.run_program(&program) {
            Ok(val) => {
                if !matches!(val, ash_interp::Value::Unit) {
                    println!("  = {val}");
                }
            }
            Err(e) => {
                eprintln!("  error: {}", e.msg);
            }
        }

        buffer.clear();
    }

    println!("bye");
}

// ─── shared helpers ───────────────────────────────────────────────────────────

fn frontend(source: &str, path: &Path) -> ash_parser::ast::Program {
    let tokens = match ash_lexer::Lexer::new(source).tokenize() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("{}:{e}", path.display());
            process::exit(1);
        }
    };
    match ash_parser::parse(tokens) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{}:{e}", path.display());
            process::exit(1);
        }
    }
}

fn read_source(path: &Path) -> String {
    match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("ash: cannot read '{}': {e}", path.display());
            process::exit(1);
        }
    }
}

fn require_file(args: &[String], cmd: &str) -> PathBuf {
    let path_str = args
        .iter()
        .find(|a| !a.starts_with('-'))
        .unwrap_or_else(|| {
            eprintln!("ash: '{cmd}' requires a file argument");
            process::exit(1);
        });
    let p = PathBuf::from(path_str);
    if !p.exists() {
        eprintln!("ash: file '{}' not found", p.display());
        process::exit(1);
    }
    p
}

fn parse_output_flag(args: &[String]) -> Option<PathBuf> {
    let idx = args.iter().position(|a| a == "-o")?;
    args.get(idx + 1).map(PathBuf::from)
}
