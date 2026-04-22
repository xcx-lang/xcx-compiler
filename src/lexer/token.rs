use crate::sema::interner::StringId;

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // Keywords
    Const,   // const
    TypeI,   // i
    TypeF,   // f
    TypeS,   // s
    TypeB,   // b
    Array,   // array
    Set,     // set
    Map,     // map
    Date,    // date
    Table,   // table
    Database,// database
    Json,    // json
    Net,     // net
    Serve,   // serve
    Columns, // columns
    Rows,    // rows
    Schema,  // schema
    Data,    // data
    Empty,   // EMPTY
    TypeSetN, // N (Natural)
    TypeSetQ, // Q (Rational)
    TypeSetZ, // Z (Integers)
    TypeSetS, // S (Strings)
    TypeSetB, // B (Booleans)
    TypeSetC, // C (Chars)
    True,    // true
    False,   // false

    // Identifiers and Literals
    Identifier(StringId),
    IntLiteral(i64),
    FloatLiteral(f64),
    StringLiteral(StringId),
    RawBlock(StringId),

    // Conditional & Loop Keywords
    If,      // if
    Then,    // then
    ElseIf,  // elseif, elif, elf
    Else,    // else, els
    End,     // end
    For,     // for
    In,      // in
    To,      // to
    While,   // while
    Do,      // do
    Break,   // break
    Continue, // continue
    And,     // AND, &&
    Or,      // OR, ||
    Not,     // NOT, !!
    Has,     // HAS
    AtStep,  // @step
    AtAuto,  // @auto
    AtWait,  // @wait
    AtPk,    // @pk
    AtUnique, // @unique
    AtOptional, // @optional
    AtDefault, // @default
    AtFk,     // @fk
    Halt,    // halt
    Alert,   // alert
    Error,   // error
    Fatal,   // fatal
    Terminal, // terminal
    Store,    // store
    Func,    // func
    Return,  // return
    Include, // include
    As,      // as
    Fiber,   // fiber
    Yield,   // yield
    
    // Set operations
    Union,        // UNION, ∪
    Intersection, // INTERSECTION, ∩
    Difference,   // DIFFERENCE, / (in context)
    SymDifference, // SYMETRIC_DIFFERENCE, ⊕
    
    // Random related
    Random, // random
    Choice, // choice
    From,   // from

    // Operators
    Plus,       // +
    PlusPlus,   // ++
    Minus,      // -
    Star,       // *
    Slash,      // /
    Percent,    // %
    Caret,      // ^
    Equal,      // =
    EqualEqual, // ==
    BangEqual,  // !=
    Greater,    // >
    Less,       // <
    GreaterEqual, // >=
    LessEqual,    // <=
    GreaterQuestion, // >?
    Arrow,      // ->

    // Punctuation
    Colon,     // :
    Semicolon, // ;
    Bang,      // !
    GreaterBang, // >! (Print)
    LeftParen,  // (
    RightParen, // )
    LeftBrace,  // {
    RightBrace, // }
    LeftBracket, // [
    RightBracket, // ]
    Comma,      // ,
    DoubleComma, // ,,
    DoubleColon, // ::
    Dot,        // .
    Bridge,     // <-> or <=>
    
    // Special
    Tag(StringId),
    EOF,
    Unknown(char),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Span {
    pub line: usize,
    pub col: usize,
    pub len: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

impl Token {
    pub fn new(kind: TokenKind, line: usize, col: usize, len: usize) -> Self {
        Self {
            kind,
            span: Span { line, col, len },
        }
    }
}
