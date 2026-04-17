#![allow(unused_variables)]
//! ash-typeck
//! Type inference and checking. Consumes an AST and produces a typed HIR.
//!
//! Algorithm: constraint-based Hindley-Milner style inference.
//! 1. Walk the AST, assign fresh type variables to unknown types
//! 2. Generate equality constraints from usage
//! 3. Solve constraints via unification
//! 4. Substitute solved types back, producing a fully-typed HIR

use ash_hir::*;
use ash_lexer::Span;
use ash_parser::ast::*;
use std::collections::HashMap;

// --- Error --------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct TypeError {
    pub msg: String,
    pub span: Option<Span>,
}

impl TypeError {
    pub fn new(msg: impl Into<String>) -> Self {
        TypeError {
            msg: msg.into(),
            span: None,
        }
    }
    pub fn at(msg: impl Into<String>, span: Span) -> Self {
        TypeError {
            msg: msg.into(),
            span: Some(span),
        }
    }
}

impl std::fmt::Display for TypeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(ref s) = self.span {
            write!(f, "error[type] at {s}: {}", self.msg)
        } else {
            write!(f, "error[type]: {}", self.msg)
        }
    }
}

type TResult<T> = Result<T, TypeError>;

// --- Type variable ------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TyVar(usize);

// --- Type with variables ------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum InferTy {
    Concrete(HirType),
    Var(TyVar),
}

impl InferTy {
    #[allow(dead_code)]
    fn is_var(&self) -> bool {
        matches!(self, InferTy::Var(_))
    }
}

// --- Constraint ---------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Constraint {
    pub lhs: InferTy,
    pub rhs: InferTy,
}

// --- Substitution ------------------------------------------------------------

#[derive(Debug, Default)]
pub struct Subst {
    map: HashMap<TyVar, InferTy>,
}

impl Subst {
    pub fn apply(&self, ty: &InferTy) -> InferTy {
        match ty {
            InferTy::Var(v) => {
                if let Some(t) = self.map.get(v) {
                    self.apply(t)
                } else {
                    ty.clone()
                }
            }
            InferTy::Concrete(h) => InferTy::Concrete(self.apply_hir(h)),
        }
    }

    pub fn apply_hir(&self, ty: &HirType) -> HirType {
        match ty {
            HirType::Option(t) => HirType::Option(Box::new(self.apply_hir(t))),
            HirType::Result(t, e) => {
                HirType::Result(Box::new(self.apply_hir(t)), Box::new(self.apply_hir(e)))
            }
            HirType::List(t) => HirType::List(Box::new(self.apply_hir(t))),
            HirType::Map(k, v) => {
                HirType::Map(Box::new(self.apply_hir(k)), Box::new(self.apply_hir(v)))
            }
            HirType::Tuple(ts) => HirType::Tuple(ts.iter().map(|t| self.apply_hir(t)).collect()),
            HirType::Fn(ps, r) => HirType::Fn(
                ps.iter().map(|t| self.apply_hir(t)).collect(),
                Box::new(self.apply_hir(r)),
            ),
            other => other.clone(),
        }
    }

    pub fn bind(&mut self, v: TyVar, ty: InferTy) {
        self.map.insert(v, ty);
    }
}

// --- Unifier -----------------------------------------------------------------

#[allow(dead_code)]
fn unify(a: &InferTy, b: &InferTy, subst: &mut Subst) -> TResult<()> {
    let a = subst.apply(a);
    let b = subst.apply(b);
    match (&a, &b) {
        (InferTy::Concrete(x), InferTy::Concrete(y)) => {
            if !types_compatible(x, y) {
                return Err(TypeError::new(format!(
                    "type mismatch: expected {x}, got {y}"
                )));
            }
            Ok(())
        }
        (InferTy::Var(v), other) | (other, InferTy::Var(v)) => {
            subst.bind(v.clone(), other.clone());
            Ok(())
        }
    }
}

fn types_compatible(a: &HirType, b: &HirType) -> bool {
    match (a, b) {
        (HirType::Unknown, _) | (_, HirType::Unknown) => true,
        (HirType::Int, HirType::Int) => true,
        (HirType::Float, HirType::Float) => true,
        (HirType::Bool, HirType::Bool) => true,
        (HirType::Str, HirType::Str) => true,
        (HirType::Void, HirType::Void) => true,
        // Numeric coercion: int and float are compatible (we promote int → float)
        (HirType::Int, HirType::Float) | (HirType::Float, HirType::Int) => true,
        (HirType::Option(x), HirType::Option(y)) => types_compatible(x, y),
        // int is compatible with ?int (auto-wrapping in Some)
        (HirType::Option(x), y) | (y, HirType::Option(x)) => types_compatible(x, y),
        (HirType::List(x), HirType::List(y)) => types_compatible(x, y),
        (HirType::Generic(_), _) | (_, HirType::Generic(_)) => true,
        (HirType::Struct(a), HirType::Struct(b)) => a == b,
        (HirType::Union(a), HirType::Union(b)) => a == b,
        _ => false,
    }
}

// --- Type checker / lowering pass --------------------------------------------

pub struct TypeChecker {
    // Counter for fresh type variables (reserved for future constraint solving)
    #[allow(dead_code)]
    next_var: usize,
    // Substitution built during constraint solving (reserved for future use)
    #[allow(dead_code)]
    subst: Subst,
    // Type environment: name → type
    env: TypeEnv,
    // Known function signatures
    fn_sigs: HashMap<String, (Vec<HirType>, HirType)>,
    // Type registry for structs/unions
    registry: TypeRegistry,
    // Lambda counter for unique names
    lambda_counter: usize,
    // Lifted lambdas collected during traversal
    lifted: Vec<HirFn>,
}

impl Default for TypeChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl TypeChecker {
    pub fn new() -> Self {
        let mut tc = TypeChecker {
            next_var: 0,
            subst: Subst::default(),
            env: TypeEnv::new(),
            fn_sigs: HashMap::new(),
            registry: TypeRegistry::new(),
            lambda_counter: 0,
            lifted: vec![],
        };
        tc.register_builtins();
        tc
    }

    #[allow(dead_code)]
    fn fresh(&mut self) -> TyVar {
        let v = TyVar(self.next_var);
        self.next_var += 1;
        v
    }

    #[allow(clippy::cloned_ref_to_slice_refs)]
    fn register_builtins(&mut self) {
        // Core functions
        let builtins: &[(&str, &[HirType], HirType)] = &[
            ("println", &[HirType::Unknown], HirType::Void),
            ("print", &[HirType::Unknown], HirType::Void),
            ("int", &[HirType::Unknown], HirType::Int),
            ("float", &[HirType::Unknown], HirType::Float),
            ("str", &[HirType::Unknown], HirType::Str),
            ("bool", &[HirType::Unknown], HirType::Bool),
            ("abs", &[HirType::Unknown], HirType::Unknown),
            (
                "min",
                &[HirType::Unknown, HirType::Unknown],
                HirType::Unknown,
            ),
            (
                "max",
                &[HirType::Unknown, HirType::Unknown],
                HirType::Unknown,
            ),
            ("fmt", &[HirType::Str], HirType::Str),
            (
                "filter",
                &[HirType::Unknown, HirType::Unknown],
                HirType::Unknown,
            ),
            (
                "map",
                &[HirType::Unknown, HirType::Unknown],
                HirType::Unknown,
            ),
            (
                "zip",
                &[HirType::Unknown, HirType::Unknown],
                HirType::Unknown,
            ),
            (
                "reduce",
                &[HirType::Unknown, HirType::Unknown, HirType::Unknown],
                HirType::Unknown,
            ),
            ("any", &[HirType::Unknown, HirType::Unknown], HirType::Bool),
            ("all", &[HirType::Unknown, HirType::Unknown], HirType::Bool),
            ("flat", &[HirType::Unknown], HirType::Unknown),
        ];
        for (name, params, ret) in builtins {
            self.fn_sigs
                .insert(name.to_string(), (params.to_vec(), ret.clone()));
        }

        // Register stdlib namespace functions (all accept Unknown args for flexibility)
        let u = HirType::Unknown;
        let s = HirType::Str;
        let b = HirType::Bool;
        let i = HirType::Int;
        let ns_fns: &[(&str, &[HirType], HirType)] = &[
            // math.*
            ("math.floor", &[u.clone()], i.clone()),
            ("math.ceil", &[u.clone()], i.clone()),
            ("math.round", &[u.clone()], i.clone()),
            ("math.sqrt", &[u.clone()], HirType::Float),
            ("math.pow", &[u.clone(), u.clone()], HirType::Float),
            ("math.log", &[u.clone()], HirType::Float),
            ("math.sin", &[u.clone()], HirType::Float),
            ("math.cos", &[u.clone()], HirType::Float),
            ("math.tan", &[u.clone()], HirType::Float),
            ("math.abs", &[u.clone()], u.clone()),
            ("math.clamp", &[u.clone(), u.clone(), u.clone()], u.clone()),
            // file.*
            (
                "file.read",
                &[s.clone()],
                HirType::Option(Box::new(s.clone())),
            ),
            ("file.write", &[s.clone(), s.clone()], HirType::Void),
            ("file.append", &[s.clone(), s.clone()], HirType::Void),
            ("file.exists", &[s.clone()], b.clone()),
            ("file.rm", &[s.clone()], HirType::Void),
            ("file.mkdir", &[s.clone()], HirType::Void),
            ("file.ls", &[s.clone()], HirType::List(Box::new(s.clone()))),
            // env.*
            (
                "env.get",
                &[s.clone()],
                HirType::Option(Box::new(s.clone())),
            ),
            ("env.require", &[s.clone()], s.clone()),
            ("env.set", &[s.clone(), s.clone()], HirType::Void),
            // json.*
            ("json.str", &[u.clone()], s.clone()),
            ("json.pretty", &[u.clone()], s.clone()),
            (
                "json.parse",
                &[s.clone()],
                HirType::Option(Box::new(u.clone())),
            ),
            // re.*
            ("re.match", &[s.clone(), s.clone()], b.clone()),
            (
                "re.find",
                &[s.clone(), s.clone()],
                HirType::Option(Box::new(s.clone())),
            ),
            (
                "re.findall",
                &[s.clone(), s.clone()],
                HirType::List(Box::new(s.clone())),
            ),
            ("re.replace", &[s.clone(), s.clone(), s.clone()], s.clone()),
            (
                "re.split",
                &[s.clone(), s.clone()],
                HirType::List(Box::new(s.clone())),
            ),
            // http.*
            (
                "http.get",
                &[s.clone()],
                HirType::Option(Box::new(s.clone())),
            ),
            (
                "http.post",
                &[s.clone(), s.clone()],
                HirType::Option(Box::new(s.clone())),
            ),
            (
                "http.put",
                &[s.clone(), s.clone()],
                HirType::Option(Box::new(s.clone())),
            ),
            (
                "http.del",
                &[s.clone()],
                HirType::Option(Box::new(s.clone())),
            ),
            (
                "http.patch",
                &[s.clone(), s.clone()],
                HirType::Option(Box::new(s.clone())),
            ),
            // db.*
            ("db.connect", &[s.clone()], u.clone()),
            ("db.exec", &[u.clone(), s.clone()], i.clone()),
            ("db.query", &[u.clone(), s.clone()], u.clone()),
            ("db.close", &[u.clone()], HirType::Void),
            // go.*
            ("go.sleep", &[i.clone()], HirType::Void),
            ("go.spawn", &[u.clone()], u.clone()),
            ("go.wait", &[u.clone()], u.clone()),
            ("go.all", &[u.clone()], u.clone()),
            // cache.*
            ("cache.set", &[s.clone(), u.clone()], HirType::Void),
            (
                "cache.get",
                &[s.clone()],
                HirType::Option(Box::new(u.clone())),
            ),
            ("cache.has", &[s.clone()], b.clone()),
            ("cache.del", &[s.clone()], HirType::Void),
            ("cache.clear", &[], HirType::Void),
            // queue.*
            ("queue.push", &[s.clone(), u.clone()], HirType::Void),
            (
                "queue.pop",
                &[s.clone()],
                HirType::Option(Box::new(u.clone())),
            ),
            ("queue.len", &[s.clone()], i.clone()),
            ("queue.clear", &[s.clone()], HirType::Void),
            // assert
            ("assert", &[b.clone(), s.clone()], HirType::Void),
            // ai.*
            ("ai.ask", &[s.clone()], s.clone()),
            ("ai.complete", &[s.clone()], s.clone()),
            ("ai.chat", &[s.clone()], s.clone()),
        ];
        for (name, params, ret) in ns_fns {
            self.fn_sigs
                .insert(name.to_string(), (params.to_vec(), ret.clone()));
        }
        // Constants
        self.env
            .define("none", HirType::Option(Box::new(HirType::Unknown)));
        self.env
            .define("None", HirType::Option(Box::new(HirType::Unknown)));
        self.fn_sigs.insert(
            "Some".into(),
            (
                vec![HirType::Unknown],
                HirType::Option(Box::new(HirType::Unknown)),
            ),
        );
        self.fn_sigs
            .insert("Ok".into(), (vec![HirType::Unknown], HirType::Unknown));
        self.fn_sigs
            .insert("Err".into(), (vec![HirType::Unknown], HirType::Unknown));
        self.fn_sigs
            .insert("panic".into(), (vec![HirType::Str], HirType::Void));
        self.fn_sigs.insert(
            "clamp".into(),
            (vec![HirType::Unknown; 3].to_vec(), HirType::Unknown),
        );
    }

    // -- public entry ---------------------------------------------------------

    pub fn check(mut self, program: &Program) -> TResult<HirProgram> {
        // First pass: register all type defs and fn signatures
        for stmt in &program.stmts {
            match &stmt.kind {
                StmtKind::TypeDef(td) => self.register_type_def(td)?,
                StmtKind::FnDef(f) => self.register_fn_sig(f),
                _ => {}
            }
        }

        // Second pass: lower everything
        let mut hir_fns = vec![];
        let mut hir_types = vec![];
        let mut top_stmts = vec![];

        for stmt in &program.stmts {
            match &stmt.kind {
                StmtKind::FnDef(f) => {
                    hir_fns.push(self.lower_fn(f)?);
                }
                StmtKind::TypeDef(td) => {
                    hir_types.push(self.lower_type_def(td)?);
                }
                _ => {
                    top_stmts.push(self.lower_stmt(stmt)?);
                }
            }
        }

        let lifted = std::mem::take(&mut self.lifted);
        Ok(HirProgram {
            fns: hir_fns,
            types: hir_types,
            top_stmts,
            lifted,
        })
    }

    fn register_type_def(&mut self, td: &TypeDef) -> TResult<()> {
        match &td.kind {
            TypeDefKind::Struct(fields) => {
                let hfields: Vec<HirField> = fields
                    .iter()
                    .map(|f| HirField {
                        name: f.name.clone(),
                        ty: self.lower_ash_type(&f.ty),
                    })
                    .collect();
                self.registry.register_struct(&td.name, hfields);
            }
            TypeDefKind::Union(variants) => {
                let hvariants: Vec<HirVariant> = variants
                    .iter()
                    .map(|v| HirVariant {
                        name: v.name.clone(),
                        fields: v.fields.iter().map(|t| self.lower_ash_type(t)).collect(),
                    })
                    .collect();
                self.registry.register_union(&td.name, hvariants);
                // Register constructor functions
                for v in variants {
                    let field_types: Vec<HirType> =
                        v.fields.iter().map(|t| self.lower_ash_type(t)).collect();
                    self.fn_sigs.insert(
                        v.name.clone(),
                        (field_types, HirType::Union(td.name.clone())),
                    );
                }
            }
        }
        Ok(())
    }

    fn register_fn_sig(&mut self, f: &FnDef) {
        let params: Vec<HirType> = f
            .params
            .iter()
            .map(|p| self.lower_ash_type(&p.ty))
            .collect();
        let ret = self.lower_ash_type(&f.ret);
        self.fn_sigs.insert(f.name.clone(), (params, ret));
    }

    // -- type lowering --------------------------------------------------------

    pub fn lower_ash_type(&self, ty: &AshType) -> HirType {
        match ty {
            AshType::Int => HirType::Int,
            AshType::Float => HirType::Float,
            AshType::Bool => HirType::Bool,
            AshType::Str => HirType::Str,
            AshType::Void => HirType::Void,
            AshType::Infer => HirType::Unknown,
            AshType::Option(t) => HirType::Option(Box::new(self.lower_ash_type(t))),
            AshType::Result(t, e) => HirType::Result(
                Box::new(self.lower_ash_type(t)),
                Box::new(self.lower_ash_type(e)),
            ),
            AshType::List(t) => HirType::List(Box::new(self.lower_ash_type(t))),
            AshType::Map(k, v) => HirType::Map(
                Box::new(self.lower_ash_type(k)),
                Box::new(self.lower_ash_type(v)),
            ),
            AshType::Tuple(ts) => {
                HirType::Tuple(ts.iter().map(|t| self.lower_ash_type(t)).collect())
            }
            AshType::Fn(ps, r) => HirType::Fn(
                ps.iter().map(|t| self.lower_ash_type(t)).collect(),
                Box::new(self.lower_ash_type(r)),
            ),
            AshType::Named(n) => {
                if self.registry.structs.contains_key(n.as_str()) {
                    HirType::Struct(n.clone())
                } else if self.registry.unions.contains_key(n.as_str()) {
                    HirType::Union(n.clone())
                } else {
                    HirType::Struct(n.clone())
                }
            }
            AshType::Generic(n) => HirType::Generic(n.clone()),
        }
    }

    // -- function lowering ----------------------------------------------------

    fn lower_fn(&mut self, f: &FnDef) -> TResult<HirFn> {
        self.env.push();
        let params: Vec<HirParam> = f
            .params
            .iter()
            .map(|p| {
                let ty = self.lower_ash_type(&p.ty);
                self.env.define(&p.name, ty.clone());
                HirParam {
                    name: p.name.clone(),
                    ty,
                    mutable: p.mutable,
                    borrow: p.borrow,
                }
            })
            .collect();

        let ret = self.lower_ash_type(&f.ret);
        let body = self.lower_block(&f.body, &ret)?;
        self.env.pop();

        Ok(HirFn {
            name: f.name.clone(),
            generics: f.generics.clone(),
            params,
            ret,
            body,
            captures: vec![],
        })
    }

    fn lower_block(&mut self, block: &Block, hint_ret: &HirType) -> TResult<HirBlock> {
        self.env.push();
        let mut stmts = vec![];
        let mut block_ty = HirType::Void;

        for (i, stmt) in block.stmts.iter().enumerate() {
            let is_last = i == block.stmts.len() - 1;
            let hstmt = self.lower_stmt(stmt)?;

            // Infer block return type from last expression
            if is_last {
                if let HirStmtKind::Expr(ref e) = hstmt.kind {
                    block_ty = e.ty.clone();
                    // If hint is known and not Unknown, use it
                    if *hint_ret != HirType::Unknown && *hint_ret != HirType::Void {
                        block_ty = hint_ret.clone();
                    }
                }
            }
            stmts.push(hstmt);
        }

        self.env.pop();
        Ok(HirBlock {
            stmts,
            ty: block_ty,
        })
    }

    // -- statement lowering ---------------------------------------------------

    fn lower_stmt(&mut self, stmt: &Stmt) -> TResult<HirStmt> {
        let kind = match &stmt.kind {
            StmtKind::Let {
                name,
                ty,
                mutable,
                value,
            } => {
                let hval = self.lower_expr(value)?;
                let resolved_ty = if *ty == AshType::Infer {
                    hval.ty.clone()
                } else {
                    let declared = self.lower_ash_type(ty);
                    // Verify compatibility
                    if !types_compatible(&declared, &hval.ty) {
                        return Err(TypeError::at(
                            format!(
                                "declared type {declared} incompatible with value type {}",
                                hval.ty
                            ),
                            stmt.span.clone(),
                        ));
                    }
                    declared
                };
                self.env.define(name, resolved_ty.clone());
                HirStmtKind::Let {
                    name: name.clone(),
                    ty: resolved_ty,
                    mutable: *mutable,
                    value: hval,
                }
            }

            StmtKind::Assign { target, value } => {
                let hval = self.lower_expr(value)?;
                // If target is a new ident, treat as implicit let
                if let ExprKind::Ident(name) = &target.kind {
                    if self.env.get(name).is_none() {
                        self.env.define(name, hval.ty.clone());
                    }
                }
                let htarget = self.lower_expr(target)?;
                HirStmtKind::Assign {
                    target: htarget,
                    value: hval,
                }
            }

            StmtKind::Return(expr) => {
                let hval = expr.as_ref().map(|e| self.lower_expr(e)).transpose()?;
                HirStmtKind::Return(hval)
            }

            StmtKind::While { cond, body } => {
                let hcond = self.lower_expr(cond)?;
                if !types_compatible(&hcond.ty, &HirType::Bool) {
                    return Err(TypeError::at(
                        "while condition must be bool",
                        stmt.span.clone(),
                    ));
                }
                let hbody = self.lower_block(body, &HirType::Void)?;
                HirStmtKind::While {
                    cond: hcond,
                    body: hbody,
                }
            }

            StmtKind::For { var, iter, body } => {
                let hiter = self.lower_expr(iter)?;
                let var_ty = match &hiter.ty {
                    HirType::List(elem) => *elem.clone(),
                    HirType::Str => HirType::Str,
                    _ => HirType::Unknown,
                };
                self.env.push();
                self.env.define(var, var_ty.clone());
                let hbody = self.lower_block(body, &HirType::Void)?;
                self.env.pop();
                HirStmtKind::For {
                    var: var.clone(),
                    var_ty,
                    iter: hiter,
                    body: hbody,
                }
            }

            StmtKind::Panic(msg) => HirStmtKind::Panic(self.lower_expr(msg)?),

            StmtKind::Expr(expr) => HirStmtKind::Expr(self.lower_expr(expr)?),

            StmtKind::FnDef(f) => {
                let hfn = self.lower_fn(f)?;
                // Register in env as a callable
                let sig_ty = HirType::Fn(
                    hfn.params.iter().map(|p| p.ty.clone()).collect(),
                    Box::new(hfn.ret.clone()),
                );
                self.env.define(&f.name, sig_ty);
                // Emit as expression wrapping the fn definition
                HirStmtKind::Expr(HirExpr::new(
                    HirExprKind::Var(f.name.clone()),
                    HirType::Void,
                ))
            }

            StmtKind::TypeDef(td) => {
                self.register_type_def(td)?;
                HirStmtKind::Expr(HirExpr::new(HirExprKind::Bool(false), HirType::Void))
            }

            // Use statements are inlined by the CLI loader; treat as no-op here
            StmtKind::Use(_) => {
                HirStmtKind::Expr(HirExpr::new(HirExprKind::Bool(false), HirType::Void))
            }
        };
        Ok(HirStmt { kind })
    }

    // -- expression lowering --------------------------------------------------

    fn lower_expr(&mut self, expr: &Expr) -> TResult<HirExpr> {
        match &expr.kind {
            ExprKind::Int(n) => Ok(HirExpr::new(HirExprKind::Int(*n), HirType::Int)),
            ExprKind::Float(n) => Ok(HirExpr::new(HirExprKind::Float(*n), HirType::Float)),
            ExprKind::Bool(b) => Ok(HirExpr::new(HirExprKind::Bool(*b), HirType::Bool)),
            ExprKind::Str(s) => {
                // Lower interpolated strings to StrConcat chains so codegen can handle them.
                // Pattern: "hello {name}!" → concat("hello ", name, "!")
                self.lower_str_interp(s, expr)
            }

            ExprKind::Ident(name) => {
                let ty = self
                    .env
                    .get(name)
                    .cloned()
                    .or_else(|| {
                        self.fn_sigs
                            .get(name)
                            .map(|(ps, r)| HirType::Fn(ps.clone(), Box::new(r.clone())))
                    })
                    .unwrap_or(HirType::Unknown);
                Ok(HirExpr::new(HirExprKind::Var(name.clone()), ty))
            }

            ExprKind::List(items) => {
                let hitems: TResult<Vec<_>> = items.iter().map(|e| self.lower_expr(e)).collect();
                let hitems = hitems?;
                let elem_ty = hitems
                    .first()
                    .map(|e| e.ty.clone())
                    .unwrap_or(HirType::Unknown);
                Ok(HirExpr::new(
                    HirExprKind::List(hitems),
                    HirType::List(Box::new(elem_ty)),
                ))
            }

            ExprKind::Tuple(items) => {
                let hitems: TResult<Vec<_>> = items.iter().map(|e| self.lower_expr(e)).collect();
                let hitems = hitems?;
                let tys = hitems.iter().map(|e| e.ty.clone()).collect();
                Ok(HirExpr::new(
                    HirExprKind::Tuple(hitems),
                    HirType::Tuple(tys),
                ))
            }

            ExprKind::Map(pairs) => {
                let mut hpairs = vec![];
                let mut key_ty = HirType::Unknown;
                let mut val_ty = HirType::Unknown;
                for (k, v) in pairs {
                    let hk = self.lower_expr(k)?;
                    let hv = self.lower_expr(v)?;
                    key_ty = hk.ty.clone();
                    val_ty = hv.ty.clone();
                    hpairs.push((hk, hv));
                }
                Ok(HirExpr::new(
                    HirExprKind::Map(hpairs),
                    HirType::Map(Box::new(key_ty), Box::new(val_ty)),
                ))
            }

            ExprKind::BinOp { op, lhs, rhs } => self.lower_binop(op, lhs, rhs),

            ExprKind::UnOp { op, expr } => {
                let hexpr = self.lower_expr(expr)?;
                let ty = match op {
                    UnOp::Neg => hexpr.ty.clone(),
                    UnOp::Not => HirType::Bool,
                };
                let hop = match op {
                    UnOp::Neg => HirUnOp::Neg,
                    UnOp::Not => HirUnOp::Not,
                };
                Ok(HirExpr::new(
                    HirExprKind::UnOp {
                        op: hop,
                        expr: Box::new(hexpr),
                    },
                    ty,
                ))
            }

            ExprKind::Call { callee, args } => self.lower_call(callee, args),

            ExprKind::Field { obj, field } => {
                let hobj = self.lower_expr(obj)?;
                let field_ty = self.resolve_field_type(&hobj.ty, field);
                Ok(HirExpr::new(
                    HirExprKind::Field {
                        obj: Box::new(hobj),
                        field: field.clone(),
                    },
                    field_ty,
                ))
            }

            ExprKind::SafeField { obj, field } => {
                let hobj = self.lower_expr(obj)?;
                let inner_ty = match &hobj.ty {
                    HirType::Option(t) => *t.clone(),
                    t => t.clone(),
                };
                let field_ty = self.resolve_field_type(&inner_ty, field);
                Ok(HirExpr::new(
                    HirExprKind::SafeField {
                        obj: Box::new(hobj),
                        field: field.clone(),
                    },
                    HirType::Option(Box::new(field_ty)),
                ))
            }

            ExprKind::Index { obj, index } => {
                let hobj = self.lower_expr(obj)?;
                let hindex = self.lower_expr(index)?;
                let elem_ty = match &hobj.ty {
                    HirType::List(t) => HirType::Option(t.clone()),
                    HirType::Map(_, v) => HirType::Option(v.clone()),
                    HirType::Str => HirType::Option(Box::new(HirType::Str)),
                    _ => HirType::Unknown,
                };
                Ok(HirExpr::new(
                    HirExprKind::Index {
                        obj: Box::new(hobj),
                        index: Box::new(hindex),
                    },
                    elem_ty,
                ))
            }

            // Desugar: a |> f(args)  →  Call(f, [a, args...])
            // Desugar: a |> f        →  Call(f, [a])
            ExprKind::Pipe { lhs, rhs } => {
                let hlhs = self.lower_expr(lhs)?;
                match &rhs.kind {
                    ExprKind::Call { callee, args } => {
                        let hcallee = self.lower_expr(callee)?;
                        let mut hargs = vec![hlhs];
                        for a in args {
                            hargs.push(self.lower_expr(a)?);
                        }
                        let ret_ty = self.call_return_type(&hcallee.ty, &hargs);
                        Ok(HirExpr::new(
                            HirExprKind::Call {
                                callee: Box::new(hcallee),
                                args: hargs,
                            },
                            ret_ty,
                        ))
                    }
                    _ => {
                        let hfn = self.lower_expr(rhs)?;
                        let ret_ty = self.call_return_type(&hfn.ty, std::slice::from_ref(&hlhs));
                        Ok(HirExpr::new(
                            HirExprKind::Call {
                                callee: Box::new(hfn),
                                args: vec![hlhs],
                            },
                            ret_ty,
                        ))
                    }
                }
            }

            // Desugar: a ?? b  →  UnwrapOr(a, b)
            ExprKind::NullCoalesce { lhs, rhs } => {
                let hlhs = self.lower_expr(lhs)?;
                let hrhs = self.lower_expr(rhs)?;
                let ty = match &hlhs.ty {
                    HirType::Option(t) => *t.clone(),
                    t => t.clone(),
                };
                Ok(HirExpr::new(
                    HirExprKind::UnwrapOr {
                        val: Box::new(hlhs),
                        default: Box::new(hrhs),
                    },
                    ty,
                ))
            }

            ExprKind::Propagate(inner) => {
                let hinner = self.lower_expr(inner)?;
                let ty = match &hinner.ty {
                    HirType::Result(t, _) => *t.clone(),
                    HirType::Option(t) => *t.clone(),
                    t => t.clone(),
                };
                Ok(HirExpr::new(
                    HirExprKind::PropagateErr(Box::new(hinner)),
                    ty,
                ))
            }

            ExprKind::Range { start, end } => {
                let _hs = self.lower_expr(start)?;
                let _he = self.lower_expr(end)?;
                // Range lowers to a list of ints
                Ok(HirExpr::new(
                    HirExprKind::List(vec![]),
                    HirType::List(Box::new(HirType::Int)),
                ))
            }

            // Lambda: lift to a named function with capture list
            ExprKind::Lambda { params, body } => {
                self.lambda_counter += 1;
                let fn_id = format!("__lambda_{}", self.lambda_counter);

                self.env.push();
                let hparams: Vec<HirParam> = params
                    .iter()
                    .map(|p| {
                        let ty = self.lower_ash_type(&p.ty);
                        self.env.define(&p.name, ty.clone());
                        HirParam {
                            name: p.name.clone(),
                            ty,
                            mutable: false,
                            borrow: false,
                        }
                    })
                    .collect();

                let hbody_expr = self.lower_expr(body)?;
                let ret = hbody_expr.ty.clone();
                self.env.pop();

                let hbody = HirBlock {
                    stmts: vec![HirStmt {
                        kind: HirStmtKind::Return(Some(hbody_expr)),
                    }],
                    ty: ret.clone(),
                };
                let fn_ty = HirType::Fn(
                    hparams.iter().map(|p| p.ty.clone()).collect(),
                    Box::new(ret),
                );
                let hfn = HirFn {
                    name: fn_id.clone(),
                    generics: vec![],
                    params: hparams,
                    ret: fn_ty.clone(),
                    body: hbody,
                    captures: vec![],
                };
                self.lifted.push(hfn);
                Ok(HirExpr::new(
                    HirExprKind::Closure {
                        fn_id,
                        captures: vec![],
                    },
                    fn_ty,
                ))
            }

            ExprKind::If { cond, then, else_ } => {
                let hcond = self.lower_expr(cond)?;
                let hthen = self.lower_block(then, &HirType::Unknown)?;
                let then_ty = hthen.ty.clone();
                let helse = if let Some(e) = else_ {
                    Some(Box::new(self.lower_expr(e)?))
                } else {
                    None
                };
                let ty = helse
                    .as_ref()
                    .map(|e| e.ty.clone())
                    .unwrap_or(then_ty.clone());
                Ok(HirExpr::new(
                    HirExprKind::If {
                        cond: Box::new(hcond),
                        then: Box::new(hthen),
                        else_: helse,
                    },
                    ty,
                ))
            }

            ExprKind::Block(block) => {
                let hblock = self.lower_block(block, &HirType::Unknown)?;
                let ty = hblock.ty.clone();
                Ok(HirExpr::new(HirExprKind::Block(Box::new(hblock)), ty))
            }

            ExprKind::Match { scrutinee, arms } => {
                let hscrutinee = self.lower_expr(scrutinee)?;
                let mut harms = vec![];
                let mut result_ty = HirType::Unknown;

                for arm in arms {
                    let (hpat, bindings) = self.lower_pattern(&arm.pattern, &hscrutinee.ty)?;
                    self.env.push();
                    for (name, ty) in &bindings {
                        self.env.define(name, ty.clone());
                    }
                    let hbody = self.lower_expr(&arm.body)?;
                    result_ty = hbody.ty.clone();
                    self.env.pop();
                    harms.push(HirArm {
                        pattern: hpat,
                        bindings,
                        body: hbody,
                    });
                }

                Ok(HirExpr::new(
                    HirExprKind::Match {
                        scrutinee: Box::new(hscrutinee),
                        arms: harms,
                    },
                    result_ty,
                ))
            }

            ExprKind::Await(inner) => {
                let hinner = self.lower_expr(inner)?;
                let ty = hinner.ty.clone();
                Ok(HirExpr::new(HirExprKind::Await(Box::new(hinner)), ty))
            }

            ExprKind::Borrow(inner) | ExprKind::Move(inner) => self.lower_expr(inner),

            ExprKind::StructLit { name, fields } => {
                let mut hfields = vec![];
                for (fname, fexpr) in fields {
                    let hval = self.lower_expr(fexpr)?;
                    hfields.push((fname.clone(), hval));
                }
                let ty = HirType::Struct(name.clone());
                Ok(HirExpr::new(
                    HirExprKind::Call {
                        callee: Box::new(HirExpr::new(HirExprKind::Var(name.clone()), ty.clone())),
                        args: hfields.into_iter().map(|(_, v)| v).collect(),
                    },
                    ty,
                ))
            }
        }
    }

    // -- string interpolation lowering ----------------------------------------

    /// Lower an interpolated string like "hello {name}!" to a chain of StrConcat ops.
    /// If the string has no `{...}` markers it returns a plain Str node.
    fn lower_str_interp(&mut self, s: &str, origin: &Expr) -> TResult<HirExpr> {
        // Quick check — if no `{` in the string, emit as plain string constant
        if !s.contains('{') {
            return Ok(HirExpr::new(HirExprKind::Str(s.to_string()), HirType::Str));
        }

        // Split the template into alternating literal and expression segments
        let mut segments: Vec<HirExpr> = vec![];
        let chars: Vec<char> = s.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            if chars[i] == '{' {
                // Find matching `}`
                let start = i + 1;
                let end = match chars[start..].iter().position(|&c| c == '}') {
                    Some(p) => start + p,
                    None => {
                        // Unclosed brace — emit rest as literal
                        let rest: String = chars[i..].iter().collect();
                        segments.push(HirExpr::new(HirExprKind::Str(rest), HirType::Str));
                        break;
                    }
                };
                let expr_src: String = chars[start..end].iter().collect();
                let expr_src = expr_src.trim().to_string();

                if expr_src.is_empty() {
                    // `{}` — literal placeholder, emit as-is
                    segments.push(HirExpr::new(HirExprKind::Str("{}".into()), HirType::Str));
                } else if expr_src.chars().all(|c| c.is_alphanumeric() || c == '_') {
                    // Simple identifier — lower as a Var with toString coercion
                    let ty = self
                        .env
                        .get(&expr_src)
                        .cloned()
                        .or_else(|| {
                            self.fn_sigs
                                .get(&expr_src)
                                .map(|(ps, r)| HirType::Fn(ps.clone(), Box::new(r.clone())))
                        })
                        .unwrap_or(HirType::Unknown);
                    let var = HirExpr::new(HirExprKind::Var(expr_src), ty.clone());
                    // Wrap in a str cast if needed (represented as identity for now — codegen handles via value_to_str)
                    segments.push(var);
                } else {
                    // Complex expression — parse and lower it
                    let token_res = ash_lexer::Lexer::new(&expr_src).tokenize();
                    match token_res {
                        Ok(toks) => {
                            match ash_parser::parse_expr_from_tokens(toks) {
                                Ok(inner_expr) => {
                                    match self.lower_expr(&inner_expr) {
                                        Ok(he) => segments.push(he),
                                        Err(_) => {
                                            // Fall back to emitting as literal
                                            segments.push(HirExpr::new(
                                                HirExprKind::Str(format!("{{{expr_src}}}")),
                                                HirType::Str,
                                            ));
                                        }
                                    }
                                }
                                Err(_) => {
                                    segments.push(HirExpr::new(
                                        HirExprKind::Str(format!("{{{expr_src}}}")),
                                        HirType::Str,
                                    ));
                                }
                            }
                        }
                        Err(_) => {
                            segments.push(HirExpr::new(
                                HirExprKind::Str(format!("{{{expr_src}}}")),
                                HirType::Str,
                            ));
                        }
                    }
                }
                i = end + 1;
            } else {
                // Collect literal segment
                let mut lit = String::new();
                while i < chars.len() && chars[i] != '{' {
                    lit.push(chars[i]);
                    i += 1;
                }
                if !lit.is_empty() {
                    segments.push(HirExpr::new(HirExprKind::Str(lit), HirType::Str));
                }
            }
        }

        if segments.is_empty() {
            return Ok(HirExpr::new(HirExprKind::Str(String::new()), HirType::Str));
        }
        if segments.len() == 1 {
            return Ok(segments.remove(0));
        }

        // Build left-associative StrConcat chain
        let _ = origin; // suppress unused
        let mut acc = segments.remove(0);
        for seg in segments {
            acc = HirExpr::new(
                HirExprKind::BinOp {
                    op: HirBinOp::StrConcat,
                    lhs: Box::new(acc),
                    rhs: Box::new(seg),
                },
                HirType::Str,
            );
        }
        Ok(acc)
    }

    // -- binary op ------------------------------------------------------------

    fn lower_binop(&mut self, op: &BinOp, lhs: &Expr, rhs: &Expr) -> TResult<HirExpr> {
        let hlhs = self.lower_expr(lhs)?;
        let hrhs = self.lower_expr(rhs)?;
        let (hop, ty) = match op {
            BinOp::Add => {
                // String + String = StrConcat
                if hlhs.ty == HirType::Str || hrhs.ty == HirType::Str {
                    (HirBinOp::StrConcat, HirType::Str)
                } else if hlhs.ty == HirType::Float || hrhs.ty == HirType::Float {
                    (HirBinOp::Add, HirType::Float)
                } else {
                    (HirBinOp::Add, HirType::Int)
                }
            }
            BinOp::Sub => (
                HirBinOp::Sub,
                if hlhs.ty == HirType::Float {
                    HirType::Float
                } else {
                    HirType::Int
                },
            ),
            BinOp::Mul => (
                HirBinOp::Mul,
                if hlhs.ty == HirType::Float {
                    HirType::Float
                } else {
                    HirType::Int
                },
            ),
            BinOp::Div => (
                HirBinOp::Div,
                if hlhs.ty == HirType::Float {
                    HirType::Float
                } else {
                    HirType::Int
                },
            ),
            BinOp::Mod => (HirBinOp::Mod, HirType::Int),
            BinOp::Eq => (HirBinOp::Eq, HirType::Bool),
            BinOp::NotEq => (HirBinOp::NotEq, HirType::Bool),
            BinOp::Lt => (HirBinOp::Lt, HirType::Bool),
            BinOp::Gt => (HirBinOp::Gt, HirType::Bool),
            BinOp::LtEq => (HirBinOp::LtEq, HirType::Bool),
            BinOp::GtEq => (HirBinOp::GtEq, HirType::Bool),
            BinOp::And => (HirBinOp::And, HirType::Bool),
            BinOp::Or => (HirBinOp::Or, HirType::Bool),
        };
        Ok(HirExpr::new(
            HirExprKind::BinOp {
                op: hop,
                lhs: Box::new(hlhs),
                rhs: Box::new(hrhs),
            },
            ty,
        ))
    }

    // -- call lowering --------------------------------------------------------

    fn lower_call(&mut self, callee: &Expr, args: &[Expr]) -> TResult<HirExpr> {
        // Method call: obj.method(args)
        if let ExprKind::Field { obj, field } = &callee.kind {
            let hobj = self.lower_expr(obj)?;
            let hargs: Vec<HirExpr> = args
                .iter()
                .map(|a| self.lower_expr(a))
                .collect::<TResult<_>>()?;
            let ret_ty = self.method_return_type(&hobj.ty, field, &hargs);
            // Build: obj.method as Field, then Call
            let field_ty = HirType::Fn(
                std::iter::once(hobj.ty.clone())
                    .chain(hargs.iter().map(|a| a.ty.clone()))
                    .collect(),
                Box::new(ret_ty.clone()),
            );
            let field_expr = HirExpr::new(
                HirExprKind::Field {
                    obj: Box::new(hobj),
                    field: field.clone(),
                },
                field_ty,
            );
            return Ok(HirExpr::new(
                HirExprKind::Call {
                    callee: Box::new(field_expr),
                    args: hargs,
                },
                ret_ty,
            ));
        }

        let hcallee = self.lower_expr(callee)?;
        let hargs: Vec<HirExpr> = args
            .iter()
            .map(|a| self.lower_expr(a))
            .collect::<TResult<_>>()?;
        let ret_ty = self.call_return_type(&hcallee.ty, &hargs);
        Ok(HirExpr::new(
            HirExprKind::Call {
                callee: Box::new(hcallee),
                args: hargs,
            },
            ret_ty,
        ))
    }

    fn call_return_type(&self, callee_ty: &HirType, _args: &[HirExpr]) -> HirType {
        match callee_ty {
            HirType::Fn(_, ret) => *ret.clone(),
            _ => HirType::Unknown,
        }
    }

    fn method_return_type(&self, obj_ty: &HirType, method: &str, args: &[HirExpr]) -> HirType {
        match (obj_ty, method) {
            (HirType::Str, "len") => HirType::Int,
            (HirType::Str, "upper") => HirType::Str,
            (HirType::Str, "lower") => HirType::Str,
            (HirType::Str, "trim") => HirType::Str,
            (HirType::Str, "split") => HirType::List(Box::new(HirType::Str)),
            (HirType::Str, "contains") => HirType::Bool,
            (HirType::Str, "starts") => HirType::Bool,
            (HirType::Str, "ends") => HirType::Bool,
            (HirType::Str, "replace") => HirType::Str,
            (HirType::Str, "find") => HirType::Option(Box::new(HirType::Int)),
            (HirType::List(_), "len") => HirType::Int,
            (HirType::List(t), "first") => HirType::Option(t.clone()),
            (HirType::List(t), "last") => HirType::Option(t.clone()),
            (HirType::List(t), "reverse") => HirType::List(t.clone()),
            (HirType::List(t), "sort") => HirType::List(t.clone()),
            (HirType::List(_), "contains") => HirType::Bool,
            (HirType::List(t), "filter") => HirType::List(t.clone()),
            (HirType::List(t), "map") => {
                // Return type depends on the fn arg
                if let Some(f) = args.first() {
                    if let HirType::Fn(_, ret) = &f.ty {
                        return HirType::List(ret.clone());
                    }
                }
                HirType::List(Box::new(HirType::Unknown))
            }
            (HirType::List(_), "reduce") => args
                .get(1)
                .map(|a| a.ty.clone())
                .unwrap_or(HirType::Unknown),
            (HirType::Map(_, v), "get") => HirType::Option(v.clone()),
            (HirType::Map(_, _), "has") => HirType::Bool,
            (HirType::Map(k, _), "keys") => HirType::List(k.clone()),
            (HirType::Map(_, v), "vals") => HirType::List(v.clone()),
            (HirType::Map(_, _), "len") => HirType::Int,
            _ => HirType::Unknown,
        }
    }

    fn resolve_field_type(&self, obj_ty: &HirType, field: &str) -> HirType {
        match obj_ty {
            HirType::Struct(name) => self
                .registry
                .field_type(name, field)
                .cloned()
                .unwrap_or(HirType::Unknown),
            _ => self.method_return_type(obj_ty, field, &[]),
        }
    }

    // -- pattern lowering -----------------------------------------------------

    fn lower_pattern(
        &self,
        pat: &Pattern,
        scrutinee_ty: &HirType,
    ) -> TResult<(HirPattern, Vec<(String, HirType)>)> {
        match pat {
            Pattern::Wildcard => Ok((HirPattern::Wildcard, vec![])),

            Pattern::Ident(name) => Ok((
                HirPattern::Var(name.clone(), scrutinee_ty.clone()),
                vec![(name.clone(), scrutinee_ty.clone())],
            )),

            Pattern::Literal(lit) => {
                let hlit = match lit {
                    LitPattern::Int(n) => HirLitPat::Int(*n),
                    LitPattern::Float(f) => HirLitPat::Float(*f),
                    LitPattern::Str(s) => HirLitPat::Str(s.clone()),
                    LitPattern::Bool(b) => HirLitPat::Bool(*b),
                };
                Ok((HirPattern::Lit(hlit), vec![]))
            }

            Pattern::Variant(name, inner_pats) => {
                // Look up fields of this variant
                let field_types = self.find_variant_fields(name, scrutinee_ty);
                let mut bindings = vec![];
                let mut hpats = vec![];
                for (i, p) in inner_pats.iter().enumerate() {
                    let field_ty = field_types.get(i).cloned().unwrap_or(HirType::Unknown);
                    let (hp, mut b) = self.lower_pattern(p, &field_ty)?;
                    bindings.append(&mut b);
                    hpats.push(hp);
                }
                Ok((HirPattern::Variant(name.clone(), hpats), bindings))
            }

            Pattern::Tuple(pats) => {
                let elem_tys = match scrutinee_ty {
                    HirType::Tuple(ts) => ts.clone(),
                    _ => vec![HirType::Unknown; pats.len()],
                };
                let mut bindings = vec![];
                let mut hpats = vec![];
                for (p, ty) in pats.iter().zip(elem_tys.iter()) {
                    let (hp, mut b) = self.lower_pattern(p, ty)?;
                    bindings.append(&mut b);
                    hpats.push(hp);
                }
                Ok((HirPattern::Tuple(hpats), bindings))
            }

            Pattern::Struct(name, field_pats) => {
                let mut bindings = vec![];
                let mut hfield_pats = vec![];
                for (fname, p) in field_pats {
                    let fty = self
                        .registry
                        .field_type(name, fname)
                        .cloned()
                        .unwrap_or(HirType::Unknown);
                    let (hp, mut b) = self.lower_pattern(p, &fty)?;
                    bindings.append(&mut b);
                    hfield_pats.push((fname.clone(), hp));
                }
                Ok((HirPattern::Struct(name.clone(), hfield_pats), bindings))
            }
        }
    }

    fn find_variant_fields(&self, variant_name: &str, scrutinee_ty: &HirType) -> Vec<HirType> {
        // Try to find from union name
        let union_name = match scrutinee_ty {
            HirType::Union(n) => Some(n.as_str()),
            _ => None,
        };
        if let Some(uname) = union_name {
            if let Some(fields) = self.registry.variant_fields(uname, variant_name) {
                return fields.clone();
            }
        }
        // Search all unions
        for variants in self.registry.unions.values() {
            for v in variants {
                if v.name == variant_name {
                    return v.fields.clone();
                }
            }
        }
        vec![]
    }

    // -- type def lowering ----------------------------------------------------

    fn lower_type_def(&self, td: &TypeDef) -> TResult<HirTypeDef> {
        let kind = match &td.kind {
            TypeDefKind::Struct(fields) => HirTypeDefKind::Struct(
                fields
                    .iter()
                    .map(|f| HirField {
                        name: f.name.clone(),
                        ty: self.lower_ash_type(&f.ty),
                    })
                    .collect(),
            ),
            TypeDefKind::Union(variants) => HirTypeDefKind::Union(
                variants
                    .iter()
                    .map(|v| HirVariant {
                        name: v.name.clone(),
                        fields: v.fields.iter().map(|t| self.lower_ash_type(t)).collect(),
                    })
                    .collect(),
            ),
        };
        Ok(HirTypeDef {
            name: td.name.clone(),
            generics: td.generics.clone(),
            kind,
        })
    }
}

// --- Public API ---------------------------------------------------------------

pub fn check(program: &Program) -> TResult<HirProgram> {
    TypeChecker::new().check(program)
}

/// A structured type error diagnostic with location information.
pub struct Diagnostic {
    pub msg: String,
    pub line: usize,
    pub col: usize,
}

/// Run the type checker and return all diagnostics.
/// For now a single error is returned if type checking fails;
/// line/col come from the span if available.
pub fn check_with_diagnostics(program: &Program) -> Vec<Diagnostic> {
    match TypeChecker::new().check(program) {
        Ok(_) => vec![],
        Err(e) => {
            let (line, col) = e.span.map(|s| (s.line, s.col)).unwrap_or((0, 0));
            vec![Diagnostic {
                msg: e.msg,
                line,
                col,
            }]
        }
    }
}

// --- Helpers -----------------------------------------------------------------

#[allow(dead_code)]
trait HirTypeNamed {
    fn named(n: &str) -> HirType;
}
impl HirTypeNamed for HirType {
    fn named(n: &str) -> HirType {
        HirType::Struct(n.to_string())
    }
}

// --- Tests --------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use ash_lexer::Lexer;
    use ash_parser::parse;

    fn typecheck(src: &str) -> HirProgram {
        let tokens = Lexer::new(src).tokenize().expect("lex");
        let program = parse(tokens).expect("parse");
        check(&program).expect("typecheck")
    }

    fn typecheck_fails(src: &str) -> bool {
        let tokens = Lexer::new(src).tokenize().expect("lex");
        let program = parse(tokens).expect("parse");
        check(&program).is_err()
    }

    #[test]
    fn test_int_literal() {
        let hir = typecheck("42");
        if let HirStmtKind::Expr(e) = &hir.top_stmts[0].kind {
            assert_eq!(e.ty, HirType::Int);
        }
    }

    #[test]
    fn test_float_literal() {
        let hir = typecheck("3.14");
        if let HirStmtKind::Expr(e) = &hir.top_stmts[0].kind {
            assert_eq!(e.ty, HirType::Float);
        }
    }

    #[test]
    fn test_bool_literal() {
        let hir = typecheck("true");
        if let HirStmtKind::Expr(e) = &hir.top_stmts[0].kind {
            assert_eq!(e.ty, HirType::Bool);
        }
    }

    #[test]
    fn test_str_literal() {
        let hir = typecheck("\"hello\"");
        if let HirStmtKind::Expr(e) = &hir.top_stmts[0].kind {
            assert_eq!(e.ty, HirType::Str);
        }
    }

    #[test]
    fn test_int_add() {
        let hir = typecheck("1 + 2");
        if let HirStmtKind::Expr(e) = &hir.top_stmts[0].kind {
            assert_eq!(e.ty, HirType::Int);
        }
    }

    #[test]
    fn test_float_add() {
        let hir = typecheck("1.0 + 2.0");
        if let HirStmtKind::Expr(e) = &hir.top_stmts[0].kind {
            assert_eq!(e.ty, HirType::Float);
        }
    }

    #[test]
    fn test_str_concat() {
        let hir = typecheck("\"a\" + \"b\"");
        if let HirStmtKind::Expr(e) = &hir.top_stmts[0].kind {
            assert_eq!(e.ty, HirType::Str);
            if let HirExprKind::BinOp { op, .. } = &e.kind {
                assert_eq!(*op, HirBinOp::StrConcat);
            }
        }
    }

    #[test]
    fn test_comparison_is_bool() {
        let hir = typecheck("1 < 2");
        if let HirStmtKind::Expr(e) = &hir.top_stmts[0].kind {
            assert_eq!(e.ty, HirType::Bool);
        }
    }

    #[test]
    fn test_let_infer_int() {
        let hir = typecheck("let x = 42");
        if let HirStmtKind::Let { ty, .. } = &hir.top_stmts[0].kind {
            assert_eq!(*ty, HirType::Int);
        }
    }

    #[test]
    fn test_let_explicit_type() {
        let hir = typecheck("let x:float = 1.0");
        if let HirStmtKind::Let { ty, .. } = &hir.top_stmts[0].kind {
            assert_eq!(*ty, HirType::Float);
        }
    }

    #[test]
    fn test_fn_def_typed() {
        let hir = typecheck("fn add(a:int b:int):int\n    a + b");
        assert_eq!(hir.fns.len(), 1);
        assert_eq!(hir.fns[0].ret, HirType::Int);
        assert_eq!(hir.fns[0].params[0].ty, HirType::Int);
    }

    #[test]
    fn test_fn_inferred_params() {
        let hir = typecheck("fn greet(name)\n    name");
        assert_eq!(hir.fns.len(), 1);
    }

    #[test]
    fn test_if_expr_bool_cond() {
        let hir = typecheck("if true\n    1\nelse\n    0");
        // Should not fail — condition is bool
        assert!(!hir.top_stmts.is_empty());
    }

    #[test]
    fn test_list_literal_typed() {
        let hir = typecheck("[1, 2, 3]");
        if let HirStmtKind::Expr(e) = &hir.top_stmts[0].kind {
            assert_eq!(e.ty, HirType::List(Box::new(HirType::Int)));
        }
    }

    #[test]
    fn test_option_type() {
        let hir = typecheck("let x:?int = 42");
        if let HirStmtKind::Let { ty, .. } = &hir.top_stmts[0].kind {
            assert_eq!(*ty, HirType::Option(Box::new(HirType::Int)));
        }
    }

    #[test]
    fn test_pipe_desugared_to_call() {
        let hir = typecheck("fn double(x:int):int\n    x * 2\n5 |> double");
        // The pipe should be in top_stmts as a Call, not a Pipe
        let last = hir.top_stmts.last().unwrap();
        if let HirStmtKind::Expr(e) = &last.kind {
            assert!(
                matches!(e.kind, HirExprKind::Call { .. }),
                "pipe should desugar to Call"
            );
        }
    }

    #[test]
    fn test_null_coalesce_desugared() {
        let hir = typecheck("let x:?int = 42\nx ?? 0");
        let last = hir.top_stmts.last().unwrap();
        if let HirStmtKind::Expr(e) = &last.kind {
            assert!(
                matches!(e.kind, HirExprKind::UnwrapOr { .. }),
                "?? should desugar to UnwrapOr"
            );
        }
    }

    #[test]
    fn test_lambda_lifted() {
        let hir = typecheck("f = x => x + 1");
        // Lambda should be lifted into hir.lifted
        assert!(!hir.lifted.is_empty(), "lambda should be lifted");
    }

    #[test]
    fn test_type_def_struct() {
        let hir = typecheck("type Point\n    x:float\n    y:float");
        assert_eq!(hir.types.len(), 1);
        assert_eq!(hir.types[0].name, "Point");
        assert!(matches!(hir.types[0].kind, HirTypeDefKind::Struct(_)));
    }

    #[test]
    fn test_type_def_union() {
        let hir = typecheck("type Color = Red | Green | Blue");
        assert_eq!(hir.types.len(), 1);
        assert!(matches!(hir.types[0].kind, HirTypeDefKind::Union(_)));
    }

    #[test]
    fn test_match_arm_bindings() {
        let src = "match 42\n    x => x + 1";
        let hir = typecheck(src);
        if let HirStmtKind::Expr(e) = &hir.top_stmts[0].kind {
            if let HirExprKind::Match { arms, .. } = &e.kind {
                assert!(!arms[0].bindings.is_empty());
            }
        }
    }

    #[test]
    fn test_str_len_method_type() {
        let hir = typecheck("\"hello\".len()");
        if let HirStmtKind::Expr(e) = &hir.top_stmts[0].kind {
            assert_eq!(e.ty, HirType::Int);
        }
    }

    #[test]
    fn test_list_filter_preserves_elem_type() {
        let hir = typecheck("[1, 2, 3].filter(x => x > 1)");
        if let HirStmtKind::Expr(e) = &hir.top_stmts[0].kind {
            assert_eq!(e.ty, HirType::List(Box::new(HirType::Int)));
        }
    }

    #[test]
    fn test_type_mismatch_explicit() {
        // let x:bool = 42  — should fail
        assert!(typecheck_fails("let x:bool = 42"));
    }

    #[test]
    fn test_for_loop_var_type() {
        let hir = typecheck("for x in [1, 2, 3]\n    println(x)");
        if let HirStmtKind::For { var_ty, .. } = &hir.top_stmts[0].kind {
            assert_eq!(*var_ty, HirType::Int);
        }
    }

    #[test]
    fn test_multiple_fns_registered() {
        let hir = typecheck("fn square(x:int):int\n    x*x\nfn cube(x:int):int\n    x*square(x)");
        assert_eq!(hir.fns.len(), 2);
    }
}
