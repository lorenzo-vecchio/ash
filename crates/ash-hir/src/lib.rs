//! ash-hir — fully-typed, desugared intermediate representation

use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum HirType {
    Int,
    Float,
    Bool,
    Str,
    Void,
    Option(Box<HirType>),
    Result(Box<HirType>, Box<HirType>),
    List(Box<HirType>),
    Map(Box<HirType>, Box<HirType>),
    Tuple(Vec<HirType>),
    Fn(Vec<HirType>, Box<HirType>),
    Struct(String),
    Union(String),
    Generic(String),
    Unknown,
}

impl std::fmt::Display for HirType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HirType::Int => write!(f, "int"),
            HirType::Float => write!(f, "float"),
            HirType::Bool => write!(f, "bool"),
            HirType::Str => write!(f, "str"),
            HirType::Void => write!(f, "void"),
            HirType::Unknown => write!(f, "?"),
            HirType::Option(t) => write!(f, "?{t}"),
            HirType::Result(t, e) => write!(f, "Result[{t} {e}]"),
            HirType::List(t) => write!(f, "[{t}]"),
            HirType::Map(k, v) => write!(f, "{{{k}: {v}}}"),
            HirType::Tuple(ts) => {
                let s: Vec<_> = ts.iter().map(|t| t.to_string()).collect();
                write!(f, "({})", s.join(" "))
            }
            HirType::Fn(ps, r) => {
                let s: Vec<_> = ps.iter().map(|t| t.to_string()).collect();
                write!(f, "({}) => {r}", s.join(" "))
            }
            HirType::Struct(n) | HirType::Union(n) | HirType::Generic(n) => write!(f, "{n}"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct HirExpr {
    pub kind: HirExprKind,
    pub ty: HirType,
}
impl HirExpr {
    pub fn new(kind: HirExprKind, ty: HirType) -> Self {
        HirExpr { kind, ty }
    }
}

#[derive(Debug, Clone)]
pub enum HirExprKind {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(String),
    List(Vec<HirExpr>),
    Map(Vec<(HirExpr, HirExpr)>),
    Tuple(Vec<HirExpr>),
    Var(String),
    BinOp {
        op: HirBinOp,
        lhs: Box<HirExpr>,
        rhs: Box<HirExpr>,
    },
    UnOp {
        op: HirUnOp,
        expr: Box<HirExpr>,
    },
    Call {
        callee: Box<HirExpr>,
        args: Vec<HirExpr>,
    },
    Field {
        obj: Box<HirExpr>,
        field: String,
    },
    SafeField {
        obj: Box<HirExpr>,
        field: String,
    },
    Index {
        obj: Box<HirExpr>,
        index: Box<HirExpr>,
    },
    If {
        cond: Box<HirExpr>,
        then: Box<HirBlock>,
        else_: Option<Box<HirExpr>>,
    },
    Match {
        scrutinee: Box<HirExpr>,
        arms: Vec<HirArm>,
    },
    Block(Box<HirBlock>),
    PropagateErr(Box<HirExpr>),
    UnwrapOr {
        val: Box<HirExpr>,
        default: Box<HirExpr>,
    },
    Closure {
        fn_id: String,
        captures: Vec<String>,
    },
    Await(Box<HirExpr>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum HirBinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    NotEq,
    Lt,
    Gt,
    LtEq,
    GtEq,
    And,
    Or,
    StrConcat,
}

#[derive(Debug, Clone, PartialEq)]
pub enum HirUnOp {
    Neg,
    Not,
}

#[derive(Debug, Clone)]
pub struct HirStmt {
    pub kind: HirStmtKind,
}

#[derive(Debug, Clone)]
pub enum HirStmtKind {
    Let {
        name: String,
        ty: HirType,
        mutable: bool,
        value: HirExpr,
    },
    Assign {
        target: HirExpr,
        value: HirExpr,
    },
    Return(Option<HirExpr>),
    While {
        cond: HirExpr,
        body: HirBlock,
    },
    For {
        var: String,
        var_ty: HirType,
        iter: HirExpr,
        body: HirBlock,
    },
    Panic(HirExpr),
    Expr(HirExpr),
}

#[derive(Debug, Clone)]
pub struct HirBlock {
    pub stmts: Vec<HirStmt>,
    pub ty: HirType,
}

#[derive(Debug, Clone)]
pub struct HirParam {
    pub name: String,
    pub ty: HirType,
    pub mutable: bool,
    pub borrow: bool,
}

#[derive(Debug, Clone)]
pub struct HirFn {
    pub name: String,
    pub generics: Vec<String>,
    pub params: Vec<HirParam>,
    pub ret: HirType,
    pub body: HirBlock,
    pub captures: Vec<(String, HirType)>,
}

#[derive(Debug, Clone)]
pub struct HirTypeDef {
    pub name: String,
    pub generics: Vec<String>,
    pub kind: HirTypeDefKind,
}

#[derive(Debug, Clone)]
pub enum HirTypeDefKind {
    Struct(Vec<HirField>),
    Union(Vec<HirVariant>),
}

#[derive(Debug, Clone)]
pub struct HirField {
    pub name: String,
    pub ty: HirType,
}
#[derive(Debug, Clone)]
pub struct HirVariant {
    pub name: String,
    pub fields: Vec<HirType>,
}

#[derive(Debug, Clone)]
pub struct HirArm {
    pub pattern: HirPattern,
    pub bindings: Vec<(String, HirType)>,
    pub body: HirExpr,
}

#[derive(Debug, Clone)]
pub enum HirPattern {
    Wildcard,
    Var(String, HirType),
    Lit(HirLitPat),
    Variant(String, Vec<HirPattern>),
    Tuple(Vec<HirPattern>),
    Struct(String, Vec<(String, HirPattern)>),
}

#[derive(Debug, Clone)]
pub enum HirLitPat {
    Int(i64),
    Float(f64),
    Str(String),
    Bool(bool),
}

#[derive(Debug, Clone)]
pub struct HirProgram {
    pub fns: Vec<HirFn>,
    pub types: Vec<HirTypeDef>,
    pub top_stmts: Vec<HirStmt>,
    pub lifted: Vec<HirFn>,
}

// ─── Type environment ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct TypeEnv {
    scopes: Vec<HashMap<String, HirType>>,
}

impl TypeEnv {
    pub fn new() -> Self {
        TypeEnv {
            scopes: vec![HashMap::new()],
        }
    }
    pub fn push(&mut self) {
        self.scopes.push(HashMap::new());
    }
    pub fn pop(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }
    pub fn define(&mut self, name: &str, ty: HirType) {
        self.scopes.last_mut().unwrap().insert(name.to_string(), ty);
    }
    pub fn get(&self, name: &str) -> Option<&HirType> {
        for s in self.scopes.iter().rev() {
            if let Some(t) = s.get(name) {
                return Some(t);
            }
        }
        None
    }
}

// ─── Type registry ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct TypeRegistry {
    pub structs: HashMap<String, Vec<HirField>>,
    pub unions: HashMap<String, Vec<HirVariant>>,
}

impl TypeRegistry {
    pub fn new() -> Self {
        TypeRegistry::default()
    }
    pub fn register_struct(&mut self, name: &str, fields: Vec<HirField>) {
        self.structs.insert(name.to_string(), fields);
    }
    pub fn register_union(&mut self, name: &str, variants: Vec<HirVariant>) {
        self.unions.insert(name.to_string(), variants);
    }
    pub fn field_type(&self, sname: &str, field: &str) -> Option<&HirType> {
        self.structs
            .get(sname)?
            .iter()
            .find(|f| f.name == field)
            .map(|f| &f.ty)
    }
    pub fn variant_fields(&self, uname: &str, variant: &str) -> Option<&Vec<HirType>> {
        self.unions
            .get(uname)?
            .iter()
            .find(|v| v.name == variant)
            .map(|v| &v.fields)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_env_scoping() {
        let mut env = TypeEnv::new();
        env.define("x", HirType::Int);
        assert_eq!(env.get("x"), Some(&HirType::Int));
        env.push();
        env.define("x", HirType::Str);
        assert_eq!(env.get("x"), Some(&HirType::Str));
        env.pop();
        assert_eq!(env.get("x"), Some(&HirType::Int));
    }

    #[test]
    fn test_type_env_nested_lookup() {
        let mut env = TypeEnv::new();
        env.define("outer", HirType::Bool);
        env.push();
        env.define("inner", HirType::Float);
        assert_eq!(env.get("outer"), Some(&HirType::Bool));
        assert_eq!(env.get("inner"), Some(&HirType::Float));
        assert!(env.get("missing").is_none());
    }

    #[test]
    fn test_type_registry_struct() {
        let mut reg = TypeRegistry::new();
        reg.register_struct(
            "Point",
            vec![
                HirField {
                    name: "x".into(),
                    ty: HirType::Float,
                },
                HirField {
                    name: "y".into(),
                    ty: HirType::Float,
                },
            ],
        );
        assert_eq!(reg.field_type("Point", "x"), Some(&HirType::Float));
        assert!(reg.field_type("Point", "z").is_none());
    }

    #[test]
    fn test_type_registry_union() {
        let mut reg = TypeRegistry::new();
        reg.register_union(
            "Shape",
            vec![
                HirVariant {
                    name: "Circle".into(),
                    fields: vec![HirType::Float],
                },
                HirVariant {
                    name: "Rect".into(),
                    fields: vec![HirType::Float, HirType::Float],
                },
            ],
        );
        assert_eq!(
            reg.variant_fields("Shape", "Circle").map(|v| v.len()),
            Some(1)
        );
        assert_eq!(
            reg.variant_fields("Shape", "Rect").map(|v| v.len()),
            Some(2)
        );
        assert!(reg.variant_fields("Shape", "None").is_none());
    }

    #[test]
    fn test_hir_type_display() {
        assert_eq!(HirType::Int.to_string(), "int");
        assert_eq!(HirType::Option(Box::new(HirType::Int)).to_string(), "?int");
        assert_eq!(HirType::List(Box::new(HirType::Str)).to_string(), "[str]");
        assert_eq!(
            HirType::Fn(vec![HirType::Int], Box::new(HirType::Bool)).to_string(),
            "(int) => bool"
        );
    }

    #[test]
    fn test_hir_expr_int() {
        let e = HirExpr::new(HirExprKind::Int(42), HirType::Int);
        assert!(matches!(e.kind, HirExprKind::Int(42)));
        assert_eq!(e.ty, HirType::Int);
    }

    #[test]
    fn test_hir_binop_construction() {
        let l = HirExpr::new(HirExprKind::Int(1), HirType::Int);
        let r = HirExpr::new(HirExprKind::Int(2), HirType::Int);
        let e = HirExpr::new(
            HirExprKind::BinOp {
                op: HirBinOp::Add,
                lhs: Box::new(l),
                rhs: Box::new(r),
            },
            HirType::Int,
        );
        assert!(matches!(
            e.kind,
            HirExprKind::BinOp {
                op: HirBinOp::Add,
                ..
            }
        ));
    }

    #[test]
    fn test_hir_block_void() {
        let b = HirBlock {
            stmts: vec![],
            ty: HirType::Void,
        };
        assert_eq!(b.ty, HirType::Void);
    }

    #[test]
    fn test_hir_fn_construction() {
        let f = HirFn {
            name: "add".into(),
            generics: vec![],
            params: vec![
                HirParam {
                    name: "a".into(),
                    ty: HirType::Int,
                    mutable: false,
                    borrow: false,
                },
                HirParam {
                    name: "b".into(),
                    ty: HirType::Int,
                    mutable: false,
                    borrow: false,
                },
            ],
            ret: HirType::Int,
            body: HirBlock {
                stmts: vec![],
                ty: HirType::Int,
            },
            captures: vec![],
        };
        assert_eq!(f.params.len(), 2);
        assert_eq!(f.ret, HirType::Int);
    }

    #[test]
    fn test_hir_program_empty() {
        let p = HirProgram {
            fns: vec![],
            types: vec![],
            top_stmts: vec![],
            lifted: vec![],
        };
        assert!(p.fns.is_empty() && p.lifted.is_empty());
    }
}
