//! ash-codegen — emits LLVM IR from a typed HIR program.
//! Types come from ash-typeck so every value has a resolved HirType.

use ash_hir::*;
use ash_parser::ast::Program;
use std::collections::HashMap;

// ─── Error ────────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct CodegenError {
    pub msg: String,
}
impl CodegenError {
    fn new(msg: impl Into<String>) -> Self {
        CodegenError { msg: msg.into() }
    }
}
impl std::fmt::Display for CodegenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "error[codegen]: {}", self.msg)
    }
}
type CResult<T> = Result<T, CodegenError>;

// ─── LLVM type ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
enum LLVMType {
    I1,
    I64,
    Double,
    Ptr,
    Void,
}

impl std::fmt::Display for LLVMType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LLVMType::I1 => write!(f, "i1"),
            LLVMType::I64 => write!(f, "i64"),
            LLVMType::Double => write!(f, "double"),
            LLVMType::Ptr => write!(f, "i8*"),
            LLVMType::Void => write!(f, "void"),
        }
    }
}

fn hir_to_llvm(ty: &HirType) -> LLVMType {
    match ty {
        HirType::Int => LLVMType::I64,
        HirType::Float => LLVMType::Double,
        // Bool uses I64 at ABI boundaries (function params/returns) to avoid
        // FastISel bugs in clang 18 with i1 phi nodes. Internal storage uses i1.
        HirType::Bool => LLVMType::I64,
        HirType::Str => LLVMType::Ptr,
        HirType::Void => LLVMType::Void,
        HirType::Unknown => LLVMType::I64,
        _ => LLVMType::Ptr,
    }
}

/// Storage type — used for alloca slots. Booleans can stay as i1 in memory.
fn hir_to_llvm_storage(ty: &HirType) -> LLVMType {
    match ty {
        HirType::Bool => LLVMType::I1,
        other => hir_to_llvm(other),
    }
}

// ─── Codegen ──────────────────────────────────────────────────────────────────

struct Codegen {
    output: Vec<String>,
    str_consts: Vec<(String, String)>,
    reg: usize,
    lbl: usize,
    vars: Vec<HashMap<String, (String, LLVMType)>>,
    cur_ret: LLVMType,
    fn_sigs: HashMap<String, (Vec<LLVMType>, LLVMType)>,
    /// Name of the current basic block being emitted — used for phi predecessor tracking
    cur_block: String,
}

impl Codegen {
    fn new() -> Self {
        Codegen {
            output: vec![],
            str_consts: vec![],
            reg: 0,
            lbl: 0,
            vars: vec![HashMap::new()],
            cur_ret: LLVMType::Void,
            fn_sigs: HashMap::new(),
            cur_block: "entry".into(),
        }
    }
    fn r(&mut self) -> String {
        self.reg += 1;
        format!("%r{}", self.reg)
    }
    fn l(&mut self) -> String {
        self.lbl += 1;
        format!("L{}", self.lbl)
    }
    fn emit(&mut self, s: impl Into<String>) {
        let s = s.into();
        // Track current block: lines ending with ':' that aren't inside instructions
        if s.ends_with(':') && !s.starts_with(' ') {
            self.cur_block = s.trim_end_matches(':').to_string();
        }
        self.output.push(s);
    }
    fn i(&mut self, s: impl Into<String>) {
        self.output.push(format!("  {}", s.into()));
    }
    fn push_scope(&mut self) {
        self.vars.push(HashMap::new());
    }
    fn pop_scope(&mut self) {
        self.vars.pop();
    }
    fn def(&mut self, name: &str, reg: &str, ty: LLVMType) {
        self.vars
            .last_mut()
            .unwrap()
            .insert(name.to_string(), (reg.to_string(), ty));
    }
    fn get_var(&self, name: &str) -> Option<&(String, LLVMType)> {
        for s in self.vars.iter().rev() {
            if let Some(v) = s.get(name) {
                return Some(v);
            }
        }
        None
    }
    fn intern(&mut self, s: &str) -> String {
        for (l, c) in &self.str_consts {
            if c == s {
                return l.clone();
            }
        }
        let l = format!("@.str{}", self.str_consts.len());
        self.str_consts.push((l.clone(), s.to_string()));
        l
    }

    // Coerce a value to a target type, emitting a conversion instruction if needed
    fn coerce(&mut self, reg: &str, from: &LLVMType, to: &LLVMType) -> String {
        if from == to {
            return reg.to_string();
        }
        let out = self.r();
        match (from, to) {
            (LLVMType::I1, LLVMType::I64) => self.i(format!("{out} = zext i1 {reg} to i64")),
            (LLVMType::I64, LLVMType::I1) => self.i(format!("{out} = icmp ne i64 {reg}, 0")),
            (LLVMType::I64, LLVMType::Double) => {
                self.i(format!("{out} = sitofp i64 {reg} to double"))
            }
            (LLVMType::Double, LLVMType::I64) => {
                self.i(format!("{out} = fptosi double {reg} to i64"))
            }
            _ => return reg.to_string(), // can't coerce, return as-is
        }
        out
    }

    // ── compile entry ─────────────────────────────────────────────────────────

    pub fn compile(mut self, hir: &HirProgram) -> CResult<String> {
        self.emit("; Ash compiled output");
        // Standard C / LLVM intrinsics
        self.emit("declare i32 @printf(i8* nocapture, ...)");
        self.emit("declare i8* @malloc(i64)");
        self.emit("declare i64 @strlen(i8*)");
        self.emit("declare void @exit(i32)");
        self.emit("declare double @llvm.fabs.f64(double)");
        // ash_runtime.c helpers — %AshList* is represented as i8* at LLVM IR level
        self.emit("declare i8* @ash_list_new()");
        self.emit("declare void @ash_list_push(i8*, i64)");
        self.emit("declare i64 @ash_list_get(i8*, i64)");
        self.emit("declare i64 @ash_list_len(i8*)");
        self.emit("declare i8* @ash_str_concat(i8*, i8*)");
        self.emit("declare i8* @ash_str_from_int(i64)");
        self.emit("declare i8* @ash_str_from_float(double)");
        self.emit("declare i8* @ash_str_from_bool(i64)");
        self.emit("");

        // Pre-register all function signatures so call sites know return types
        for f in hir.fns.iter().chain(hir.lifted.iter()) {
            let param_tys: Vec<LLVMType> = f.params.iter().map(|p| hir_to_llvm(&p.ty)).collect();
            let ret_ty = hir_to_llvm(&f.ret);
            self.fn_sigs.insert(f.name.clone(), (param_tys, ret_ty));
        }

        // Emit all lifted lambdas first
        for f in &hir.lifted {
            self.emit_fn(f)?;
        }
        // Emit user functions
        for f in &hir.fns {
            self.emit_fn(f)?;
        }
        // Emit main wrapping top-level statements
        self.emit_main(&hir.top_stmts)?;

        // Prepend string constants
        let mut header = vec![];
        for (lbl, content) in &self.str_consts {
            let esc = content
                .replace('\\', "\\\\")
                .replace('\n', "\\0A")
                .replace('\t', "\\09")
                .replace('"', "\\22");
            let len = content.len() + 1;
            header.push(format!(
                "{lbl} = private unnamed_addr constant [{len} x i8] c\"{esc}\\00\""
            ));
        }
        header.push(String::new());
        let mut all = header;
        all.extend(self.output);
        Ok(all.join("\n"))
    }

    // ── function ──────────────────────────────────────────────────────────────

    fn emit_fn(&mut self, f: &HirFn) -> CResult<()> {
        let declared_ret = hir_to_llvm(&f.ret);
        // Use Unknown as a sentinel — will be resolved after emitting the body
        self.cur_ret = declared_ret.clone();

        let params: Vec<String> = f
            .params
            .iter()
            .map(|p| format!("{} %p_{}", hir_to_llvm(&p.ty), p.name))
            .collect();

        // Reserve a slot for the define line — we'll patch it once we know the
        // actual return type (needed when f.ret == Unknown / string functions).
        let define_idx = self.output.len();
        self.output.push(String::new()); // placeholder
        self.emit("entry:");
        self.cur_block = "entry".into();
        self.push_scope();

        for p in &f.params {
            let ty = hir_to_llvm(&p.ty);
            let slot = format!("%slot_{}", p.name);
            self.i(format!("{slot} = alloca {ty}"));
            self.i(format!("store {ty} %p_{}, {ty}* {slot}", p.name));
            self.def(&p.name, &slot, ty);
        }

        let mut last: Option<(String, LLVMType)> = None;
        for stmt in &f.body.stmts {
            last = self.emit_stmt(stmt)?;
        }

        // Resolve the actual return type: when declared Unknown, use the type
        // of the last expression (e.g. Ptr for string-returning functions).
        let resolved_ret = if declared_ret == LLVMType::I64 && f.ret == HirType::Unknown {
            last.as_ref()
                .map(|(_, t)| t.clone())
                .unwrap_or(declared_ret.clone())
        } else {
            declared_ret.clone()
        };

        // Patch the define line now that we know the resolved return type
        self.output[define_idx] = format!(
            "define {} @{}({}) {{",
            resolved_ret,
            f.name,
            params.join(", ")
        );

        // Update fn_sig with resolved return type (for callers that use println etc.)
        if let Some(sig) = self.fn_sigs.get_mut(&f.name) {
            sig.1 = resolved_ret.clone();
        }

        // Emit return
        if resolved_ret == LLVMType::Void {
            self.i("ret void");
        } else if let Some((reg, ty)) = last {
            let coerced = self.coerce(&reg, &ty, &resolved_ret);
            self.i(format!("ret {resolved_ret} {coerced}"));
        } else {
            match &resolved_ret {
                LLVMType::I64 | LLVMType::I1 => self.i(format!("ret {resolved_ret} 0")),
                LLVMType::Double => self.i("ret double 0.0"),
                _ => self.i(format!("ret {resolved_ret} null")),
            }
        }

        self.pop_scope();
        self.emit("}");
        self.emit("");
        Ok(())
    }

    fn emit_main(&mut self, stmts: &[HirStmt]) -> CResult<()> {
        self.emit("define i32 @main() {");
        self.emit("entry:");
        self.cur_ret = LLVMType::I64;
        self.push_scope();
        for s in stmts {
            self.emit_stmt(s)?;
        }
        self.pop_scope();
        self.i("ret i32 0");
        self.emit("}");
        self.emit("");
        Ok(())
    }

    // ── statements ────────────────────────────────────────────────────────────

    fn emit_stmt(&mut self, stmt: &HirStmt) -> CResult<Option<(String, LLVMType)>> {
        match &stmt.kind {
            HirStmtKind::Let {
                name, ty, value, ..
            } => {
                let (reg, vty) = self.emit_expr(value)?;
                // Use storage type for alloca (i1 for bool), ABI type for the def
                let _abi_ty = if *ty == HirType::Unknown {
                    vty.clone()
                } else {
                    hir_to_llvm(ty)
                };
                let store_ty = if *ty == HirType::Unknown {
                    vty.clone()
                } else {
                    hir_to_llvm_storage(ty)
                };
                let slot = format!("%slot_{name}");
                self.i(format!("{slot} = alloca {store_ty}"));
                let coerced = self.coerce(&reg, &vty, &store_ty);
                self.i(format!("store {store_ty} {coerced}, {store_ty}* {slot}"));
                // Track with storage type so loads use the right type
                self.def(name, &slot, store_ty);
                Ok(None)
            }
            HirStmtKind::Assign { target, value } => {
                let (reg, vty) = self.emit_expr(value)?;
                if let HirExprKind::Var(name) = &target.kind {
                    if let Some((slot, slot_ty)) = self.get_var(name).cloned() {
                        let coerced = self.coerce(&reg, &vty, &slot_ty);
                        self.i(format!("store {slot_ty} {coerced}, {slot_ty}* {slot}"));
                    } else {
                        // Implicit let
                        let slot = format!("%slot_{name}");
                        self.i(format!("{slot} = alloca {vty}"));
                        self.i(format!("store {vty} {reg}, {vty}* {slot}"));
                        self.def(name, &slot, vty);
                    }
                }
                Ok(None)
            }
            HirStmtKind::Return(expr) => {
                if let Some(e) = expr {
                    let (reg, ty) = self.emit_expr(e)?;
                    let coerced = self.coerce(&reg, &ty, &self.cur_ret.clone());
                    self.i(format!("ret {} {coerced}", self.cur_ret));
                } else {
                    self.i("ret void");
                }
                Ok(None)
            }
            HirStmtKind::Expr(expr) => {
                let (reg, ty) = self.emit_expr(expr)?;
                Ok(Some((reg, ty)))
            }
            HirStmtKind::While { cond, body } => {
                let cl = self.l();
                let bl = self.l();
                let al = self.l();
                self.i(format!("br label %{cl}"));
                self.emit(format!("{cl}:"));
                let (cr, cty) = self.emit_expr(cond)?;
                let cond_i1 = self.coerce(&cr, &cty, &LLVMType::I1);
                self.i(format!("br i1 {cond_i1}, label %{bl}, label %{al}"));
                self.emit(format!("{bl}:"));
                self.push_scope();
                for s in &body.stmts {
                    self.emit_stmt(s)?;
                }
                self.pop_scope();
                self.i(format!("br label %{cl}"));
                self.emit(format!("{al}:"));
                Ok(None)
            }
            HirStmtKind::For {
                var,
                var_ty,
                iter,
                body,
            } => {
                // Simplified: only handles list iteration via range
                let (iter_reg, _) = self.emit_expr(iter)?;
                let idx = self.r();
                let cl = self.l();
                let bl = self.l();
                let al = self.l();
                self.i(format!("{idx} = alloca i64"));
                self.i(format!("store i64 0, i64* {idx}"));
                self.i(format!("br label %{cl}"));
                self.emit(format!("{cl}:"));
                let cur = self.r();
                self.i(format!("{cur} = load i64, i64* {idx}"));
                let cmp = self.r();
                self.i(format!("{cmp} = icmp slt i64 {cur}, {iter_reg}"));
                self.i(format!("br i1 {cmp}, label %{bl}, label %{al}"));
                self.emit(format!("{bl}:"));
                self.push_scope();
                let vs = format!("%slot_{var}");
                let vty = hir_to_llvm(var_ty);
                self.i(format!("{vs} = alloca {vty}"));
                let cv = self.r();
                self.i(format!("{cv} = load i64, i64* {idx}"));
                self.i(format!("store i64 {cv}, i64* {vs}"));
                self.def(var, &vs, vty);
                for s in &body.stmts {
                    self.emit_stmt(s)?;
                }
                self.pop_scope();
                let nxt = self.r();
                let cv2 = self.r();
                self.i(format!("{cv2} = load i64, i64* {idx}"));
                self.i(format!("{nxt} = add i64 {cv2}, 1"));
                self.i(format!("store i64 {nxt}, i64* {idx}"));
                self.i(format!("br label %{cl}"));
                self.emit(format!("{al}:"));
                Ok(None)
            }
            HirStmtKind::Panic(msg) => {
                let fmt = self.intern("panic: %s\n");
                let (mr, _) = self.emit_expr(msg)?;
                let flen = self
                    .str_consts
                    .iter()
                    .find(|(l, _)| l == &fmt)
                    .map(|(_, c)| c.len() + 1)
                    .unwrap_or(12);
                let fp = self.r();
                self.i(format!(
                    "{fp} = getelementptr [{flen} x i8], [{flen} x i8]* {fmt}, i64 0, i64 0"
                ));
                self.i(format!("call i32 (i8*, ...) @printf(i8* {fp}, i8* {mr})"));
                self.i("call void @exit(i32 1)");
                self.i("unreachable");
                Ok(None)
            }
        }
    }

    // ── expressions ───────────────────────────────────────────────────────────

    fn emit_expr(&mut self, expr: &HirExpr) -> CResult<(String, LLVMType)> {
        let expr_ty = hir_to_llvm(&expr.ty);
        match &expr.kind {
            HirExprKind::Int(n) => Ok((n.to_string(), LLVMType::I64)),
            HirExprKind::Float(f) => Ok((format!("{f:.17}"), LLVMType::Double)),
            HirExprKind::Bool(b) => Ok((if *b { "1" } else { "0" }.to_string(), LLVMType::I1)),
            HirExprKind::Str(s) => {
                let lbl = self.intern(s);
                let len = s.len() + 1;
                let r = self.r();
                self.i(format!(
                    "{r} = getelementptr [{len} x i8], [{len} x i8]* {lbl}, i64 0, i64 0"
                ));
                Ok((r, LLVMType::Ptr))
            }
            HirExprKind::Var(name) => {
                if let Some((slot, ty)) = self.get_var(name).cloned() {
                    let r = self.r();
                    self.i(format!("{r} = load {ty}, {ty}* {slot}"));
                    Ok((r, ty))
                } else {
                    Err(CodegenError::new(format!("undefined variable '{name}'")))
                }
            }
            HirExprKind::BinOp { op, lhs, rhs } => self.emit_binop(op, lhs, rhs),
            HirExprKind::UnOp { op, expr } => {
                let (r, ty) = self.emit_expr(expr)?;
                let out = self.r();
                match op {
                    HirUnOp::Neg => match ty {
                        LLVMType::I64 => self.i(format!("{out} = sub i64 0, {r}")),
                        LLVMType::Double => self.i(format!("{out} = fneg double {r}")),
                        _ => return Err(CodegenError::new("negation requires number")),
                    },
                    HirUnOp::Not => {
                        let as_i1 = self.coerce(&r, &ty, &LLVMType::I1);
                        self.i(format!("{out} = xor i1 {as_i1}, 1"));
                    }
                }
                Ok((out, ty))
            }
            HirExprKind::Call { callee, args } => {
                // Handle field calls (method calls)
                if let HirExprKind::Field { obj, field } = &callee.kind {
                    // Try namespace call: math.sqrt etc
                    if let HirExprKind::Var(ns) = &obj.kind {
                        if ns.as_str() == "math" {
                            return self.emit_math_call(field, args);
                        }
                    }
                    // Method call on value
                    let (obj_reg, obj_ty) = self.emit_expr(obj)?;
                    return self.emit_method_call(&obj_reg, &obj_ty, field, args);
                }
                // Named function call
                if let HirExprKind::Var(name) = &callee.kind {
                    return self.emit_named_call(name, args, &expr_ty);
                }
                Err(CodegenError::new("indirect calls not supported in codegen"))
            }
            HirExprKind::If { cond, then, else_ } => {
                self.emit_if(cond, then, else_.as_deref(), &expr_ty)
            }
            HirExprKind::Block(block) => {
                let mut last = ("0".into(), LLVMType::I64);
                self.push_scope();
                for s in &block.stmts {
                    if let Some(v) = self.emit_stmt(s)? {
                        last = v;
                    }
                }
                self.pop_scope();
                Ok(last)
            }
            HirExprKind::UnwrapOr { val, default } => {
                // Simplified null coalescing: emit val, if zero emit default
                let (vr, vty) = self.emit_expr(val)?;
                let (dr, _) = self.emit_expr(default)?;
                let cmp = self.r();
                let out = self.r();
                let tl = self.l();
                let fl = self.l();
                let ml = self.l();
                self.i(format!("{cmp} = icmp ne {vty} {vr}, 0"));
                self.i(format!("br i1 {cmp}, label %{tl}, label %{fl}"));
                self.emit(format!("{tl}:"));
                self.i(format!("br label %{ml}"));
                self.emit(format!("{fl}:"));
                self.i(format!("br label %{ml}"));
                self.emit(format!("{ml}:"));
                self.i(format!(
                    "{out} = phi {vty} [ {vr}, %{tl} ], [ {dr}, %{fl} ]"
                ));
                Ok((out, vty))
            }
            HirExprKind::Match { scrutinee, arms } => self.emit_match(scrutinee, arms, &expr_ty),
            HirExprKind::Await(e) => self.emit_expr(e),
            HirExprKind::Closure { fn_id, .. } => {
                // Function pointer — for compiled mode lambdas are lifted to named fns
                let out = self.r();
                self.i(format!("{out} = bitcast i64 (i64)* @{fn_id} to i8*"));
                Ok((out, LLVMType::Ptr))
            }
            HirExprKind::PropagateErr(e) => self.emit_expr(e),
            HirExprKind::SafeField { obj, field } => {
                let (reg, ty) = self.emit_expr(obj)?;
                self.emit_method_call(&reg, &ty, field, &[])
            }
            HirExprKind::Field { obj, field } => {
                let (reg, ty) = self.emit_expr(obj)?;
                self.emit_method_call(&reg, &ty, field, &[])
            }
            HirExprKind::List(items) => {
                // Allocate a new AshList and push each element
                let list_reg = self.r();
                self.i(format!("{list_reg} = call i8* @ash_list_new()"));
                for item in items {
                    let (elem_reg, elem_ty) = self.emit_expr(item)?;
                    // Coerce to i64 for storage in the list
                    let stored = self.coerce(&elem_reg, &elem_ty, &LLVMType::I64);
                    self.i(format!(
                        "call void @ash_list_push(i8* {list_reg}, i64 {stored})"
                    ));
                }
                Ok((list_reg, LLVMType::Ptr))
            }

            HirExprKind::Index { obj, index } => {
                let (obj_reg, _obj_ty) = self.emit_expr(obj)?;
                let (idx_reg, idx_ty) = self.emit_expr(index)?;
                let idx = self.coerce(&idx_reg, &idx_ty, &LLVMType::I64);
                let out = self.r();
                self.i(format!(
                    "{out} = call i64 @ash_list_get(i8* {obj_reg}, i64 {idx})"
                ));
                Ok((out, LLVMType::I64))
            }

            HirExprKind::Map(_) | HirExprKind::Tuple(_) => Err(CodegenError::new(
                "map/tuple literals not yet supported in compiled mode — use ash run",
            )),
        }
    }

    // ── binary ops ────────────────────────────────────────────────────────────

    fn emit_binop(
        &mut self,
        op: &HirBinOp,
        lhs: &HirExpr,
        rhs: &HirExpr,
    ) -> CResult<(String, LLVMType)> {
        let (l, lt) = self.emit_expr(lhs)?;
        let (r, rt) = self.emit_expr(rhs)?;

        // Unify types — promote to float if either side is float
        let (l, r, ty) = if lt == LLVMType::Double || rt == LLVMType::Double {
            let l = self.coerce(&l, &lt, &LLVMType::Double);
            let r = self.coerce(&r, &rt, &LLVMType::Double);
            (l, r, LLVMType::Double)
        } else if lt == LLVMType::I1 && rt == LLVMType::I1 {
            (l, r, LLVMType::I1)
        } else {
            // Coerce both to i64
            let l = self.coerce(&l, &lt, &LLVMType::I64);
            let r = self.coerce(&r, &rt, &LLVMType::I64);
            (l, r, LLVMType::I64)
        };

        let out = self.r();
        let fl = ty == LLVMType::Double;

        match op {
            HirBinOp::Add => {
                if fl {
                    self.i(format!("{out} = fadd double {l}, {r}"));
                } else {
                    self.i(format!("{out} = add i64 {l}, {r}"));
                }
                Ok((out, ty))
            }
            HirBinOp::Sub => {
                if fl {
                    self.i(format!("{out} = fsub double {l}, {r}"));
                } else {
                    self.i(format!("{out} = sub i64 {l}, {r}"));
                }
                Ok((out, ty))
            }
            HirBinOp::Mul => {
                if fl {
                    self.i(format!("{out} = fmul double {l}, {r}"));
                } else {
                    self.i(format!("{out} = mul i64 {l}, {r}"));
                }
                Ok((out, ty))
            }
            HirBinOp::Div => {
                if fl {
                    self.i(format!("{out} = fdiv double {l}, {r}"));
                } else {
                    self.i(format!("{out} = sdiv i64 {l}, {r}"));
                }
                Ok((out, ty))
            }
            HirBinOp::Mod => {
                self.i(format!("{out} = srem i64 {l}, {r}"));
                Ok((out, LLVMType::I64))
            }
            HirBinOp::Eq => {
                if fl {
                    self.i(format!("{out} = fcmp oeq double {l}, {r}"));
                } else {
                    self.i(format!("{out} = icmp eq i64 {l}, {r}"));
                }
                Ok((out, LLVMType::I1))
            }
            HirBinOp::NotEq => {
                if fl {
                    self.i(format!("{out} = fcmp one double {l}, {r}"));
                } else {
                    self.i(format!("{out} = icmp ne i64 {l}, {r}"));
                }
                Ok((out, LLVMType::I1))
            }
            HirBinOp::Lt => {
                if fl {
                    self.i(format!("{out} = fcmp olt double {l}, {r}"));
                } else {
                    self.i(format!("{out} = icmp slt i64 {l}, {r}"));
                }
                Ok((out, LLVMType::I1))
            }
            HirBinOp::Gt => {
                if fl {
                    self.i(format!("{out} = fcmp ogt double {l}, {r}"));
                } else {
                    self.i(format!("{out} = icmp sgt i64 {l}, {r}"));
                }
                Ok((out, LLVMType::I1))
            }
            HirBinOp::LtEq => {
                if fl {
                    self.i(format!("{out} = fcmp ole double {l}, {r}"));
                } else {
                    self.i(format!("{out} = icmp sle i64 {l}, {r}"));
                }
                Ok((out, LLVMType::I1))
            }
            HirBinOp::GtEq => {
                if fl {
                    self.i(format!("{out} = fcmp oge double {l}, {r}"));
                } else {
                    self.i(format!("{out} = icmp sge i64 {l}, {r}"));
                }
                Ok((out, LLVMType::I1))
            }
            HirBinOp::And => {
                let li1 = self.coerce(&l, &ty, &LLVMType::I1);
                let ri1 = self.coerce(&r, &ty, &LLVMType::I1);
                self.i(format!("{out} = and i1 {li1}, {ri1}"));
                Ok((out, LLVMType::I1))
            }
            HirBinOp::Or => {
                let li1 = self.coerce(&l, &ty, &LLVMType::I1);
                let ri1 = self.coerce(&r, &ty, &LLVMType::I1);
                self.i(format!("{out} = or i1 {li1}, {ri1}"));
                Ok((out, LLVMType::I1))
            }
            HirBinOp::StrConcat => {
                let (l, lt) = self.emit_expr(lhs)?;
                let (r, rt) = self.emit_expr(rhs)?;
                // Convert each side to i8* (string) if needed
                let ls = self.value_to_str(&l, &lt);
                let rs = self.value_to_str(&r, &rt);
                let out = self.r();
                self.i(format!(
                    "{out} = call i8* @ash_str_concat(i8* {ls}, i8* {rs})"
                ));
                Ok((out, LLVMType::Ptr))
            }
        }
    }

    // ── if expression ─────────────────────────────────────────────────────────

    fn emit_if(
        &mut self,
        cond: &HirExpr,
        then: &HirBlock,
        else_: Option<&HirExpr>,
        hint: &LLVMType,
    ) -> CResult<(String, LLVMType)> {
        let (cr, cty) = self.emit_expr(cond)?;
        let ci1 = self.coerce(&cr, &cty, &LLVMType::I1);
        let tl = self.l();
        let el = self.l();
        let ml = self.l();
        self.i(format!("br i1 {ci1}, label %{tl}, label %{el}"));

        // Phi type: promote i1 to i64 to avoid FastISel issues
        let phi_ty = if hint == &LLVMType::I1 {
            &LLVMType::I64
        } else {
            hint
        };

        // Then block
        self.emit(format!("{tl}:"));
        self.push_scope();
        let mut tv = ("0".to_string(), LLVMType::I64);
        for s in &then.stmts {
            if let Some(v) = self.emit_stmt(s)? {
                tv = v;
            }
        }
        self.pop_scope();
        let then_val = self.coerce(&tv.0, &tv.1, phi_ty);
        // Capture the ACTUAL block that will branch to merge — may differ from tl
        // if then block emitted additional sub-blocks (e.g. while loops inside if)
        let then_pred = self.cur_block.clone();
        self.i(format!("br label %{ml}"));

        // Else block
        self.emit(format!("{el}:"));
        let (else_val, else_pred) = if let Some(e) = else_ {
            let (er, ety) = self.emit_expr(e)?;
            let coerced = self.coerce(&er, &ety, phi_ty);
            let pred = self.cur_block.clone();
            self.i(format!("br label %{ml}"));
            (coerced, pred)
        } else {
            let pred = self.cur_block.clone();
            self.i(format!("br label %{ml}"));
            let zero = match phi_ty {
                LLVMType::Double => "0.0".into(),
                _ => "0".into(),
            };
            (zero, pred)
        };

        self.emit(format!("{ml}:"));
        if hint == &LLVMType::Void {
            return Ok(("0".into(), LLVMType::Void));
        }
        let out = self.r();
        self.i(format!(
            "{out} = phi {phi_ty} [ {then_val}, %{then_pred} ], [ {else_val}, %{else_pred} ]"
        ));
        Ok((out, phi_ty.clone()))
    }

    // ── match expression ──────────────────────────────────────────────────────

    fn emit_match(
        &mut self,
        scrutinee: &HirExpr,
        arms: &[HirArm],
        hint: &LLVMType,
    ) -> CResult<(String, LLVMType)> {
        let (sr, st) = self.emit_expr(scrutinee)?;
        let ml = self.l();
        let mut phi: Vec<(String, String)> = vec![];
        let mut next = self.l();

        for (i, arm) in arms.iter().enumerate() {
            let is_last = i == arms.len() - 1;
            let bl = self.l();
            let fl = if is_last { ml.clone() } else { next.clone() };

            match &arm.pattern {
                HirPattern::Wildcard | HirPattern::Var(_, _) => {
                    self.i(format!("br label %{bl}"));
                }
                HirPattern::Lit(lit) => {
                    let (lr, _) = match lit {
                        HirLitPat::Int(n) => (n.to_string(), LLVMType::I64),
                        HirLitPat::Bool(b) => {
                            (if *b { "1" } else { "0" }.to_string(), LLVMType::I1)
                        }
                        HirLitPat::Float(f) => (format!("{f:.17}"), LLVMType::Double),
                        HirLitPat::Str(_) => {
                            return Err(CodegenError::new("str pattern match not in codegen"))
                        }
                    };
                    let cmp = self.r();
                    let sr_coerced = self.coerce(&sr, &st, &LLVMType::I64);
                    self.i(format!("{cmp} = icmp eq i64 {sr_coerced}, {lr}"));
                    self.i(format!("br i1 {cmp}, label %{bl}, label %{fl}"));
                }
                _ => {
                    self.i(format!("br label %{bl}"));
                }
            }

            self.emit(format!("{bl}:"));
            self.push_scope();
            if let HirPattern::Var(name, ty) = &arm.pattern {
                let vty = hir_to_llvm(ty);
                let slot = format!("%slot_{name}");
                let sr_c = self.coerce(&sr, &st, &vty);
                self.i(format!("{slot} = alloca {vty}"));
                self.i(format!("store {vty} {sr_c}, {vty}* {slot}"));
                self.def(name, &slot, vty);
            }
            let (br, bty) = self.emit_expr(&arm.body)?;
            let coerced_br = self.coerce(&br, &bty, hint);
            let cur = bl.clone();
            self.pop_scope();
            self.i(format!("br label %{ml}"));
            phi.push((coerced_br, cur));
            if !is_last {
                self.emit(format!("{next}:"));
                next = self.l();
            }
        }

        self.emit(format!("{ml}:"));
        if phi.is_empty() {
            return Ok(("0".into(), LLVMType::I64));
        }
        let out = self.r();
        // Normalize: never use i1 as phi type
        let phi_ty = if hint == &LLVMType::I1 {
            &LLVMType::I64
        } else {
            hint
        };
        let pp: Vec<String> = phi.iter().map(|(r, l)| format!("[ {r}, %{l} ]")).collect();
        self.i(format!("{out} = phi {phi_ty} {}", pp.join(", ")));
        Ok((out, phi_ty.clone()))
    }

    // ── named function call ───────────────────────────────────────────────────

    fn emit_named_call(
        &mut self,
        name: &str,
        args: &[HirExpr],
        ret_hint: &LLVMType,
    ) -> CResult<(String, LLVMType)> {
        match name {
            "println" | "print" => {
                let nl = if name == "println" { "\n" } else { "" };
                let fmt_str = if args.is_empty() {
                    nl.to_string()
                } else {
                    let f = match self.infer_arg_llvm_ty(args.first()) {
                        LLVMType::I64 => "%ld",
                        LLVMType::Double => "%f",
                        LLVMType::I1 => "%d",
                        LLVMType::Ptr => "%s",
                        _ => "%d",
                    };
                    format!("{f}{nl}")
                };
                let lbl = self.intern(&fmt_str);
                let len = fmt_str.len() + 1;
                let ptr = self.r();
                self.i(format!(
                    "{ptr} = getelementptr [{len} x i8], [{len} x i8]* {lbl}, i64 0, i64 0"
                ));
                let mut arg_strs = vec![];
                for a in args {
                    let (r, t) = self.emit_expr(a)?;
                    arg_strs.push(format!("{t} {r}"));
                }
                let sep = if arg_strs.is_empty() {
                    "".into()
                } else {
                    format!(", {}", arg_strs.join(", "))
                };
                self.i(format!("call i32 (i8*, ...) @printf(i8* {ptr}{sep})"));
                Ok(("0".into(), LLVMType::Void))
            }
            "int" => {
                if args.is_empty() {
                    return Ok(("0".into(), LLVMType::I64));
                }
                let (r, t) = self.emit_expr(&args[0])?;
                let out = self.r();
                match t {
                    LLVMType::Double => self.i(format!("{out} = fptosi double {r} to i64")),
                    LLVMType::I1 => self.i(format!("{out} = zext i1 {r} to i64")),
                    _ => return Ok((r, LLVMType::I64)),
                };
                Ok((out, LLVMType::I64))
            }
            "float" => {
                if args.is_empty() {
                    return Ok(("0.0".into(), LLVMType::Double));
                }
                let (r, t) = self.emit_expr(&args[0])?;
                let out = self.r();
                match t {
                    LLVMType::I64 => self.i(format!("{out} = sitofp i64 {r} to double")),
                    _ => return Ok((r, LLVMType::Double)),
                };
                Ok((out, LLVMType::Double))
            }
            "abs" => {
                if args.is_empty() {
                    return Ok(("0".into(), LLVMType::I64));
                }
                let (r, t) = self.emit_expr(&args[0])?;
                let neg = self.r();
                let cmp = self.r();
                let out = self.r();
                match t {
                    LLVMType::I64 => {
                        self.i(format!("{neg} = sub i64 0, {r}"));
                        self.i(format!("{cmp} = icmp slt i64 {r}, 0"));
                        self.i(format!("{out} = select i1 {cmp}, i64 {neg}, i64 {r}"));
                        Ok((out, LLVMType::I64))
                    }
                    LLVMType::Double => {
                        self.i(format!("{out} = call double @llvm.fabs.f64(double {r})"));
                        Ok((out, LLVMType::Double))
                    }
                    _ => Err(CodegenError::new("abs requires number")),
                }
            }
            "min" | "max" => {
                if args.len() < 2 {
                    return Err(CodegenError::new(format!("{name} requires 2 args")));
                }
                let (a, _) = self.emit_expr(&args[0])?;
                let (b, _) = self.emit_expr(&args[1])?;
                let cmp = self.r();
                let out = self.r();
                let op = if name == "min" { "slt" } else { "sgt" };
                self.i(format!("{cmp} = icmp {op} i64 {a}, {b}"));
                self.i(format!("{out} = select i1 {cmp}, i64 {a}, i64 {b}"));
                Ok((out, LLVMType::I64))
            }
            _ => {
                // User-defined function — use registered signature for return type
                let ret_ty = self
                    .fn_sigs
                    .get(name)
                    .map(|(_, r)| r.clone())
                    .unwrap_or_else(|| {
                        // Fall back to hint if not registered (builtins etc)
                        if ret_hint == &LLVMType::Void {
                            LLVMType::I64
                        } else {
                            ret_hint.clone()
                        }
                    });
                let mut arg_strs = vec![];
                for a in args {
                    let (r, t) = self.emit_expr(a)?;
                    arg_strs.push(format!("{t} {r}"));
                }
                let out = self.r();
                self.i(format!(
                    "{out} = call {ret_ty} @{name}({})",
                    arg_strs.join(", ")
                ));
                Ok((out, ret_ty))
            }
        }
    }

    /// Convert a register of any type to i8* (string) using runtime helpers.
    fn value_to_str(&mut self, reg: &str, ty: &LLVMType) -> String {
        match ty {
            LLVMType::Ptr => reg.to_string(),
            LLVMType::I64 => {
                let out = self.r();
                self.i(format!("{out} = call i8* @ash_str_from_int(i64 {reg})"));
                out
            }
            LLVMType::Double => {
                let out = self.r();
                self.i(format!(
                    "{out} = call i8* @ash_str_from_float(double {reg})"
                ));
                out
            }
            LLVMType::I1 => {
                let ext = self.r();
                self.i(format!("{ext} = zext i1 {reg} to i64"));
                let out = self.r();
                self.i(format!("{out} = call i8* @ash_str_from_bool(i64 {ext})"));
                out
            }
            _ => reg.to_string(),
        }
    }

    fn infer_arg_llvm_ty(&self, arg: Option<&HirExpr>) -> LLVMType {
        let expr = match arg {
            Some(e) => e,
            None => return LLVMType::I64,
        };
        // For function calls whose HIR type is Unknown, look up the resolved
        // return type from fn_sigs (which gets patched during emit_fn).
        if expr.ty == HirType::Unknown {
            if let HirExprKind::Call { callee, .. } = &expr.kind {
                if let HirExprKind::Var(name) = &callee.kind {
                    if let Some((_, ret)) = self.fn_sigs.get(name) {
                        return ret.clone();
                    }
                }
            }
        }
        hir_to_llvm(&expr.ty)
    }

    // ── method call ───────────────────────────────────────────────────────────

    fn emit_method_call(
        &mut self,
        obj_reg: &str,
        obj_ty: &LLVMType,
        method: &str,
        _args: &[HirExpr],
    ) -> CResult<(String, LLVMType)> {
        match (obj_ty, method) {
            (LLVMType::Ptr, "len") => {
                let out = self.r();
                self.i(format!("{out} = call i64 @strlen(i8* {obj_reg})"));
                Ok((out, LLVMType::I64))
            }
            _ => Err(CodegenError::new(format!(
                "method '{method}' on {obj_ty} not yet in codegen"
            ))),
        }
    }

    // ── math namespace ────────────────────────────────────────────────────────

    fn emit_math_call(&mut self, func: &str, args: &[HirExpr]) -> CResult<(String, LLVMType)> {
        let (r, _t) = if args.is_empty() {
            ("0.0".to_string(), LLVMType::Double)
        } else {
            let (r, t) = self.emit_expr(&args[0])?;
            let r = self.coerce(&r, &t, &LLVMType::Double);
            (r, LLVMType::Double)
        };
        let out = self.r();
        match func {
            "sqrt" => {
                self.i(format!("{out} = call double @llvm.sqrt.f64(double {r})"));
                Ok((out, LLVMType::Double))
            }
            "floor" => {
                self.i(format!("{out} = call double @llvm.floor.f64(double {r})"));
                Ok((out, LLVMType::Double))
            }
            "ceil" => {
                self.i(format!("{out} = call double @llvm.ceil.f64(double {r})"));
                Ok((out, LLVMType::Double))
            }
            "sin" => {
                self.i(format!("{out} = call double @sin(double {r})"));
                Ok((out, LLVMType::Double))
            }
            "cos" => {
                self.i(format!("{out} = call double @cos(double {r})"));
                Ok((out, LLVMType::Double))
            }
            "log" => {
                self.i(format!("{out} = call double @log(double {r})"));
                Ok((out, LLVMType::Double))
            }
            "pi" => Ok(("3.141592653589793".to_string(), LLVMType::Double)),
            "e" => Ok(("2.718281828459045".to_string(), LLVMType::Double)),
            "abs" => {
                self.i(format!("{out} = call double @llvm.fabs.f64(double {r})"));
                Ok((out, LLVMType::Double))
            }
            "pow" => {
                if args.len() < 2 {
                    return Err(CodegenError::new("math.pow needs 2 args"));
                }
                let (r2, t2) = self.emit_expr(&args[1])?;
                let r2 = self.coerce(&r2, &t2, &LLVMType::Double);
                self.i(format!("{out} = call double @pow(double {r}, double {r2})"));
                Ok((out, LLVMType::Double))
            }
            _ => Err(CodegenError::new(format!("math.{func} not in codegen"))),
        }
    }
}

// ─── Public API ───────────────────────────────────────────────────────────────

/// Compile an AST via typeck → HIR → LLVM IR.
pub fn compile(program: &Program) -> Result<String, CodegenError> {
    // Run type checker to get fully typed HIR
    let hir =
        ash_typeck::check(program).map_err(|e| CodegenError::new(format!("type error: {e}")))?;
    Codegen::new().compile(&hir)
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use ash_lexer::Lexer;
    use ash_parser::parse;

    fn codegen(src: &str) -> String {
        let tokens = Lexer::new(src).tokenize().expect("lex");
        let program = parse(tokens).expect("parse");
        compile(&program).expect("codegen")
    }
    #[allow(dead_code)]
    fn codegen_fails(src: &str) -> bool {
        let tokens = Lexer::new(src).tokenize().expect("lex");
        let program = parse(tokens).expect("parse");
        compile(&program).is_err()
    }

    #[test]
    fn test_has_main() {
        assert!(codegen("1+1").contains("define i32 @main()"));
    }
    #[test]
    fn test_ret_i32_0() {
        assert!(codegen("1+1").contains("ret i32 0"));
    }
    #[test]
    fn test_add() {
        assert!(codegen("1+2").contains("add i64"));
    }
    #[test]
    fn test_sub() {
        assert!(codegen("5-3").contains("sub i64"));
    }
    #[test]
    fn test_mul() {
        assert!(codegen("3*4").contains("mul i64"));
    }
    #[test]
    fn test_div() {
        assert!(codegen("10/2").contains("sdiv i64"));
    }
    #[test]
    fn test_mod() {
        assert!(codegen("7%3").contains("srem i64"));
    }
    #[test]
    fn test_float_add() {
        assert!(codegen("1.5+2.5").contains("fadd double"));
    }
    #[test]
    fn test_lt() {
        assert!(codegen("1<2").contains("icmp slt"));
    }
    #[test]
    fn test_gt() {
        assert!(codegen("2>1").contains("icmp sgt"));
    }
    #[test]
    fn test_eq() {
        assert!(codegen("1==1").contains("icmp eq"));
    }
    #[test]
    fn test_not() {
        assert!(codegen("!true").contains("xor i1"));
    }
    #[test]
    fn test_neg() {
        assert!(codegen("-5").contains("sub i64 0"));
    }
    #[test]
    fn test_fn_def_typed() {
        assert!(codegen("fn add(a:int b:int):int\n    a+b").contains("define i64 @add("));
    }
    #[test]
    fn test_fn_inferred() {
        assert!(codegen("fn double(x)\n    x*2").contains("define i64 @double("));
    }
    #[test]
    fn test_fn_call() {
        assert!(codegen("fn double(x:int):int\n    x*2\ndouble(5)").contains("call i64 @double("));
    }
    #[test]
    fn test_let_binding() {
        let ir = codegen("let x:int=42");
        assert!(ir.contains("alloca i64") && ir.contains("store i64 42"));
    }
    #[test]
    fn test_if_phi() {
        assert!(codegen("if true\n    1\nelse\n    0").contains("phi i64"));
    }
    #[test]
    fn test_while() {
        assert!(codegen("mut x:int=0\nwhile x<5\n    x=x+1").contains("icmp slt"));
    }
    #[test]
    fn test_println_int() {
        assert!(codegen("println(42)").contains("@printf"));
    }
    #[test]
    fn test_int_conv() {
        assert!(codegen("int(3.14)").contains("fptosi"));
    }
    #[test]
    fn test_float_conv() {
        assert!(codegen("float(3)").contains("sitofp"));
    }
    #[test]
    fn test_abs() {
        assert!(codegen("abs(-5)").contains("select"));
    }
    #[test]
    fn test_min() {
        assert!(codegen("min(3 7)").contains("icmp slt"));
    }
    #[test]
    fn test_max() {
        assert!(codegen("max(3 7)").contains("icmp sgt"));
    }
    #[test]
    fn test_match() {
        assert!(
            codegen("let x:int=2\nmatch x\n    1=>10\n    2=>20\n    _=>0").contains("phi i64")
        );
    }
    #[test]
    fn test_recursive() {
        let ir =
            codegen("fn fact(n:int):int\n    if n<=1\n        1\n    else\n        n*fact(n-1)");
        assert!(ir.contains("call i64 @fact("));
    }
    #[test]
    fn test_multiple_fns() {
        let ir = codegen("fn sq(x:int):int\n    x*x\nfn cb(x:int):int\n    x*sq(x)");
        assert!(ir.contains("@sq") && ir.contains("@cb"));
    }
    #[test]
    fn test_is_prime_ir() {
        let src = "fn is_prime(n:bool)\n    if n < 2\n        false\n    else\n        true\nprintln(is_prime(7))";
        let ir = codegen(src);
        // Should have i64 params because n is inferred
        assert!(ir.contains("@is_prime"), "should define is_prime");
    }
    #[test]
    fn test_is_prime_correct_ir() {
        let src = "fn is_prime(n)\n    if n < 2\n        false\n    else\n        mut prime = true\n        prime\nprintln(is_prime(7))";
        let ir = codegen(src);
        eprintln!("=== IR ===\n{}", ir);
        // is_prime should have i64 return (bools promoted)
        assert!(
            ir.contains("define i64 @is_prime(i64 %p_n)"),
            "should have i64 param"
        );
    }
    #[test]
    fn test_bool_and() {
        assert!(codegen("true && false").contains("and i1"));
    }
    #[test]
    fn test_bool_or() {
        assert!(codegen("true || false").contains("or i1"));
    }
    #[test]
    fn test_return() {
        assert!(codegen("fn f(x:int):int\n    return x+1").contains("ret i64"));
    }
    #[test]
    fn test_inferred_fn_body() {
        // fn fib(n) with no annotations should still produce correct IR
        let ir = codegen("fn fib(n)\n    if n<=1\n        n\n    else\n        fib(n-1)+fib(n-2)");
        assert!(
            ir.contains("define i64 @fib(i64 %p_n)"),
            "fib should have i64 params"
        );
        assert!(
            ir.contains("call i64 @fib("),
            "fib should call itself with i64"
        );
    }
}
