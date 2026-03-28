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
pub struct ColumnDef {
    pub name: StringId,
    pub ty: Type,
    pub is_auto: bool,
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
    Json,
    Builtin(StringId),
    /// fiber:T (typed) or fiber (void, inner = None)
    Fiber(Option<Box<Type>>),
    Unknown,
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
        args: Vec<Expr>,
    },
    MethodCall {
        receiver: Box<Expr>,
        method: StringId,
        args: Vec<Expr>,
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
    Index {
        receiver: Box<Expr>,
        index: Box<Expr>,
    },
    MemberAccess {
        receiver: Box<Expr>,
        member: StringId,
    },
    TerminalCommand(StringId, Option<Box<Expr>>),
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
    Input(StringId),
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
        args: Vec<Expr>,
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
        args: Vec<Expr>,
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
