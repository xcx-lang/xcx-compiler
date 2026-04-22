use crate::lexer::token::{TokenKind, Span};
use crate::sema::interner::StringId;

#[derive(Debug, Clone, PartialEq)]
pub enum SetType {
    N, // Natural
    Q, // Rational
    Z, // Integers
    S, // Strings
    B, // Booleans
    C, // Chars
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForIterType {
    Range,
    Array,
    Set,
    Fiber,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Argument {
    Positional(Expr),
    Named(StringId, Expr),
}

impl Argument {
    pub fn expr(&self) -> &Expr {
        match self {
            Argument::Positional(e) => e,
            Argument::Named(_, e) => e,
        }
    }

    pub fn expr_mut(&mut self) -> &mut Expr {
        match self {
            Argument::Positional(e) => e,
            Argument::Named(_, e) => e,
        }
    }
}


#[derive(Debug, Clone, PartialEq)]
pub enum ColumnAttribute {
    Auto,
    PrimaryKey,
    Unique,
    Optional,
    Default(Expr),
    ForeignKey(StringId, StringId), // Table, Column
}

#[derive(Debug, Clone, PartialEq)]
pub struct ColumnDef {
    pub name: StringId,
    pub ty: Type,
    pub attributes: Vec<ColumnAttribute>,
}

impl ColumnDef {
    pub fn is_auto(&self) -> bool {
        self.attributes.iter().any(|a| matches!(a, ColumnAttribute::Auto))
    }

    pub fn is_pk(&self) -> bool {
        self.attributes.iter().any(|a| matches!(a, ColumnAttribute::PrimaryKey))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Int,
    Float,
    String,
    Bool,
    Array(Box<Type>),
    Set(SetType),
    Map(Box<Type>, Box<Type>),
    Date,
    Table(Vec<ColumnDef>),
    Database,
    DatabaseOperation(DatabaseOpKind, Vec<ColumnDef>),
    Json,
    Builtin(StringId),
    /// fiber:T (typed) or fiber (void, inner = None)
    Fiber(Option<Box<Type>>),
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DatabaseOpKind {
    Remove,
}

impl std::fmt::Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Type::Int => write!(f, "int"),
            Type::Float => write!(f, "float"),
            Type::String => write!(f, "str"),
            Type::Bool => write!(f, "bool"),
            Type::Array(inner) => write!(f, "array:{}", inner),
            Type::Set(st) => write!(f, "set:{}", st),
            Type::Map(k, v) => write!(f, "map:{}<->{}", k, v),
            Type::Date => write!(f, "date"),
            Type::Table(_) => write!(f, "table"),
            Type::Database => write!(f, "database"),
            Type::DatabaseOperation(kind, _) => write!(f, "database_op:{:?}", kind),
            Type::Json => write!(f, "json"),
            Type::Builtin(_) => write!(f, "builtin"),
            Type::Fiber(inner) => {
                if let Some(t) = inner {
                    write!(f, "fiber:{}", t)
                } else {
                    write!(f, "fiber")
                }
            }
            Type::Unknown => write!(f, "unknown"),
        }
    }
}

impl std::fmt::Display for SetType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SetType::N => write!(f, "N"),
            SetType::Q => write!(f, "Q"),
            SetType::Z => write!(f, "Z"),
            SetType::S => write!(f, "S"),
            SetType::B => write!(f, "B"),
            SetType::C => write!(f, "C"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum HaltLevel {
    Alert,
    Error,
    Fatal,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExprKind {
    IntLiteral(i64),
    FloatLiteral(f64),
    StringLiteral(StringId),
    BoolLiteral(bool),
    Identifier(StringId),
    RawBlock(StringId),
    ArrayLiteral {
        elements: Vec<Expr>,
    },
    Binary {
        left: Box<Expr>,
        op: TokenKind,
        right: Box<Expr>,
    },
    Unary {
        op: TokenKind,
        right: Box<Expr>,
    },
    FunctionCall {
        name: StringId,
        args: Vec<Argument>,
    },
    MethodCall {
        receiver: Box<Expr>,
        method: StringId,
        args: Vec<Argument>,
        wait_after: bool,
    },
    SetLiteral {
        set_type: SetType,
        elements: Vec<Expr>,
        range: Option<SetRange>,
    },
    ArrayOrSetLiteral {
        elements: Vec<Expr>,
    },
    RandomChoice {
        set: Box<Expr>,
    },
    RandomInt {
        min: Box<Expr>,
        max: Box<Expr>,
        step: Option<Box<Expr>>,
    },
    RandomFloat {
        min: Box<Expr>,
        max: Box<Expr>,
        step: Option<Box<Expr>>,
    },
    MapLiteral {
        key_type: Type,
        value_type: Type,
        elements: Vec<(Expr, Expr)>,
    },
    DateLiteral {
        date_string: StringId,
        format: Option<StringId>,
    },
    TableLiteral {
        columns: Vec<ColumnDef>,
        rows: Vec<Vec<Expr>>,
    },
    DatabaseLiteral(Vec<(StringId, Expr)>),
    Index {
        receiver: Box<Expr>,
        index: Box<Expr>,
    },
    MemberAccess {
        receiver: Box<Expr>,
        member: StringId,
    },
    TerminalCommand(StringId, Vec<Expr>),
    Lambda {
        params: Vec<(Type, StringId)>,
        return_type: Option<Type>,
        body: Box<Expr>,
    },
    Tuple(Vec<Expr>),
    NetCall {
        method: StringId,
        url: Box<Expr>,
        body: Option<Box<Expr>>,
    },
    NetRespond {
        status: Box<Expr>,
        body: Box<Expr>,
        headers: Option<Box<Expr>>,
    },
    As {
        expr: Box<Expr>,
        name: StringId,
    },
    Yield(Box<Expr>),
    Tag(StringId),
}

#[derive(Debug, Clone, PartialEq)]
pub struct SetRange {
    pub start: Box<Expr>,
    pub end: Box<Expr>,
    pub step: Option<Box<Expr>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Stmt {
    pub kind: StmtKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StmtKind {
    VarDecl {
        is_const: bool,
        ty: Type,
        name: StringId,
        value: Option<Expr>,
    },
    Print(Expr),
    TerminalWrite(Expr),
    Input(StringId, Type),
    ExprStmt(Expr),
    If {
        condition: Expr,
        then_branch: Vec<Stmt>,
        else_ifs: Vec<(Expr, Vec<Stmt>)>,
        else_branch: Option<Vec<Stmt>>,
    },
    While {
        condition: Expr,
        body: Vec<Stmt>,
    },
    For {
        var_name: StringId,
        start: Expr,
        end: Expr,
        step: Option<Expr>,
        body: Vec<Stmt>,
        iter_type: ForIterType,
    },
    Break,
    Continue,
    Assign {
        name: StringId,
        value: Expr,
    },
    Halt {
        level: HaltLevel,
        message: Expr,
    },
    FunctionDef {
        name: StringId,
        params: Vec<(Type, StringId)>,
        return_type: Option<Type>,
        body: Vec<Stmt>,
    },
    Return(Option<Expr>),
    FunctionCallStmt {
        name: StringId,
        args: Vec<Argument>,
    },
    Include {
        path: StringId,
        alias: Option<StringId>,
    },
    JsonBind {
        json: Box<Expr>,
        path: Box<Expr>,
        target: StringId,
    },
    JsonInject {
        json: Box<Expr>,
        mapping: Box<Expr>,
        table: StringId,
    },
    FiberDef {
        name: StringId,
        params: Vec<(Type, StringId)>,
        return_type: Option<Type>,   
        body: Vec<Stmt>,
    },
    FiberDecl {
        inner_type: Option<Type>,    
        name: StringId,              
        fiber_name: StringId,        
        args: Vec<Argument>,
    },
    Yield(Expr),
    YieldFrom(Expr),
    YieldVoid,
    NetRequestStmt {
        method: Box<Expr>,
        url: Box<Expr>,
        headers: Option<Box<Expr>>,
        body: Option<Box<Expr>>,
        timeout: Option<Box<Expr>>,
        target: StringId,
    },
    Serve {
        name: StringId,
        port: Box<Expr>,
        host: Option<Box<Expr>>,
        workers: Option<Box<Expr>>,
        routes: Box<Expr>,
    },
    Wait(Expr),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub stmts: Vec<Stmt>,
}
