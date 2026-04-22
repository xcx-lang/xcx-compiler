use crate::lexer::token::{Token, TokenKind};
use crate::sema::interner::Interner;

pub struct Scanner<'a> {
    source: &'a [u8],
    pos: usize,
    char_pos: usize,
    line: usize,
    col: usize,
}

impl<'a> Scanner<'a> {
    pub fn new(source: &'a str) -> Self {
        Self {
            source: source.as_bytes(),
            pos: 0,
            char_pos: 0,
            line: 1,
            col: 1,
        }
    }

    pub fn next_token(&mut self, interner: &mut Interner) -> Token {
        self.skip_whitespace_and_comments();

        let line = self.line;
        let col = self.col;

        if self.is_at_end() {
             return Token::new(TokenKind::EOF, line, col, 0);
        }

        let c = self.advance();

        if c.is_ascii_digit() {
            return self.number(c, line, col);
        }

        if c.is_ascii_alphabetic() || c == b'_' {
            return self.identifier(line, col, interner);
        }

        if c >= 128 {
            let remain = &self.source[self.pos - 1..];
            if remain.starts_with("∪".as_bytes()) {
                self.advance(); self.advance();
                return Token::new(TokenKind::Union, line, col, 1);
            } else if remain.starts_with("∩".as_bytes()) {
                self.advance(); self.advance();
                return Token::new(TokenKind::Intersection, line, col, 1);
            } else if remain.starts_with("⊕".as_bytes()) {
                self.advance(); self.advance();
                return Token::new(TokenKind::SymDifference, line, col, 1);
            }
            return self.identifier(line, col, interner);
        }

        match c {
            b'(' => Token::new(TokenKind::LeftParen, line, col, 1),
            b')' => Token::new(TokenKind::RightParen, line, col, 1),
            b'{' => Token::new(TokenKind::LeftBrace, line, col, 1),
            b'}' => Token::new(TokenKind::RightBrace, line, col, 1),
            b'[' => Token::new(TokenKind::LeftBracket, line, col, 1),
            b']' => Token::new(TokenKind::RightBracket, line, col, 1),
            b',' => {
                if self.peek() == b',' {
                    self.advance();
                    Token::new(TokenKind::DoubleComma, line, col, 2)
                } else {
                    Token::new(TokenKind::Comma, line, col, 1)
                }
            }
            b'.' => {
                if self.peek() == b'.' {
                    self.advance();
                    Token::new(TokenKind::To, line, col, 2)
                } else {
                    Token::new(TokenKind::Dot, line, col, 1)
                }
            }
            b';' => Token::new(TokenKind::Semicolon, line, col, 1),
            b':' => {
                if self.peek() == b':' {
                    self.advance();
                    Token::new(TokenKind::DoubleColon, line, col, 2)
                } else {
                    Token::new(TokenKind::Colon, line, col, 1)
                }
            }
            b'+' => {
                if self.peek() == b'+' {
                    self.advance();
                    Token::new(TokenKind::PlusPlus, line, col, 2)
                } else {
                    Token::new(TokenKind::Plus, line, col, 1)
                }
            }
            b'-' => {
                if self.peek() == b'>' {
                    self.advance();
                    Token::new(TokenKind::Arrow, line, col, 2)
                } else {
                    Token::new(TokenKind::Minus, line, col, 1)
                }
            }
            b'*' => Token::new(TokenKind::Star, line, col, 1),
            b'/' => Token::new(TokenKind::Slash, line, col, 1),
            b'%' => Token::new(TokenKind::Percent, line, col, 1),
            b'^' => Token::new(TokenKind::Caret, line, col, 1),
            b'!' => {
                if self.peek() == b'=' {
                    self.advance();
                    Token::new(TokenKind::BangEqual, line, col, 2)
                } else if self.peek() == b'!' {
                    self.advance();
                    Token::new(TokenKind::Not, line, col, 2)
                } else {
                    Token::new(TokenKind::Bang, line, col, 1)
                }
            }
            b'=' => {
                if self.peek() == b'=' {
                    self.advance();
                    Token::new(TokenKind::EqualEqual, line, col, 2)
                } else {
                    Token::new(TokenKind::Equal, line, col, 1)
                }
            }
            b'<' => {
                if self.peek() == b'=' {
                    self.advance();
                    Token::new(TokenKind::LessEqual, line, col, 2)
                } else if self.peek() == b'-' && self.peek_next() == b'>' {
                    self.advance();
                    self.advance();
                    Token::new(TokenKind::Bridge, line, col, 3)
                } else if self.peek() == b'<' && self.peek_next() == b'<' {
                    self.advance();
                    self.advance();
                    return self.raw_block(line, col, interner);
                } else {
                    Token::new(TokenKind::Less, line, col, 1)
                }
            }
            b'>' => {
                if self.peek() == b'=' {
                    self.advance();
                    Token::new(TokenKind::GreaterEqual, line, col, 2)
                } else if self.peek() == b'!' {
                    self.advance();
                    Token::new(TokenKind::GreaterBang, line, col, 2)
                } else if self.peek() == b'?' {
                    self.advance();
                    Token::new(TokenKind::GreaterQuestion, line, col, 2)
                } else {
                    Token::new(TokenKind::Greater, line, col, 1)
                }
            }
            b'@' => {
                let start_col = col;
                let mut name_bytes = Vec::new();
                let mut offset = 0;
                while self.peek_at(offset).is_ascii_alphabetic() {
                    name_bytes.push(self.peek_at(offset));
                    offset += 1;
                }
                let name = std::str::from_utf8(&name_bytes).unwrap_or("");
                match name {
                    "step" => {
                        for _ in 0..name.len() { self.advance(); }
                        Token::new(TokenKind::AtStep, line, start_col, name.len() + 1)
                    }
                    "auto" => {
                        for _ in 0..name.len() { self.advance(); }
                        Token::new(TokenKind::AtAuto, line, start_col, name.len() + 1)
                    }
                    "wait" => {
                        for _ in 0..name.len() { self.advance(); }
                        Token::new(TokenKind::AtWait, line, start_col, name.len() + 1)
                    }
                    "pk" => {
                        for _ in 0..name.len() { self.advance(); }
                        Token::new(TokenKind::AtPk, line, start_col, name.len() + 1)
                    }
                    "unique" => {
                        for _ in 0..name.len() { self.advance(); }
                        Token::new(TokenKind::AtUnique, line, start_col, name.len() + 1)
                    }
                    "optional" => {
                        for _ in 0..name.len() { self.advance(); }
                        Token::new(TokenKind::AtOptional, line, start_col, name.len() + 1)
                    }
                    "default" => {
                        for _ in 0..name.len() { self.advance(); }
                        Token::new(TokenKind::AtDefault, line, start_col, name.len() + 1)
                    }
                    "fk" => {
                        for _ in 0..name.len() { self.advance(); }
                        Token::new(TokenKind::AtFk, line, start_col, name.len() + 1)
                    }
                    _ => Token::new(TokenKind::Unknown('@'), line, start_col, 1),
                }
            }
            b'&' => {
                if self.peek() == b'&' {
                    self.advance();
                    Token::new(TokenKind::And, line, col, 2)
                } else {
                    Token::new(TokenKind::Unknown('&'), line, col, 1)
                }
            }
            b'|' => {
                if self.peek() == b'|' {
                    self.advance();
                    Token::new(TokenKind::Or, line, col, 2)
                } else {
                    Token::new(TokenKind::Unknown('|'), line, col, 1)
                }
            }
            b'\"' => {
                self.string(line, col, interner)
            }
            b'#' => {
                let start_char_pos = self.char_pos - 1;
                while self.peek().is_ascii_alphanumeric() || self.peek() == b'_' {
                    self.advance();
                }
                let text = std::str::from_utf8(&self.source[self.pos - (self.char_pos - start_char_pos - 1) - 1..self.pos]).unwrap_or("");
                let tag_text = &text[1..]; // skip #
                Token::new(TokenKind::Tag(interner.intern(tag_text)), line, col, self.char_pos - start_char_pos)
            }
            b'\\' => Token::new(TokenKind::Difference, line, col, 1),
            _ => {
                let arr = [c];
                let text = std::str::from_utf8(&arr).unwrap_or("?");
                Token::new(TokenKind::Unknown(text.chars().next().unwrap_or('?')), line, col, 1)
            }
        }
    }

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            let c = self.peek();
            if c.is_ascii_whitespace() {
                self.advance();
            } else if self.source[self.pos..].starts_with(b"---") {
                self.advance(); self.advance(); self.advance();
                
                let mut is_multi = true;
                let mut temp_pos = self.pos;
                while temp_pos < self.source.len() && self.source[temp_pos] != b'\n' {
                    if !self.source[temp_pos].is_ascii_whitespace() {
                        is_multi = false;
                        break;
                    }
                    temp_pos += 1;
                }
                
                if !is_multi {
                    while self.peek() != b'\n' && self.peek() != b'\0' {
                        self.advance();
                    }
                } else {
                    while self.peek() != b'\0' {
                        if self.source[self.pos..].starts_with(b"*---") {
                            self.advance(); self.advance(); self.advance(); self.advance();
                            break;
                        }
                        self.advance();
                    }
                }
            } else {
                break;
            }
        }
    }

    fn peek(&self) -> u8 {
        self.peek_at(0)
    }

    fn peek_next(&self) -> u8 {
        self.peek_at(1)
    }

    fn peek_at(&self, offset: usize) -> u8 {
        if self.pos + offset >= self.source.len() {
            b'\0'
        } else {
            self.source[self.pos + offset]
        }
    }

    fn advance(&mut self) -> u8 {
        let c = self.source[self.pos];
        self.pos += 1;
        if c < 128 || (c & 0b1100_0000) != 0b1000_0000 {
            self.char_pos += 1;
            if c == b'\n' {
                self.line += 1;
                self.col = 1;
            } else {
                self.col += 1;
            }
        }
        c
    }

    fn is_at_end(&self) -> bool {
        self.pos >= self.source.len()
    }

    fn identifier(&mut self, line: usize, col: usize, interner: &mut Interner) -> Token {
        let start_byte_pos = self.pos - 1;
        let start_char_pos = self.char_pos - 1;
        while self.peek().is_ascii_alphanumeric() || self.peek() == b'_' || self.peek() >= 128 {
            self.advance();
        }
        let text_bytes = &self.source[start_byte_pos..self.pos];
        let text = std::str::from_utf8(text_bytes).unwrap_or("");
        let lower_text = text.to_lowercase();

        let kind = match text {
            "UNION" => TokenKind::Union,
            "INTERSECTION" => TokenKind::Intersection,
            "DIFFERENCE" => TokenKind::Difference,
            "SYMMETRIC_DIFFERENCE" => TokenKind::SymDifference,
            "HAS" => TokenKind::Has,
            "AND" => TokenKind::And,
            "OR" => TokenKind::Or,
            "NOT" => TokenKind::Not,
            "N" => TokenKind::TypeSetN,
            "Q" => TokenKind::TypeSetQ,
            "Z" => TokenKind::TypeSetZ,
            "S" => TokenKind::TypeSetS,
            "B" => TokenKind::TypeSetB,
            "C" => TokenKind::TypeSetC,
            _ => match lower_text.as_str() {
                "const" => TokenKind::Const,
                "i" | "int" => TokenKind::TypeI,
                "f" | "float" => TokenKind::TypeF,
                "s" | "string" | "str" => TokenKind::TypeS,
                "b" | "bool" | "boolean" => TokenKind::TypeB,
                "true" => TokenKind::True,
                "false" => TokenKind::False,
                "if" => TokenKind::If,
                "then" => TokenKind::Then,
                "elseif" | "elif" | "elf" => TokenKind::ElseIf,
                "else" | "els" => TokenKind::Else,
                "end" => TokenKind::End,
                "for" => TokenKind::For,
                "in" => TokenKind::In,
                "to" => TokenKind::To,
                "while" => TokenKind::While,
                "do" => TokenKind::Do,
                "break" => TokenKind::Break,
                "continue" => TokenKind::Continue,
                "and" => TokenKind::And,
                "or" => TokenKind::Or,
                "not" => TokenKind::Not,
                "has" => TokenKind::Has,
                "halt" => TokenKind::Halt,
                "alert" => TokenKind::Alert,
                "error" => TokenKind::Error,
                "fatal" => TokenKind::Fatal,
                "terminal" => TokenKind::Terminal,
                "func" | "function" | "fun" => TokenKind::Func,
                "return" => TokenKind::Return,
                "array" => TokenKind::Array,
                "set" => TokenKind::Set,
                "map" => TokenKind::Map,
                "date" => TokenKind::Date,
                "store" => TokenKind::Store,
                "schema" => TokenKind::Schema,
                "data" => TokenKind::Data,
                "table" => TokenKind::Table,
                "database" => TokenKind::Database,
                "columns" => TokenKind::Columns,
                "rows" => TokenKind::Rows,
                "json" => TokenKind::Json,
                "net" => TokenKind::Net,
                "serve" => TokenKind::Serve,
                "fiber" => TokenKind::Fiber,
                "yield" => TokenKind::Yield,
                "empty" => TokenKind::Empty,
                "include" => TokenKind::Include,
                "as" => TokenKind::As,
                "random" => TokenKind::Random,
                "choice" => TokenKind::Choice,
                "from" => TokenKind::From,
                _ => TokenKind::Identifier(interner.intern(text)),
            }
        };
        Token::new(kind, line, col, self.char_pos - start_char_pos)
    }

    fn number(&mut self, _first: u8, line: usize, col: usize) -> Token {
        let start_byte_pos = self.pos - 1;
        let start_char_pos = self.char_pos - 1;
        let mut is_float = false;
        while self.peek().is_ascii_digit() {
            self.advance();
        }
        if self.peek() == b'.' && self.peek_next().is_ascii_digit() {
            is_float = true;
            self.advance();
            while self.peek().is_ascii_digit() {
                self.advance();
            }
        }
        let num_str = std::str::from_utf8(&self.source[start_byte_pos..self.pos]).unwrap_or("0");
        if is_float {
            Token::new(TokenKind::FloatLiteral(num_str.parse().unwrap_or(0.0)), line, col, self.char_pos - start_char_pos)
        } else {
            Token::new(TokenKind::IntLiteral(num_str.parse().unwrap_or(0)), line, col, self.char_pos - start_char_pos)
        }
    }

    fn string(&mut self, line: usize, col: usize, interner: &mut Interner) -> Token {
        let start_char_pos = self.char_pos - 1;
        let mut bytes = Vec::new();
        while self.peek() != b'"' && self.peek() != b'\0' {
            let c = self.advance();
            if c == b'\\' {
                match self.peek() {
                    b'n' => { self.advance(); bytes.push(b'\n'); }
                    b't' => { self.advance(); bytes.push(b'\t'); }
                    b'r' => { self.advance(); bytes.push(b'\r'); }
                    b'0'..=b'7' => {
                        let mut octal = String::new();
                        for _ in 0..3 {
                            if self.peek().is_ascii_digit() && self.peek() <= b'7' {
                                octal.push(self.advance() as char);
                            } else {
                                break;
                            }
                        }
                        if let Ok(val) = u32::from_str_radix(&octal, 8) {
                            if let Some(ch) = std::char::from_u32(val) {
                                let mut b = [0; 4];
                                bytes.extend_from_slice(ch.encode_utf8(&mut b).as_bytes());
                            }
                        }
                    }
                    b'x' => {
                        self.advance();
                        let mut hex = String::new();
                        for _ in 0..2 {
                            if self.peek().is_ascii_hexdigit() {
                                hex.push(self.advance() as char);
                            } else {
                                break;
                            }
                        }
                        if let Ok(val) = u32::from_str_radix(&hex, 16) {
                            if let Some(ch) = std::char::from_u32(val) {
                                let mut b = [0; 4];
                                bytes.extend_from_slice(ch.encode_utf8(&mut b).as_bytes());
                            }
                        }
                    }
                    b'"' => { self.advance(); bytes.push(b'"'); }
                    b'\\' => { self.advance(); bytes.push(b'\\'); }
                    _ => { bytes.push(b'\\'); }
                }
            } else {
                bytes.push(c);
            }
        }
        if self.peek() == b'"' {
            self.advance();
            let parsed_str = String::from_utf8(bytes).unwrap_or_default();
            Token::new(TokenKind::StringLiteral(interner.intern(&parsed_str)), line, col, self.char_pos - start_char_pos)
        } else {
            Token::new(TokenKind::Unknown('"'), line, col, 1)
        }
    }

    fn raw_block(&mut self, line: usize, col: usize, interner: &mut Interner) -> Token {
        let start_raw = self.pos;
        let start_char_pos = self.char_pos - 3;
        while self.peek() != b'\0' {
            if self.source[self.pos..].starts_with(b">>>") {
                let parsed_str = std::str::from_utf8(&self.source[start_raw..self.pos]).unwrap_or_default();
                self.advance(); self.advance(); self.advance();
                return Token::new(TokenKind::RawBlock(interner.intern(parsed_str)), line, col, self.char_pos - start_char_pos);
            }
            self.advance();
        }
        Token::new(TokenKind::Unknown('<'), line, col, 3)
    }
}
