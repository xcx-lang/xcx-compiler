use crate::lexer::scanner::Scanner;
use crate::lexer::token::{Token, TokenKind, Span};
use crate::parser::ast::{Expr, Stmt, Type, Program, SetType, SetRange};
use crate::sema::interner::Interner;
use crate::sema::interner::StringId;

pub struct Parser<'a> {
    scanner: Scanner,
    interner: Interner,
    source: &'a str,
    current: Token,
    peek: Token,
    pub has_error: bool,
}

#[derive(Debug, PartialOrd, PartialEq, Clone, Copy)]
pub enum Precedence {
    Lowest,
    Lambda,      // ->
    Assignment,  // =
    LogicalOr,   // OR, ||
    LogicalAnd,  // AND, &&
    Equals,      // == !=
    LessGreater, // > < >= <= HAS
    Sum,         // + -
    SetOp,       // UNION, INTERSECTION, DIFFERENCE, SYMMETRIC_DIFFERENCE, ∪, ∩, \, ⊕
    Product,     // * / %
    Power,       // ^
    Prefix,      // -x
    Call,        // f(x)
}

impl<'a> Parser<'a> {
    pub fn new(source: &'a str) -> Self {
        let scanner = Scanner::new(source);
        Self::new_with_interner(source, scanner, Interner::new())
    }

    pub fn new_with_interner(source: &'a str, mut scanner: Scanner, mut interner: Interner) -> Self {
        let current = scanner.next_token(&mut interner);
        let peek = scanner.next_token(&mut interner);
        Self {
            scanner,
            interner,
            source,
            current,
            peek,
            has_error: false,
        }
    }

    fn error(&mut self, message: &str) {
        let reporter = crate::diagnostic::report::Reporter::new(self.source);
        reporter.error(self.current.span.line, self.current.span.col, self.current.span.len, message);
        self.has_error = true;
    }

    fn synchronize(&mut self) {
        while self.current.kind != TokenKind::EOF {
            if self.current.kind == TokenKind::Semicolon {
                self.advance();
                return;
            }

            match self.current.kind {
                TokenKind::Func | TokenKind::Fiber | TokenKind::Const | TokenKind::If | TokenKind::While | TokenKind::For | TokenKind::Return | TokenKind::GreaterBang => return,
                _ => {}
            }

            self.advance();
        }
    }

    fn expect(&mut self, kind: TokenKind, message: &str) -> bool {
        if self.current.kind == kind {
            self.advance();
            true
        } else {
            self.error(message);
            false
        }
    }

    fn expect_semicolon(&mut self) -> bool {
        self.expect(TokenKind::Semicolon, "Expected ';' at the end of the statement.")
    }

    fn parse_array_or_set_literal_elements(&mut self, end_kind: TokenKind) -> Vec<Expr> {
        let mut elements = Vec::new();
        while self.current.kind != end_kind && self.current.kind != TokenKind::EOF {
            if let Some(expr) = self.parse_expression(Precedence::Lowest) {
                elements.push(expr);
            }
            // Expression parser stops AT the token after the expression (e.g. at Comma or RightBrace)
            self.advance(); 
            if self.current.kind == TokenKind::Comma {
                self.advance();
            } else if self.current.kind != end_kind {
                // If not comma and not end, something is wrong, but we'll let the loop check end
            }
        }
        elements
    }

    fn parse_set_literal_content(&mut self, st: SetType, lit_span: Span) -> Option<Expr> {
        let mut elements = Vec::new();
        let mut is_range = false;
        let mut range = None;
        
        if self.current.kind != TokenKind::RightBrace && self.current.kind != TokenKind::EOF {
            let first_expr = self.parse_expression(Precedence::Lowest)?;
            self.advance();
            
            if self.current.kind == TokenKind::DoubleComma {
                is_range = true;
                self.advance();
                let end_expr = self.parse_expression(Precedence::Lowest)?;
                self.advance();
                
                let mut step_expr = None;
                if self.current.kind == TokenKind::AtStep {
                    self.advance();
                    let s_expr = self.parse_expression(Precedence::Lowest)?;
                    self.advance();
                    step_expr = Some(Box::new(s_expr));
                }
                
                range = Some(SetRange {
                    start: Box::new(first_expr),
                    end: Box::new(end_expr),
                    step: step_expr,
                });
            } else {
                elements.push(first_expr);
                if self.current.kind == TokenKind::Comma {
                    self.advance();
                }
                while self.current.kind != TokenKind::RightBrace && self.current.kind != TokenKind::EOF {
                    if let Some(expr) = self.parse_expression(Precedence::Lowest) {
                        elements.push(expr);
                    }
                    self.advance();
                    if self.current.kind == TokenKind::Comma {
                        self.advance();
                    }
                }
            }
        }
        
        if self.current.kind == TokenKind::RightBrace {
            self.advance();
        }
        
        if is_range {
            Some(Expr {
                kind: crate::parser::ast::ExprKind::SetLiteral {
                    set_type: st,
                    elements: Vec::new(),
                    range,
                },
                span: lit_span,
            })
        } else {
            Some(Expr {
                kind: crate::parser::ast::ExprKind::SetLiteral {
                    set_type: st,
                    elements,
                    range: None,
                },
                span: lit_span,
            })
        }
    }

    pub fn into_interner(self) -> Interner {
        self.interner
    }

    fn advance(&mut self) {
        self.current = self.peek.clone();
        self.peek = self.scanner.next_token(&mut self.interner);
    }

    fn current_precedence(&self) -> Precedence {
        match self.current.kind {
            TokenKind::Arrow => Precedence::Lambda,
            TokenKind::Equal => Precedence::Assignment,
            TokenKind::Or => Precedence::LogicalOr,
            TokenKind::And => Precedence::LogicalAnd,
            TokenKind::EqualEqual | TokenKind::BangEqual => Precedence::Equals,
            TokenKind::Less | TokenKind::Greater | TokenKind::LessEqual | TokenKind::GreaterEqual | TokenKind::Has => Precedence::LessGreater,
            TokenKind::Plus | TokenKind::Minus | TokenKind::PlusPlus => Precedence::Sum,
            TokenKind::Union | TokenKind::Difference | TokenKind::SymDifference | TokenKind::Intersection => Precedence::SetOp,
            TokenKind::Star | TokenKind::Slash | TokenKind::Percent => Precedence::Product,
            TokenKind::Caret => Precedence::Power,
            TokenKind::Dot | TokenKind::LeftBracket => Precedence::Call,
            _ => Precedence::Lowest,
        }
    }

    fn peek_precedence(&self) -> Precedence {
        match self.peek.kind {
            TokenKind::Arrow => Precedence::Lambda,
            TokenKind::Equal => Precedence::Assignment,
            TokenKind::Or => Precedence::LogicalOr,
            TokenKind::And => Precedence::LogicalAnd,
            TokenKind::EqualEqual | TokenKind::BangEqual => Precedence::Equals,
            TokenKind::Less | TokenKind::Greater | TokenKind::LessEqual | TokenKind::GreaterEqual | TokenKind::Has => Precedence::LessGreater,
            TokenKind::Plus | TokenKind::Minus | TokenKind::PlusPlus => Precedence::Sum,
            TokenKind::Union | TokenKind::Difference | TokenKind::SymDifference | TokenKind::Intersection => Precedence::SetOp,
            TokenKind::Star | TokenKind::Slash | TokenKind::Percent => Precedence::Product,
            TokenKind::Caret => Precedence::Power,
            TokenKind::Dot | TokenKind::LeftBracket => Precedence::Call,
            _ => Precedence::Lowest,
        }
    }

    pub fn parse_program(&mut self) -> Program {
        let mut stmts = Vec::new();
        while self.current.kind != TokenKind::EOF {
            if let Some(stmt) = self.parse_statement() {
                stmts.push(stmt);
            } else {
                self.synchronize();
            }
        }
        Program { stmts }
    }

    fn parse_identifier_as_string_id(&mut self, allow_dots: bool) -> Option<crate::sema::interner::StringId> {
        let kind = self.current.kind.clone();
        let mut text = match kind {
            TokenKind::Identifier(id) => self.interner.lookup(id).to_string(),
            TokenKind::TypeI => "i".to_string(),
            TokenKind::TypeF => "f".to_string(),
            TokenKind::TypeS => "s".to_string(),
            TokenKind::TypeB => "b".to_string(),
            TokenKind::Choice => "choice".to_string(),
            TokenKind::Union => "union".to_string(),
            TokenKind::Intersection => "intersection".to_string(),
            TokenKind::Difference => "difference".to_string(),
            TokenKind::SymDifference => "symmetric_difference".to_string(),
            TokenKind::Alert => "alert".to_string(),
            TokenKind::Error => "error".to_string(),
            TokenKind::Fatal => "fatal".to_string(),
            TokenKind::Terminal => "terminal".to_string(),
            TokenKind::Store => "store".to_string(),
            TokenKind::Date => "date".to_string(),
            TokenKind::Json => "json".to_string(),
            TokenKind::Columns => "columns".to_string(),
            TokenKind::Rows => "rows".to_string(),
            TokenKind::Schema => "schema".to_string(),
            TokenKind::Data => "data".to_string(),
            TokenKind::Empty => "EMPTY".to_string(),
            TokenKind::TypeSetN => "N".to_string(),
            TokenKind::TypeSetQ => "Q".to_string(),
            TokenKind::TypeSetZ => "Z".to_string(),
            TokenKind::TypeSetS => "S".to_string(),
            TokenKind::TypeSetB => "B".to_string(),
            TokenKind::TypeSetC => "C".to_string(),
            TokenKind::Set => "set".to_string(),
            TokenKind::Map => "map".to_string(),
            TokenKind::Table => "table".to_string(),
            TokenKind::Fiber => "fiber".to_string(),
            TokenKind::Serve => "serve".to_string(),
            TokenKind::Net => "net".to_string(),
            TokenKind::Yield => "yield".to_string(),
            TokenKind::Return => "return".to_string(),
            TokenKind::Func => "func".to_string(),
            TokenKind::Array => "array".to_string(),
            TokenKind::Include => "include".to_string(),
            TokenKind::As => "as".to_string(),
            TokenKind::From => "from".to_string(),
            TokenKind::To => "to".to_string(),
            _ => return None,
        };

        if allow_dots {
            while self.peek.kind == TokenKind::Dot {
                self.advance(); // now at '.'
                self.advance(); // now at next part
                if let Some(part_id) = self.parse_identifier_as_string_id(false) {
                    text.push('.');
                    text.push_str(self.interner.lookup(part_id));
                } else {
                    break;
                }
            }
        }

        Some(self.interner.intern(&text))
    }

    fn parse_statement(&mut self) -> Option<Stmt> {
        self.parse_statement_internal()
    }

    fn parse_statement_internal(&mut self) -> Option<Stmt> {
        match self.current.kind {
            TokenKind::Const | TokenKind::TypeI | TokenKind::TypeF | TokenKind::TypeS | TokenKind::TypeB | TokenKind::Array | TokenKind::Set | TokenKind::Map | TokenKind::Date | TokenKind::Table | TokenKind::Json => {
                if self.peek.kind == TokenKind::Equal {
                    self.parse_assignment()
                } else {
                    self.parse_var_decl()
                }
            }
            TokenKind::Identifier(id) if self.interner.lookup(id) == "var" => {
                self.parse_var_decl()
            }
            TokenKind::GreaterBang => {
                self.parse_print_stmt()
            }
            TokenKind::GreaterQuestion => {
                self.parse_input_stmt()
            }
            TokenKind::Halt => {
                self.parse_halt_stmt()
            }
            TokenKind::If => {
                self.parse_if_statement()
            }
            TokenKind::While => {
                self.parse_while_statement()
            }
            TokenKind::For => {
                self.parse_for_statement()
            }
            TokenKind::Break => {
                self.parse_break_statement()
            }
            TokenKind::Continue => {
                self.parse_continue_statement()
            }
            TokenKind::Dot => {
                self.parse_expr_stmt()
            }
            TokenKind::Func => {
                self.parse_func_def()
            }
            TokenKind::Return => {
                self.parse_return_stmt()
            }
            TokenKind::Include => {
                self.parse_include_stmt()
            }
            TokenKind::Fiber => {
                self.parse_fiber_statement()
            }
            TokenKind::Yield => {
                self.parse_yield_stmt()
            }
            TokenKind::AtWait => {
                self.parse_wait_stmt()
            }
            TokenKind::Serve => {
                self.parse_serve_stmt()
            }
            TokenKind::Net => {
                self.parse_net_stmt()
            }
            TokenKind::End => {
                None
            }
            TokenKind::Identifier(_) | TokenKind::Union | TokenKind::Intersection | TokenKind::Difference | TokenKind::SymDifference => {
                let peek_kind = self.peek.kind.clone();
                if peek_kind == TokenKind::Equal {
                    self.parse_assignment()
                } else if peek_kind == TokenKind::LeftParen {
                    self.parse_func_call_stmt()
                } else if matches!(peek_kind, TokenKind::Colon) && !matches!(self.current.kind, TokenKind::Identifier(_)) {
                   self.parse_var_decl()
                } else {
                    self.parse_var_decl().or_else(|| self.parse_expr_stmt())
                }
            }
            _ => {
                self.parse_expr_stmt()
            }
        }
    }

    fn parse_wait_stmt(&mut self) -> Option<Stmt> {
        let span = self.current.span.clone();
        self.advance(); // past '@wait'

        let has_parens = self.current.kind == TokenKind::LeftParen;
        if has_parens {
            self.advance(); // past '('
        }
        
        let ms_expr = self.parse_expression(Precedence::Lowest)?;
        self.advance(); // past last token of expression
        
        if has_parens {
            if self.current.kind != TokenKind::RightParen {
                self.error("Expected ')' to close '@wait(...)'.");
                return None;
            }
            self.advance(); // past ')'
        }
        self.expect_semicolon();
        
        Some(Stmt { kind: crate::parser::ast::StmtKind::Wait(ms_expr), span })
    }

    fn parse_net_stmt(&mut self) -> Option<Stmt> {
        let span = self.current.span.clone();
        self.advance(); // past 'net'

        if self.current.kind != TokenKind::Dot {
            self.error("Expected '.' after 'net'.");
            return None;
        }
        self.advance(); // past '.'

        let method_name = if let TokenKind::Identifier(id) = self.current.kind {
            self.interner.lookup(id).to_string()
        } else {
            self.error("Expected method name after 'net.'.");
            return None;
        };
        self.advance(); // past method name

        match method_name.as_str() {
            // added patch, head, options
            "get" | "post" | "put" | "delete" | "patch" | "head" | "options" => {
                let method_id = self.interner.intern(&method_name);

                if self.current.kind != TokenKind::LeftParen {
                    self.error(&format!("Expected '(' after 'net.{}'.", method_name));
                    return None;
                }
                self.advance(); // past '('

                let url = self.parse_expression(Precedence::Lowest)?;
                self.advance(); // past url

                let mut body = None;
                if self.current.kind == TokenKind::Comma {
                    self.advance(); // past ','
                    body = Some(Box::new(self.parse_expression(Precedence::Lowest)?));
                    self.advance(); // past body
                }

                if self.current.kind != TokenKind::RightParen {
                    self.error(&format!("Expected ')' after 'net.{}' arguments.", method_name));
                    return None;
                }
                self.advance(); // past ')'

                if self.current.kind == TokenKind::As {
                    self.advance(); // past 'as'
                    let target = if let TokenKind::Identifier(t_id) = self.current.kind {
                        t_id
                    } else {
                        self.error("Expected identifier after 'as'.");
                        return None;
                    };
                    self.advance(); // past target
                    self.expect_semicolon();
                    return Some(Stmt {
                        kind: crate::parser::ast::StmtKind::NetRequestStmt {
                            method: Box::new(crate::parser::ast::Expr {
                                kind: crate::parser::ast::ExprKind::StringLiteral(method_id),
                                span: span.clone(),
                            }),
                            url: Box::new(url),
                            headers: None,
                            body,
                            timeout: None,
                            target,
                        },
                        span,
                    });
                }

                // No 'as' — just a statement expression
                self.expect_semicolon();
                Some(Stmt {
                    kind: crate::parser::ast::StmtKind::ExprStmt(crate::parser::ast::Expr {
                        kind: crate::parser::ast::ExprKind::NetCall {
                            method: method_id,
                            url: Box::new(url),
                            body,
                        },
                        span: span.clone(),
                    }),
                    span,
                })
            }

            "request" => {
                if self.current.kind != TokenKind::LeftBrace {
                    self.error("Expected '{' after 'net.request'.");
                    return None;
                }
                self.advance(); // past '{'

                let mut r_method = None;
                let mut r_url = None;
                let mut r_headers = None;
                let mut r_body = None;
                let mut r_timeout = None;

                while self.current.kind != TokenKind::RightBrace && self.current.kind != TokenKind::EOF {
                    let field = if let Some(f_id) = self.parse_identifier_as_string_id(false) {
                        self.interner.lookup(f_id).to_string()
                    } else {
                        break;
                    };
                    self.advance(); // past field name

                    if self.current.kind != TokenKind::Equal {
                        self.error(&format!("Expected '=' after '{}' in net.request.", field));
                        break;
                    }
                    self.advance(); // past '='

                    let val = self.parse_expression(Precedence::Lowest)?;
                    self.advance(); // past val

                    match field.as_str() {
                        "method"  => r_method  = Some(Box::new(val)),
                        "url"     => r_url     = Some(Box::new(val)),
                        "headers" => r_headers = Some(Box::new(val)),
                        "body"    => r_body    = Some(Box::new(val)),
                        "timeout" => r_timeout = Some(Box::new(val)),
                        _ => {}
                    }

                    if self.current.kind == TokenKind::Comma {
                        self.advance();
                    }
                }

                if self.current.kind == TokenKind::RightBrace {
                    self.advance();
                }

                if self.current.kind == TokenKind::As {
                    self.advance(); // past 'as'
                    let target = if let TokenKind::Identifier(t_id) = self.current.kind {
                        t_id
                    } else {
                        self.error("Expected identifier after 'as'.");
                        return None;
                    };
                    self.advance(); // past target
                    self.expect_semicolon();
                    return Some(Stmt {
                        kind: crate::parser::ast::StmtKind::NetRequestStmt {
                            method: r_method.unwrap_or_else(|| Box::new(crate::parser::ast::Expr {
                                kind: crate::parser::ast::ExprKind::StringLiteral(self.interner.intern("GET")),
                                span: span.clone(),
                            })),
                            url: r_url.unwrap_or_else(|| Box::new(crate::parser::ast::Expr {
                                kind: crate::parser::ast::ExprKind::StringLiteral(self.interner.intern("")),
                                span: span.clone(),
                            })),
                            headers: r_headers,
                            body: r_body,
                            timeout: r_timeout,
                            target,
                        },
                        span,
                    });
                }

                // net.request without 'as' — not useful but don't crash
                self.expect_semicolon();
                None
            }

            "respond" => {
                // net.respond(...) used as statement (inside fiber)
                if self.current.kind != TokenKind::LeftParen {
                    self.error("Expected '(' after 'net.respond'.");
                    return None;
                }
                self.advance(); // past '('

                let status = self.parse_expression(Precedence::Lowest)?;
                self.advance(); // past status

                if self.current.kind != TokenKind::Comma {
                    self.error("Expected ',' after status in 'net.respond'.");
                    return None;
                }
                self.advance(); // past ','

                let body = self.parse_expression(Precedence::Lowest)?;
                self.advance(); // past body

                let mut headers = None;
                if self.current.kind == TokenKind::Comma {
                    self.advance(); // past ','
                    headers = Some(Box::new(self.parse_expression(Precedence::Lowest)?));
                    self.advance(); // past headers
                }

                if self.current.kind != TokenKind::RightParen {
                    self.error("Expected ')' after 'net.respond' arguments.");
                    return None;
                }
                self.advance(); // past ')'
                self.expect_semicolon();

                Some(Stmt {
                    kind: crate::parser::ast::StmtKind::ExprStmt(crate::parser::ast::Expr {
                        kind: crate::parser::ast::ExprKind::NetRespond {
                            status: Box::new(status),
                            body: Box::new(body),
                            headers,
                        },
                        span: span.clone(),
                    }),
                    span,
                })
            }

            _ => {
                self.error(&format!("Unknown method 'net.{}'", method_name));
                None
            }
        }
    }

    fn parse_include_stmt(&mut self) -> Option<Stmt> {
        let span = self.current.span.clone();
        self.advance(); // past 'include'

        let path = if let TokenKind::StringLiteral(id) = self.current.kind {
            id
        } else {
            return None;
        };
        self.advance(); // past path string

        let mut alias = None;
        if self.current.kind == TokenKind::As {
            self.advance(); // past 'as'
            if let TokenKind::Identifier(id) = self.current.kind {
                alias = Some(id);
                self.advance();
            } else {
                return None;
            }
        }

        self.expect_semicolon();

        Some(Stmt {
            kind: crate::parser::ast::StmtKind::Include { path, alias },
            span,
        })
    }

    fn parse_if_statement(&mut self) -> Option<Stmt> {
        let start_span = self.current.span.clone();
        self.advance(); // move past 'if' to '('
        
        if self.current.kind != TokenKind::LeftParen {
            self.error("The condition for 'if' must be enclosed in parentheses '()'.");
            return None;
        }
        self.advance(); // move past '(' to start of expr
        
        let condition_opt = self.parse_expression(Precedence::Lowest);
        if condition_opt.is_none() {
            return None;
        }
        let condition = condition_opt.unwrap();
        self.advance(); // past last token of condition
        
        if self.current.kind != TokenKind::RightParen {
            self.error("Missing ')' after 'if' condition.");
            return None;
        }
        self.advance(); // past ')' to 'then'
        
        if self.current.kind != TokenKind::Then {
            self.error("Missing 'then' after 'if' condition.");
        } else {
            self.advance(); // move past 'then'
        }

        self.expect_semicolon();
        
        let mut then_branch = Vec::new();
        while self.current.kind != TokenKind::ElseIf && 
              self.current.kind != TokenKind::Else && 
              self.current.kind != TokenKind::End && 
              self.current.kind != TokenKind::EOF 
        {
            if let Some(stmt) = self.parse_statement() {
                then_branch.push(stmt);
            } else {
                self.advance();
            }
        }

        let mut else_ifs = Vec::new();
        while self.current.kind == TokenKind::ElseIf {
            self.advance(); // move past 'elseif'
            if self.current.kind != TokenKind::LeftParen {
                self.error("The condition for 'elseif' must be enclosed in parentheses '()'.");
                return None;
            }
            self.advance(); // move past '('
            
            let elif_cond_opt = self.parse_expression(Precedence::Lowest);
            if elif_cond_opt.is_none() { return None; }
            let elif_cond = elif_cond_opt.unwrap();
            self.advance(); // move past last expr token to ')'
            
            if self.current.kind != TokenKind::RightParen { 
                self.error("Missing ')' after 'elseif' condition.");
                return None;
            }
            self.advance(); // past ')'
            if self.current.kind != TokenKind::Then { 
                self.error("Missing 'then' after 'elseif' condition.");
                return None;
            }
            self.advance(); // past 'then'
            
            self.expect_semicolon();
            
            let mut elif_branch = Vec::new();
            while self.current.kind != TokenKind::ElseIf && 
                  self.current.kind != TokenKind::Else && 
                  self.current.kind != TokenKind::End && 
                  self.current.kind != TokenKind::EOF 
            {
                if let Some(stmt) = self.parse_statement() {
                    elif_branch.push(stmt);
                } else {
                    self.advance();
                }
            }
            else_ifs.push((elif_cond, elif_branch));
        }

        let mut else_branch = None;
        if self.current.kind == TokenKind::Else {
            self.advance(); // past 'else'
            self.expect_semicolon();
            
            let mut branch = Vec::new();
            while self.current.kind != TokenKind::End && self.current.kind != TokenKind::EOF {
                if let Some(stmt) = self.parse_statement() {
                    branch.push(stmt);
                } else {
                    self.advance();
                }
            }
            else_branch = Some(branch);
        }

        if self.current.kind == TokenKind::End {
            self.advance(); // past 'end'
            self.expect_semicolon();
            
            return Some(Stmt {
                kind: crate::parser::ast::StmtKind::If {
                    condition,
                    then_branch,
                    else_ifs,
                    else_branch,
                },
                span: start_span,
            });
        }
        
        None
    }

    fn parse_while_statement(&mut self) -> Option<Stmt> {
        let start_span = self.current.span.clone();
        self.advance(); // past 'while'
        
        if self.current.kind != TokenKind::LeftParen { 
            self.error("The condition for 'while' must be enclosed in parentheses '()'.");
            return None;
        }
        self.advance(); // past '('
        
        let condition = self.parse_expression(Precedence::Lowest)?;
        self.advance(); // past last token of condition
        
        if self.current.kind != TokenKind::RightParen { return None; }
        self.advance(); // past ')'
        
        if self.current.kind != TokenKind::Do { return None; }
        self.advance(); // past 'do'
        
        self.expect_semicolon();

        let mut body = Vec::new();
        while self.current.kind != TokenKind::End && self.current.kind != TokenKind::EOF {
            if let Some(stmt) = self.parse_statement() {
                body.push(stmt);
            } else {
                self.advance();
            }
        }

        if self.current.kind == TokenKind::End {
            self.advance();
            self.expect_semicolon();
        }

        Some(Stmt {
            kind: crate::parser::ast::StmtKind::While { condition, body },
            span: start_span,
        })
    }

    fn parse_for_statement(&mut self) -> Option<Stmt> {
        let start_span = self.current.span.clone();
        self.advance(); // past 'for'

        let var_name = if let Some(id) = self.parse_identifier_as_string_id(true) {
            id
        } else {
            return None;
        };
        self.advance(); // past name

        if self.current.kind != TokenKind::In { return None; }
        self.advance(); // past 'in'

        let start = self.parse_expression(Precedence::Lowest)?;
        self.advance(); // past start expr

        let (end, step, iter_type) = if self.current.kind == TokenKind::To {
            self.advance(); // past 'to'
            let end = self.parse_expression(Precedence::Lowest)?;
            self.advance(); // past end expr
 
            let mut step = None;
            if self.current.kind == TokenKind::AtStep {
                self.advance(); // past '@step'
                step = Some(self.parse_expression(Precedence::Lowest)?);
                self.advance(); // past step expr
            }
            (end, step, crate::parser::ast::ForIterType::Range)
        } else {
            let mut step = None;
            if self.current.kind == TokenKind::AtStep {
                self.advance(); // past '@step'
                step = Some(self.parse_expression(Precedence::Lowest)?);
                self.advance(); // past step expr
            }
            (Expr { kind: crate::parser::ast::ExprKind::IntLiteral(0), span: start_span.clone() }, step, crate::parser::ast::ForIterType::Array)
        };

        if self.current.kind != TokenKind::Do { return None; }
        self.advance(); // past 'do'

        self.expect_semicolon();

        let mut body = Vec::new();
        while self.current.kind != TokenKind::End && self.current.kind != TokenKind::EOF {
            if let Some(stmt) = self.parse_statement() {
                body.push(stmt);
            } else {
                self.advance();
            }
        }

        if self.current.kind == TokenKind::End {
            self.advance();
            self.expect_semicolon();
        }

        Some(Stmt {
            kind: crate::parser::ast::StmtKind::For { 
                var_name, 
                start, 
                end, 
                step, 
                body,
                iter_type
            },
            span: start_span,
        })
    }

    fn parse_assignment(&mut self) -> Option<Stmt> {
        let span = self.current.span.clone();
        let name = self.parse_identifier_as_string_id(true)?;
        self.advance(); // past name
        
        if self.current.kind != TokenKind::Equal { return None; }
        self.advance(); // past '='
        
        let value = self.parse_expression(Precedence::Lowest)?;
        
        self.advance(); // past last token of expr
        self.expect_semicolon();
        
        Some(Stmt {
            kind: crate::parser::ast::StmtKind::Assign { name, value },
            span,
        })
    }

    fn parse_break_statement(&mut self) -> Option<Stmt> {
        let span = self.current.span.clone();
        self.advance(); // past 'break'
        self.expect_semicolon();
        Some(Stmt {
            kind: crate::parser::ast::StmtKind::Break,
            span,
        })
    }

    fn parse_continue_statement(&mut self) -> Option<Stmt> {
        let span = self.current.span.clone();
        self.advance(); // past 'continue'
        self.expect_semicolon();
        Some(Stmt {
            kind: crate::parser::ast::StmtKind::Continue,
            span,
        })
    }

    fn parse_expr_stmt(&mut self) -> Option<Stmt> {
        let span = self.current.span.clone();
        let expr = self.parse_expression(Precedence::Lowest)?;
        self.advance(); // past last token of expr
        
        let kind = if let crate::parser::ast::ExprKind::MethodCall { receiver, method, args } = expr.kind {
            if method == self.interner.intern("bind") && args.len() == 2 {
                if let crate::parser::ast::ExprKind::Identifier(target) = args[1].kind {
                    crate::parser::ast::StmtKind::JsonBind {
                        json: receiver,
                        path: Box::new(args[0].clone()),
                        target,
                    }
                } else {
                    crate::parser::ast::StmtKind::ExprStmt(crate::parser::ast::Expr {
                        kind: crate::parser::ast::ExprKind::MethodCall { receiver, method, args },
                        span: expr.span,
                    })
                }
            } else if method == self.interner.intern("inject") && args.len() == 2 {
                 if let crate::parser::ast::ExprKind::Identifier(table_id) = args[1].kind {
                    crate::parser::ast::StmtKind::JsonInject {
                        json: receiver,
                        mapping: Box::new(args[0].clone()),
                        table: table_id,
                    }
                } else if let crate::parser::ast::ExprKind::StringLiteral(table_name) = args[1].kind {
                    crate::parser::ast::StmtKind::JsonInject {
                        json: receiver,
                        mapping: Box::new(args[0].clone()),
                        table: table_name,
                    }
                } else {
                    crate::parser::ast::StmtKind::ExprStmt(crate::parser::ast::Expr {
                        kind: crate::parser::ast::ExprKind::MethodCall { receiver, method, args },
                        span: expr.span,
                    })
                }
            } else {
                crate::parser::ast::StmtKind::ExprStmt(crate::parser::ast::Expr {
                    kind: crate::parser::ast::ExprKind::MethodCall { receiver, method, args },
                    span: expr.span,
                })
            }
        } else {
            crate::parser::ast::StmtKind::ExprStmt(expr)
        };

        self.expect_semicolon();
        Some(Stmt {
            kind,
            span,
        })
    }

    fn parse_var_decl(&mut self) -> Option<Stmt> {
        let span = self.current.span.clone();
        let mut is_const = false;
        if self.current.kind == TokenKind::Const {
            is_const = true;
            self.advance();
        }

        let is_var_inferred = if let TokenKind::Identifier(id) = self.current.kind {
            self.interner.lookup(id) == "var"
        } else {
            false
        };

        let mut ty = if is_var_inferred {
            self.advance(); // past 'var'
            if self.current.kind == TokenKind::Colon {
                self.advance(); // past ':'
            }
            Type::Unknown
        } else {
            let ty_opt = self.parse_type();
            if ty_opt.is_none() {
                if is_const {
                    self.error("Expected type specification after 'const'.");
                    return None;
                }
                return None; 
            }
            ty_opt.unwrap()
        };

        let is_map = matches!(ty, Type::Map(_, _));
        let is_table = matches!(ty, Type::Table(_));

        if self.current.kind == TokenKind::Colon {
            self.advance(); // past ':'
        }
       

        let name = if let Some(id) = self.parse_identifier_as_string_id(true) {
            id
        } else {
            return None;
        };
        self.advance(); // past name

        let value = if is_map {
            if self.current.kind == TokenKind::Equal {
                self.advance();
            }
            if self.current.kind == TokenKind::LeftBrace || self.current.kind == TokenKind::Map {
                let map_lit = self.parse_map_literal()?;
                if let crate::parser::ast::ExprKind::MapLiteral { ref key_type, ref value_type, .. } = map_lit.kind {
                    ty = Type::Map(Box::new(key_type.clone()), Box::new(value_type.clone()));
                }
                self.expect_semicolon();
                Some(map_lit)
            } else {
                let val = self.parse_expression(Precedence::Lowest)?;
                self.advance();
                self.expect_semicolon();
                Some(val)
            }
        } else if is_table {
            if self.current.kind == TokenKind::Equal {
                self.advance();
            }
            if self.current.kind == TokenKind::LeftBrace || self.current.kind == TokenKind::Table {
                let table_lit = self.parse_table_literal()?;
                if let crate::parser::ast::ExprKind::TableLiteral { ref columns, .. } = table_lit.kind {
                    ty = Type::Table(columns.clone());
                }
                self.expect_semicolon();
                Some(table_lit)
            } else {
                let val = self.parse_expression(Precedence::Lowest)?;
                self.advance();
                self.expect_semicolon();
                Some(val)
            }
        } else if self.current.kind == TokenKind::Equal {
            self.advance(); // past '='
            let val = self.parse_expression(Precedence::Lowest)?;
            self.advance();
            self.expect_semicolon();
            Some(val)
        } else if matches!(self.current.kind, TokenKind::RawBlock(_)) {
            let mut val = self.parse_expression(Precedence::Lowest)?;
            if matches!(ty, Type::Json) {
                let parse_method = self.interner.intern("parse");
                let json_target = self.interner.intern("json");
                
                val = crate::parser::ast::Expr {
                    kind: crate::parser::ast::ExprKind::MethodCall {
                        receiver: Box::new(crate::parser::ast::Expr { kind: crate::parser::ast::ExprKind::Identifier(json_target), span: span.clone() }),
                        method: parse_method,
                        args: vec![val.clone()],
                    },
                    span: span.clone(),
                };
            }
            
            self.advance();
            self.expect_semicolon();
            Some(val)
        } else if self.current.kind == TokenKind::LeftBrace {
            let lit_span = self.current.span.clone();
            self.advance(); // past '{'
            
            if let Type::Set(st) = &ty {
                let st = st.clone();
                let mut elements = Vec::new();
                let mut range = None;
                
                if self.current.kind != TokenKind::RightBrace && self.current.kind != TokenKind::EOF {
                    let first_expr = self.parse_expression(Precedence::Lowest)?;
                    self.advance();
                    
                    if self.current.kind == TokenKind::DoubleComma {
                        self.advance();
                        let end_expr = self.parse_expression(Precedence::Lowest)?;
                        self.advance();
                        
                        let mut step_expr = None;
                        if self.current.kind == TokenKind::AtStep {
                            self.advance();
                            let s_expr = self.parse_expression(Precedence::Lowest)?;
                            self.advance();
                            step_expr = Some(Box::new(s_expr));
                        }
                        
                        range = Some(SetRange {
                            start: Box::new(first_expr),
                            end: Box::new(end_expr),
                            step: step_expr,
                        });
                    } else {
                        elements.push(first_expr);
                        if self.current.kind == TokenKind::Comma {
                            self.advance();
                        }
                        while self.current.kind != TokenKind::RightBrace && self.current.kind != TokenKind::EOF {
                            if let Some(expr) = self.parse_expression(Precedence::Lowest) {
                                elements.push(expr);
                            }
                            self.advance();
                            if self.current.kind == TokenKind::Comma {
                                self.advance();
                            }
                        }
                    }
                }
                
                if self.current.kind == TokenKind::RightBrace {
                    self.advance();
                }
                self.expect_semicolon();
                Some(crate::parser::ast::Expr {
                    kind: crate::parser::ast::ExprKind::SetLiteral {
                        set_type: st,
                        elements,
                        range,
                    },
                    span: lit_span,
                })
            } else {
                let mut elements = Vec::new();
                while self.current.kind != TokenKind::RightBrace && self.current.kind != TokenKind::EOF {
                    if let Some(expr) = self.parse_expression(Precedence::Lowest) {
                        elements.push(expr);
                    }
                    self.advance();
                    if self.current.kind == TokenKind::Comma {
                        self.advance();
                    }
                }
                if self.current.kind == TokenKind::RightBrace {
                    self.advance();
                }
                self.expect_semicolon();
                Some(crate::parser::ast::Expr {
                    kind: crate::parser::ast::ExprKind::ArrayLiteral { elements },
                    span: lit_span,
                })
            }
        } else if self.current.kind == TokenKind::LeftBrace {
            let lit_span = self.current.span.clone();
            self.advance(); // past '{'
            // In VarDecl, if we don't have an explicit Type::Set, we still might be 
            // an array if ty is Type::Array.
            if let Type::Array(_) = &ty {
                let elements = self.parse_array_or_set_literal_elements(TokenKind::RightBrace);
                Some(crate::parser::ast::Expr {
                    kind: crate::parser::ast::ExprKind::ArrayLiteral { elements },
                    span: lit_span,
                })
            } else {
                let elements = self.parse_array_or_set_literal_elements(TokenKind::RightBrace);
                Some(crate::parser::ast::Expr {
                    kind: crate::parser::ast::ExprKind::ArrayOrSetLiteral { elements },
                    span: lit_span,
                })
            }
        } else {
            self.expect_semicolon();
            None
        };

        Some(Stmt {
            kind: crate::parser::ast::StmtKind::VarDecl { is_const, ty, name, value },
            span,
        })
    }

    fn parse_input_stmt(&mut self) -> Option<Stmt> {
        let span = self.current.span.clone();
        self.advance(); // past '>?'
        
        let name = if let Some(id) = self.parse_identifier_as_string_id(true) {
            id
        } else {
            return None;
        };
        
        self.advance(); // past name
        self.expect_semicolon();
        
        Some(Stmt {
            kind: crate::parser::ast::StmtKind::Input(name),
            span,
        })
    }

    fn parse_print_stmt(&mut self) -> Option<Stmt> {
        let span = self.current.span.clone();
        self.advance(); // past '>!'
        let expr = self.parse_expression(Precedence::Lowest)?;
        self.advance();
        self.expect_semicolon();
        Some(Stmt {
            kind: crate::parser::ast::StmtKind::Print(expr),
            span,
        })
    }

    fn parse_halt_stmt(&mut self) -> Option<Stmt> {
        let start_span = self.current.span.clone();
        self.advance(); // past 'halt'
        
        if self.current.kind != TokenKind::Dot { return None; }
        self.advance(); // past '.'
        
        let level = match self.current.kind {
            TokenKind::Alert => crate::parser::ast::HaltLevel::Alert,
            TokenKind::Error => crate::parser::ast::HaltLevel::Error,
            TokenKind::Fatal => crate::parser::ast::HaltLevel::Fatal,
            _ => return None,
        };
        self.advance(); // past level
        
        if self.current.kind != TokenKind::GreaterBang { return None; }
        self.advance(); // past '>!'
        
        let message = self.parse_expression(Precedence::Lowest)?;
        self.advance();
        self.expect_semicolon();
        
        Some(Stmt {
            kind: crate::parser::ast::StmtKind::Halt { level, message },
            span: start_span,
        })
    }

    fn parse_func_def(&mut self) -> Option<Stmt> {
        let start_span = self.current.span.clone();
        self.advance(); // past 'func'

        let mut return_type = None;
        let mut is_xcx_style = false;

        if self.current.kind == TokenKind::Colon {
             is_xcx_style = true;
             self.advance(); // past ':'
             return_type = self.parse_type();
             if self.current.kind == TokenKind::Colon {
                 self.advance(); // past second ':'
             }
        }

        let name = if let Some(id) = self.parse_identifier_as_string_id(true) {
            id
        } else {
            return None;
        };
        self.advance(); // past name

        if self.current.kind != TokenKind::LeftParen { return None; }
        self.advance(); // past '('

        let mut params: Vec<(crate::parser::ast::Type, crate::sema::interner::StringId)> = Vec::new();
        while self.current.kind != TokenKind::RightParen && self.current.kind != TokenKind::EOF {
            if self.current.kind == TokenKind::Arrow {
                self.advance(); // past '->'
                return_type = self.parse_type();
                break;
            }

            let ty = self.parse_type();
            if ty.is_none() { break; }
            let ty = ty.unwrap();
            
            if self.current.kind != TokenKind::Colon { break; }
            self.advance(); // past ':'

            let param_name = if let Some(id) = self.parse_identifier_as_string_id(false) {
                id
            } else {
                break;
            };
            self.advance(); // past param name

            params.push((ty, param_name));

            if self.current.kind == TokenKind::Comma {
                self.advance();
            }
        }

        if self.current.kind == TokenKind::RightParen {
            self.advance(); // past ')'
        }

        if !is_xcx_style && self.current.kind == TokenKind::Arrow {
            self.advance(); // past '->'
            return_type = self.parse_type();
        }

        let mut body = Vec::new();
        if is_xcx_style {
            if self.current.kind == TokenKind::Do {
                self.advance();
                if self.current.kind == TokenKind::Semicolon {
                    self.advance();
                }
            }
            while self.current.kind != TokenKind::End && self.current.kind != TokenKind::EOF {
                if let Some(stmt) = self.parse_statement() {
                    body.push(stmt);
                } else {
                    self.advance();
                }
            }
            if self.current.kind == TokenKind::End {
                self.advance();
                if self.current.kind == TokenKind::Semicolon {
                    self.advance();
                }
            }
        } else {
            if self.current.kind != TokenKind::LeftBrace { return None; }
            self.advance(); // past '{'
            while self.current.kind != TokenKind::RightBrace && self.current.kind != TokenKind::EOF {
                if let Some(stmt) = self.parse_statement() {
                    body.push(stmt);
                } else {
                    self.advance();
                }
            }
            if self.current.kind == TokenKind::RightBrace {
                self.advance();
                self.expect_semicolon();
            }
        }

        Some(Stmt {
            kind: crate::parser::ast::StmtKind::FunctionDef {
                name,
                params,
                return_type,
                body,
            },
            span: start_span,
        })
    }

    fn parse_fiber_statement(&mut self) -> Option<Stmt> {
        if self.peek.kind == TokenKind::Colon {
            self.parse_fiber_decl()
        } else {
            self.parse_fiber_def()
        }
    }

    fn parse_fiber_def(&mut self) -> Option<Stmt> {
        let start_span = self.current.span.clone();
        self.advance(); // past 'fiber'

        let name = if let Some(id) = self.parse_identifier_as_string_id(true) {
            id
        } else {
            self.error("Expected fiber name after 'fiber'.");
            return None;
        };
        self.advance(); // past name

        self.finish_fiber_def(start_span, name, None)
    }

    fn finish_fiber_def(&mut self, start_span: Span, name: crate::sema::interner::StringId, return_type: Option<crate::parser::ast::Type>) -> Option<Stmt> {
        if self.current.kind != TokenKind::LeftParen {
            self.error("Expected '(' after fiber name.");
            return None;
        }
        self.advance(); // past '('

        let mut params: Vec<(crate::parser::ast::Type, crate::sema::interner::StringId)> = Vec::new();
        let mut return_type = return_type;

        while self.current.kind != TokenKind::RightParen && self.current.kind != TokenKind::EOF {
            if self.current.kind == TokenKind::Arrow {
                self.advance(); // past '->'
                return_type = self.parse_type();
                break;
            }

            let ty = self.parse_type();
            if ty.is_none() { break; }
            let ty = ty.unwrap();

            if self.current.kind != TokenKind::Colon { break; }
            self.advance(); // past ':'

            let param_name = if let Some(id) = self.parse_identifier_as_string_id(false) {
                id
            } else {
                break;
            };
            self.advance(); // past param name

            params.push((ty, param_name));

            if self.current.kind == TokenKind::Comma {
                self.advance();
            }
        }

        if self.current.kind == TokenKind::RightParen {
            self.advance(); // past ')'
        }

        if self.current.kind != TokenKind::LeftBrace {
            self.error("Expected '{' to start fiber body.");
            return None;
        }
        self.advance(); // past '{'

        let mut body = Vec::new();
        while self.current.kind != TokenKind::RightBrace && self.current.kind != TokenKind::EOF {
            if let Some(stmt) = self.parse_statement() {
                body.push(stmt);
            } else {
                self.advance();
            }
        }

        if self.current.kind == TokenKind::RightBrace {
            self.advance();
            self.expect_semicolon();
        }

        Some(Stmt {
            kind: crate::parser::ast::StmtKind::FiberDef {
                name,
                params,
                return_type,
                body,
            },
            span: start_span,
        })
    }

    fn parse_fiber_decl(&mut self) -> Option<Stmt> {
        let start_span = self.current.span.clone();
        self.advance(); // past 'fiber'
        self.advance(); // past ':'

        let inner_type: Option<crate::parser::ast::Type> = if self.current.kind == TokenKind::Identifier(self.interner.intern("")) {
            unreachable!()
        } else {
            if let TokenKind::Identifier(_) = self.current.kind {
                if self.peek.kind == TokenKind::Equal {
                    None
                } else {
                    let ty = self.parse_type();
                    if self.current.kind == TokenKind::Colon {
                        self.advance();
                    }
                    ty
                }
            } else {
                let ty = self.parse_type();
                if self.current.kind == TokenKind::Colon {
                    self.advance();
                }
                ty
            }
        };

        let name = if let Some(id) = self.parse_identifier_as_string_id(true) {
            id
        } else {
            self.error("Expected variable name in fiber declaration.");
            return None;
        };
        self.advance(); // past varname

        if self.current.kind == TokenKind::LeftParen {
            // Pivot to definition
            return self.finish_fiber_def(start_span, name, inner_type);
        }

        if self.current.kind != TokenKind::Equal {
            self.error("Expected '(' for fiber definition or '=' for fiber instantiation.");
            return None;
        }
        self.advance(); // past '='

        let fiber_name = if let Some(id) = self.parse_identifier_as_string_id(true) {
            id
        } else {
            self.error("Expected fiber definition name in fiber declaration.");
            return None;
        };
        self.advance(); // past fiber_name

        if self.current.kind != TokenKind::LeftParen {
            self.error("Expected '(' after fiber name in fiber declaration.");
            return None;
        }
        self.advance(); // past '('

        let mut args = Vec::new();
        while self.current.kind != TokenKind::RightParen && self.current.kind != TokenKind::EOF {
            let arg = self.parse_expression(Precedence::Lowest)?;
            args.push(arg);
            self.advance();
            if self.current.kind == TokenKind::Comma {
                self.advance();
            }
        }

        if self.current.kind == TokenKind::RightParen {
            self.advance(); // past ')'
        }
        self.expect_semicolon();

        Some(Stmt {
            kind: crate::parser::ast::StmtKind::FiberDecl {
                inner_type,
                name,
                fiber_name,
                args,
            },
            span: start_span,
        })
    }

    fn parse_yield_stmt(&mut self) -> Option<Stmt> {
        let span = self.current.span.clone();
        self.advance(); // past 'yield'

        if self.current.kind == TokenKind::From {
            self.advance(); // past 'from'
            let expr = self.parse_expression(Precedence::Lowest)?;
            if self.current.kind != TokenKind::Semicolon {
                self.advance();
            }
            self.expect_semicolon();
            return Some(Stmt {
                kind: crate::parser::ast::StmtKind::YieldFrom(expr),
                span,
            });
        }

        if self.current.kind == TokenKind::Semicolon {
            self.advance();
            return Some(Stmt {
                kind: crate::parser::ast::StmtKind::YieldVoid,
                span,
            });
        }

        let expr = self.parse_expression(Precedence::Lowest)?;
        if self.current.kind != TokenKind::Semicolon {
            self.advance();
        }
        self.expect_semicolon();

        Some(Stmt {
            kind: crate::parser::ast::StmtKind::Yield(expr),
            span,
        })
    }

    // ---------------------------------------------------------------------------
    // parse_serve_stmt — routes is a MapLiteral [s :: fiber_id, ...]
    // The issue was that after parse_expression for 'routes', current lands on ']'
    // (last token of the array literal). Then advance() moves to ',' or '}'.
    // This is actually correct behaviour — but we need to make sure the map literal
    // parser inside parse_expression leaves current at ']' not past it.
    // The real fix: parse_expression already handles [...] via LeftBracket prefix,
    // leaving current on ']'. Then advance() in the field loop moves us to ',' or '}'.
    // This should work. The panic/error "Expected '}'" suggests routes value parsing
    // consumed the '}' of the serve block. That happens when the value is NOT just
    // a [...] literal but something else. Let's make the field loop more robust.
    // ---------------------------------------------------------------------------
    fn parse_serve_stmt(&mut self) -> Option<Stmt> {
        let span = self.current.span.clone();
        self.advance(); // past 'serve'
        if self.current.kind != TokenKind::Colon { 
            self.error("Expected ':' after 'serve'.");
            return None; 
        }
        self.advance(); // past ':'
        
        let name = if let Some(id) = self.parse_identifier_as_string_id(true) {
            id
        } else {
            self.error("Expected identifier after 'serve:'.");
            return None;
        };
        self.advance(); // past name
        
        if self.current.kind != TokenKind::LeftBrace { 
             self.error("Expected '{' to start 'serve' block.");
             return None; 
        }
        self.advance(); // past '{'
        
        let mut port = None;
        let mut host = None;
        let mut workers = None;
        let mut routes = None;
        
        while self.current.kind != TokenKind::RightBrace && self.current.kind != TokenKind::EOF {
             let field = if let Some(id) = self.parse_identifier_as_string_id(false) {
                 self.interner.lookup(id).to_string()
             } else {
                 // Can't parse field name — skip token to avoid infinite loop
                 self.advance();
                 continue;
             };
             self.advance(); // past field name
             
             if self.current.kind != TokenKind::Equal { 
                 self.error(&format!("Expected '=' after '{}' field in serve block.", field));
                 // Skip to next comma or closing brace
                 while self.current.kind != TokenKind::Comma 
                     && self.current.kind != TokenKind::RightBrace 
                     && self.current.kind != TokenKind::EOF {
                     self.advance();
                 }
                 if self.current.kind == TokenKind::Comma { self.advance(); }
                 continue;
             }
             self.advance(); // past '='
             
             let val = self.parse_expression(Precedence::Lowest)?;
             // parse_expression for bracket/map literals already consumes the closing
             // bracket, leaving current on the token AFTER the expression (comma or '}'.
             // For scalar expressions (int, string) current is still ON the last token.
             // We advance only if we're not already past the expression.
             if self.current.kind != TokenKind::Comma
                 && self.current.kind != TokenKind::RightBrace
                 && self.current.kind != TokenKind::EOF {
                 self.advance();
             }
             
             match field.as_str() {
                 "port"    => port    = Some(Box::new(val)),
                 "host"    => host    = Some(Box::new(val)),
                 "workers" => workers = Some(Box::new(val)),
                 "routes"  => routes  = Some(Box::new(val)),
                 _ => self.error(&format!("Unknown field '{}' in 'serve' block.", field)),
             }
             
             if self.current.kind == TokenKind::Comma {
                 self.advance();
             }
        }
        
        if self.current.kind != TokenKind::RightBrace { 
            self.error("Expected '}' at the end of 'serve' block.");
            return None; 
        }
        self.advance(); // past '}'
        self.expect_semicolon();
        
        let routes_val = match routes {
            Some(r) => r,
            None => {
                self.error("Field 'routes' is required in 'serve' block.");
                return None;
            }
        };

        Some(Stmt {
            kind: crate::parser::ast::StmtKind::Serve {
                name,
                port: port.unwrap_or_else(|| Box::new(crate::parser::ast::Expr { kind: crate::parser::ast::ExprKind::IntLiteral(8080), span: span.clone() })),
                host,
                workers,
                routes: routes_val,
            },
            span,
        })
    }

    fn parse_return_stmt(&mut self) -> Option<Stmt> {
        let span = self.current.span.clone();
        self.advance(); // past 'return'

        if self.current.kind == TokenKind::Semicolon {
            self.advance();
            return Some(Stmt { kind: crate::parser::ast::StmtKind::Return(None), span });
        }

        let value = self.parse_expression(Precedence::Lowest);
        if value.is_some() {
            self.advance();
        }
        self.expect_semicolon();

        Some(Stmt { kind: crate::parser::ast::StmtKind::Return(value), span })
    }

    fn parse_func_call_stmt(&mut self) -> Option<Stmt> {
        let span = self.current.span.clone();
        let name = if let Some(id) = self.parse_identifier_as_string_id(true) { id } else { return None; };
        self.advance(); // past name
        self.advance(); // past '('

        let mut args = Vec::new();
        while self.current.kind != TokenKind::RightParen && self.current.kind != TokenKind::EOF {
            if let Some(arg) = self.parse_expression(Precedence::Lowest) {
                args.push(arg);
            }
            self.advance();
            if self.current.kind == TokenKind::Comma {
                self.advance();
            }
        }

        if self.current.kind == TokenKind::RightParen {
            self.advance(); // past ')'
        }
        self.expect_semicolon();

        Some(Stmt {
            kind: crate::parser::ast::StmtKind::FunctionCallStmt { name, args },
            span,
        })
    }

    fn parse_expression(&mut self, precedence: Precedence) -> Option<Expr> {
        let mut left = self.parse_prefix()?;

        while self.peek.kind != TokenKind::Semicolon && self.peek.kind != TokenKind::EOF && precedence < self.peek_precedence() {
            self.advance();
            left = self.parse_infix(left)?;
        }

        Some(left)
    }

    fn parse_prefix(&mut self) -> Option<Expr> {
        let span = self.current.span.clone();
        match &self.current.kind {
        TokenKind::Dot => {
            if self.peek.kind == TokenKind::Terminal {
                self.advance(); 
                self.advance(); 
        
                if self.current.kind != TokenKind::Bang { return None; }
                self.advance(); 
        
                let command = if let Some(id) = self.parse_identifier_as_string_id(false) {
                    id
                } else {
                    return None;
        };
        let mut arg = None;
        if self.peek.kind != TokenKind::Semicolon 
            && self.peek.kind != TokenKind::RightParen 
            && self.peek.kind != TokenKind::Comma
            && self.peek.kind != TokenKind::EOF
        {
            self.advance(); 
            if let Some(expr) = self.parse_expression(Precedence::Lowest) {
                arg = Some(Box::new(expr));
            }
        }
        
        Some(Expr {
            kind: crate::parser::ast::ExprKind::TerminalCommand(command, arg),
            span,
        })
    } else {
        None
    }
            }
            TokenKind::Identifier(_) | TokenKind::TypeI | TokenKind::TypeF | TokenKind::TypeS | TokenKind::TypeB | TokenKind::Halt | TokenKind::Store | TokenKind::Terminal | TokenKind::Json | TokenKind::Choice |
            TokenKind::TypeSetN | TokenKind::TypeSetQ | TokenKind::TypeSetZ | TokenKind::TypeSetS | TokenKind::TypeSetB | TokenKind::TypeSetC => {
                let id_val = if let Some(id) = self.parse_identifier_as_string_id(false) {
                    id
                } else {
                    unreachable!()
                };
                let id_span = span.clone();
                if self.peek.kind == TokenKind::LeftParen {
                    self.advance(); // past identifier to '('
                    self.advance(); // past '('
                    let mut args = Vec::new();
                    while self.current.kind != TokenKind::RightParen && self.current.kind != TokenKind::EOF {
                        if let Some(arg) = self.parse_expression(Precedence::Lowest) {
                            args.push(arg);
                        }
                        self.advance();
                        if self.current.kind == TokenKind::Comma {
                            self.advance();
                        }
                    }
                    Some(Expr {
                        kind: crate::parser::ast::ExprKind::FunctionCall { name: id_val, args },
                        span: id_span,
                    })
                } else {
                    Some(Expr { kind: crate::parser::ast::ExprKind::Identifier(id_val), span })
                }
            }
            TokenKind::Union | TokenKind::Intersection | TokenKind::Difference | TokenKind::SymDifference | TokenKind::Alert | TokenKind::Error | TokenKind::Fatal | TokenKind::Columns | TokenKind::Rows | TokenKind::Schema | TokenKind::Data | TokenKind::Empty => {
                let text = match self.current.kind {
                    TokenKind::Union => "union",
                    TokenKind::Intersection => "intersection",
                    TokenKind::Difference => "difference",
                    TokenKind::SymDifference => "symmetric_difference",
                    TokenKind::Alert => "alert",
                    TokenKind::Error => "error",
                    TokenKind::Fatal => "fatal",
                    TokenKind::Columns => "columns",
                    TokenKind::Rows => "rows",
                    TokenKind::Schema => "schema",
                    TokenKind::Data => "data",
                    TokenKind::Empty => "EMPTY",
                    _ => unreachable!(),
                };
                let id = self.interner.intern(text);
                Some(Expr { kind: crate::parser::ast::ExprKind::Identifier(id), span })
            }
            TokenKind::IntLiteral(val) => Some(Expr { kind: crate::parser::ast::ExprKind::IntLiteral(*val), span }),
            TokenKind::FloatLiteral(val) => Some(Expr { kind: crate::parser::ast::ExprKind::FloatLiteral(*val), span }),
            TokenKind::StringLiteral(id) => Some(Expr { kind: crate::parser::ast::ExprKind::StringLiteral(*id), span }),
            TokenKind::True => Some(Expr { kind: crate::parser::ast::ExprKind::BoolLiteral(true), span }),
            TokenKind::False => Some(Expr { kind: crate::parser::ast::ExprKind::BoolLiteral(false), span }),
            TokenKind::Minus => {
                let op = self.current.kind.clone();
                self.advance();
                let right = self.parse_expression(Precedence::Prefix)?;
                Some(Expr {
                    kind: crate::parser::ast::ExprKind::Binary {
                        left: Box::new(Expr { kind: crate::parser::ast::ExprKind::IntLiteral(0), span: span.clone() }),
                        op,
                        right: Box::new(right),
                    },
                    span,
                })
            }
            TokenKind::Not | TokenKind::Bang => {
                let op = self.current.kind.clone();
                self.advance();
                let right = self.parse_expression(Precedence::Prefix)?;
                Some(Expr {
                    kind: crate::parser::ast::ExprKind::Unary {
                        op,
                        right: Box::new(right),
                    },
                    span,
                })
            }
            TokenKind::LeftParen => {
                self.advance(); // past '('
                let mut exprs = Vec::new();
                if self.current.kind != TokenKind::RightParen {
                    loop {
                        if let Some(e) = self.parse_expression(Precedence::Lowest) {
                            exprs.push(e);
                        } else {
                            return None;
                        }
                        self.advance();
                        if self.current.kind == TokenKind::Comma {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                }
                if self.current.kind != TokenKind::RightParen { return None; }
                
                if exprs.len() == 1 {
                    Some(exprs.remove(0))
                } else {
                    Some(Expr { kind: crate::parser::ast::ExprKind::Tuple(exprs), span: span.clone() })
                }
            }
            TokenKind::Set => {
                self.advance(); // past 'set'
                if self.current.kind != TokenKind::Colon {
                    return Some(Expr { kind: crate::parser::ast::ExprKind::Identifier(self.interner.intern("set")), span });
                }
                self.advance(); // past ':'
                let st = match self.current.kind {
                    TokenKind::TypeSetN => SetType::N,
                    TokenKind::TypeSetQ => SetType::Q,
                    TokenKind::TypeSetZ => SetType::Z,
                    TokenKind::TypeSetS => SetType::S,
                    TokenKind::TypeSetB => SetType::B,
                    TokenKind::TypeSetC => SetType::C,
                    _ => return None,
                };
                self.advance(); // past TypeSetX
                if self.current.kind == TokenKind::LeftBrace {
                    self.advance(); // past '{'
                    self.parse_set_literal_content(st, span)
                } else {
                    // Just the type as an identifier? 
                    // Technically set:N could be used standalone in some contexts, 
                    // but for compatibility we return it as an identifier if no brace follows.
                    let name = match st {
                        SetType::N => "set:N",
                        SetType::Q => "set:Q",
                        SetType::Z => "set:Z",
                        SetType::S => "set:S",
                        SetType::B => "set:B",
                        SetType::C => "set:C",
                    };
                    Some(Expr { kind: crate::parser::ast::ExprKind::Identifier(self.interner.intern(name)), span })
                }
            }
            TokenKind::LeftBrace => {
                let lit_span = span.clone();
                if self.peek.kind == TokenKind::Schema {
                    return self.parse_map_literal();
                }
                
                self.advance(); // past '{'
                let elements = self.parse_array_or_set_literal_elements(TokenKind::RightBrace);
                Some(Expr { kind: crate::parser::ast::ExprKind::ArrayOrSetLiteral { elements }, span: lit_span })
            }
            TokenKind::LeftBracket => {
                let lit_span = span.clone();
                self.advance(); // past '['
                
                if self.current.kind == TokenKind::RightBracket {
                    return Some(Expr {
                        kind: crate::parser::ast::ExprKind::ArrayLiteral { elements: Vec::new() },
                        span: lit_span,
                    });
                }

                let first_expr = self.parse_expression(Precedence::Lowest)?;
                self.advance();
                
                if self.current.kind == TokenKind::DoubleColon {
                    self.advance(); // past '::'
                    let first_val = self.parse_expression(Precedence::Lowest)?;
                    self.advance();
                    
                    let mut elements = vec![(first_expr, first_val)];
                    
                    if self.current.kind == TokenKind::Comma {
                        self.advance();
                    }
                    
                    while self.current.kind != TokenKind::RightBracket && self.current.kind != TokenKind::EOF {
                        let k = self.parse_expression(Precedence::Lowest)?;
                        self.advance();
                        if self.current.kind != TokenKind::DoubleColon {
                             self.error("Expected '::' in map literal.");
                             return None;
                        }
                        self.advance();
                        let v = self.parse_expression(Precedence::Lowest)?;
                        self.advance();
                        elements.push((k, v));
                        if self.current.kind == TokenKind::Comma {
                            self.advance();
                        }
                    }
                    
                    if self.current.kind == TokenKind::RightBracket {
                        // Stop on ']'
                    }
                    
                    Some(Expr {
                        kind: crate::parser::ast::ExprKind::MapLiteral {
                            key_type: Type::String, 
                            value_type: Type::Unknown,
                            elements,
                        },
                        span: lit_span,
                    })
                } else {
                    let mut elements = vec![first_expr];
                    if self.current.kind == TokenKind::Comma {
                        self.advance();
                    }
                    elements.extend(self.parse_array_or_set_literal_elements(TokenKind::RightBracket));
                    
                    Some(Expr {
                        kind: crate::parser::ast::ExprKind::ArrayLiteral { elements },
                        span: lit_span,
                    })
                }
            }
            TokenKind::Random => {
                self.advance(); // past 'random'
                if self.current.kind != TokenKind::Dot { return None; }
                self.advance(); // past '.'
                if self.current.kind != TokenKind::Choice { return None; }
                self.advance(); // past 'choice'
                if self.current.kind != TokenKind::From { return None; }
                self.advance(); // past 'from'
                
                let set_expr = self.parse_expression(Precedence::Lowest)?;
                
                Some(Expr {
                    kind: crate::parser::ast::ExprKind::RandomChoice {
                        set: Box::new(set_expr),
                    },
                    span,
                })
            }
            TokenKind::Date => {
                let span = self.current.span.clone();
                if self.peek.kind == TokenKind::LeftParen {
                    self.advance(); // past 'date'
                    self.advance(); // past '('
                    let date_string = if let TokenKind::StringLiteral(id) = self.current.kind { id } else { return None; };
                    self.advance();
                    let format = if self.current.kind == TokenKind::Comma {
                        self.advance();
                        if let TokenKind::StringLiteral(fmt_id) = self.current.kind { self.advance(); Some(fmt_id) } else { return None; }
                    } else { None };
                    if self.current.kind != TokenKind::RightParen { return None; }
                    Some(Expr { kind: crate::parser::ast::ExprKind::DateLiteral { date_string, format }, span })
                } else {
                    let id = self.interner.intern("date");
                    Some(Expr { kind: crate::parser::ast::ExprKind::Identifier(id), span })
                }
            }
            TokenKind::Net => {
                // In expression context: net.get/post/put/delete/patch/respond(...)
                // net.request in expr context is not supported (use as stmt instead)
                self.advance(); // past 'net'
                if self.current.kind != TokenKind::Dot { 
                    self.error("Expected '.' after 'net'.");
                    return None; 
                }
                self.advance(); // past '.'
                
                let method_name = if let TokenKind::Identifier(id) = self.current.kind {
                    self.interner.lookup(id).to_string()
                } else {
                    self.error("Expected method name after 'net.'.");
                    return None;
                };
                
                match method_name.as_str() {
                    // also in expression context
                    "get" | "post" | "put" | "delete" | "patch" | "head" | "options" => {
                        let method_id = self.interner.intern(&method_name);
                        self.advance(); // past method name
                        if self.current.kind != TokenKind::LeftParen { 
                            self.error(&format!("Expected '(' after 'net.{}'.", method_name));
                            return None; 
                        }
                        self.advance(); // past '('
                        
                        let url = self.parse_expression(Precedence::Lowest)?;
                        self.advance(); // past url
                        
                        let mut body = None;
                        if self.current.kind == TokenKind::Comma {
                            self.advance();
                            body = Some(Box::new(self.parse_expression(Precedence::Lowest)?));
                            self.advance();
                        }
                        
                        if self.current.kind != TokenKind::RightParen { 
                             self.error(&format!("Expected ')' after 'net.{}' arguments.", method_name));
                             return None; 
                        }
                        // Leaves current at ')'
                        Some(Expr {
                            kind: crate::parser::ast::ExprKind::NetCall {
                                method: method_id,
                                url: Box::new(url),
                                body,
                            },
                            span,
                        })
                    }
                    "respond" => {
                        self.advance(); // past 'respond'
                        if self.current.kind != TokenKind::LeftParen { 
                            self.error("Expected '(' after 'net.respond'.");
                            return None; 
                        }
                        self.advance(); // past '('
                        
                        let status = self.parse_expression(Precedence::Lowest)?;
                        self.advance();
                        
                        if self.current.kind != TokenKind::Comma { 
                            self.error("Expected ',' after status in 'net.respond'.");
                            return None; 
                        }
                        self.advance(); // past ','
                        
                        let body = self.parse_expression(Precedence::Lowest)?;
                        self.advance();
                        
                        let mut headers = None;
                        if self.current.kind == TokenKind::Comma {
                            self.advance();
                            headers = Some(Box::new(self.parse_expression(Precedence::Lowest)?));
                            self.advance();
                        }
                        
                        if self.current.kind != TokenKind::RightParen { 
                            self.error("Expected ')' after 'net.respond' arguments.");
                            return None; 
                        }
                        // Leaves current at ')'
                        Some(Expr {
                            kind: crate::parser::ast::ExprKind::NetRespond {
                                status: Box::new(status),
                                body: Box::new(body),
                                headers,
                            },
                            span,
                        })
                    }
                    "request" => {
                        // net.request in expression context — redirect to statement parser
                        // This shouldn't normally appear here; emit error.
                        self.error("'net.request { }' must be used as a statement with 'as target', not as an expression.");
                        None
                    }
                    _ => {
                        self.error(&format!("Unknown method 'net.{}'", method_name));
                        None
                    }
                }
            }
            TokenKind::RawBlock(id) => {
                let id = *id;
                Some(Expr {
                    kind: crate::parser::ast::ExprKind::RawBlock(id),
                    span,
                })
            }
            TokenKind::Map => self.parse_map_literal(),
            TokenKind::Table => self.parse_table_literal(),
            _ => None,
        }
    }

    fn parse_infix(&mut self, left: Expr) -> Option<Expr> {
        let op = self.current.kind.clone();
        if op == TokenKind::Dot {
             return self.parse_dot_infix(left);
        }
        if op == TokenKind::LeftBracket {
             return self.parse_index_infix(left);
        }
        if op == TokenKind::Arrow {
             return self.parse_lambda_infix(left);
        }

        let span = left.span.clone();
        let precedence = self.current_precedence();
        
        let next_precedence = if op == TokenKind::Caret {
            match precedence {
                Precedence::Power => Precedence::Sum,
                p => p,
            }
        } else {
            precedence
        };

        self.advance();
        let right = self.parse_expression(next_precedence)?;
        Some(Expr {
            kind: crate::parser::ast::ExprKind::Binary {
                left: Box::new(left),
                op,
                right: Box::new(right),
            },
            span,
        })
    }

    fn parse_dot_infix(&mut self, receiver: Expr) -> Option<Expr> {
        let span = receiver.span.clone();
        self.advance(); // past '.'

        // Handle .["key"] — dot-bracket index access (e.g. headers.["X-Client"])
        if self.current.kind == TokenKind::LeftBracket {
            self.advance(); // past '['
            let index = self.parse_expression(Precedence::Lowest)?;
            self.advance(); // past index expr to ']'
            if self.current.kind != TokenKind::RightBracket { return None; }
            return Some(Expr {
                kind: crate::parser::ast::ExprKind::Index {
                    receiver: Box::new(receiver),
                    index: Box::new(index),
                },
                span,
            });
        }
        
        let member = if let Some(id) = self.parse_identifier_as_string_id(false) {
            id
        } else {
            return None;
        };
        
        if self.peek.kind == TokenKind::LeftParen {
            self.advance(); // past member name, now at '('
            self.advance(); // past '('
            let mut args = Vec::new();
            while self.current.kind != TokenKind::RightParen && self.current.kind != TokenKind::EOF {
                if let Some(arg) = self.parse_expression(Precedence::Lowest) {
                    args.push(arg);
                }
                
                if self.peek.kind == TokenKind::Comma || self.peek.kind == TokenKind::RightParen {
                    self.advance();
                }

                if self.current.kind == TokenKind::Comma {
                    self.advance();
                }
            }
            
            Some(Expr {
                kind: crate::parser::ast::ExprKind::MethodCall {
                    receiver: Box::new(receiver),
                    method: member,
                    args,
                },
                span,
            })
        } else {
            Some(Expr {
                kind: crate::parser::ast::ExprKind::MemberAccess {
                    receiver: Box::new(receiver),
                    member,
                },
                span,
            })
        }
    }

    fn parse_index_infix(&mut self, receiver: Expr) -> Option<Expr> {
        let span = receiver.span.clone();
        self.advance(); // past '['
        let index = self.parse_expression(Precedence::Lowest)?;
        self.advance(); // past index expression to ']'
        if self.current.kind != TokenKind::RightBracket { return None; }
        Some(Expr {
            kind: crate::parser::ast::ExprKind::Index {
                receiver: Box::new(receiver),
                index: Box::new(index),
            },
            span,
        })
    }

    fn is_type_intro(&self, kind: &TokenKind) -> bool {
        matches!(kind, TokenKind::TypeI | TokenKind::TypeF | TokenKind::TypeS | TokenKind::TypeB | 
                 TokenKind::Date | TokenKind::Json | TokenKind::Set | TokenKind::Map | 
                 TokenKind::Table | TokenKind::Fiber | TokenKind::Array)
    }

    fn parse_type(&mut self) -> Option<Type> {
        let mut is_array = false;
        if self.current.kind == TokenKind::Array {
            is_array = true;
            self.advance();
            if self.current.kind != TokenKind::Colon { return None; }
            self.advance();
        }

        let ty = match self.current.kind {
            TokenKind::TypeI => { self.advance(); Type::Int },
            TokenKind::TypeF => { self.advance(); Type::Float },
            TokenKind::TypeS => { self.advance(); Type::String },
            TokenKind::TypeB => { self.advance(); Type::Bool },
            TokenKind::Date => { self.advance(); Type::Date },
            TokenKind::Json => { self.advance(); Type::Json },
            TokenKind::Set => {
                self.advance();
                if self.current.kind == TokenKind::Colon {
                    self.advance();
                    let st = match self.current.kind {
                        TokenKind::TypeSetN => SetType::N,
                        TokenKind::TypeSetQ => SetType::Q,
                        TokenKind::TypeSetZ => SetType::Z,
                        TokenKind::TypeSetS => SetType::S,
                        TokenKind::TypeSetB => SetType::B,
                        TokenKind::TypeSetC => SetType::C,
                        _ => return None,
                    };
                    self.advance();
                    Type::Set(st)
                } else {
                    Type::Set(SetType::N)
                }
            }
            TokenKind::Map => {
                self.advance(); // past 'map'
                if self.current.kind == TokenKind::Colon && self.is_type_intro(&self.peek.kind) {
                    self.advance(); // past ':'
                    let k_ty = self.parse_type()?;
                    if self.current.kind == TokenKind::Bridge {
                        self.advance(); // past '<->'
                        let v_ty = self.parse_type()?;
                        Type::Map(Box::new(k_ty), Box::new(v_ty))
                    } else {
                        Type::Map(Box::new(k_ty), Box::new(Type::Int))
                    }
                } else {
                    Type::Map(Box::new(Type::Int), Box::new(Type::Int))
                }
            }
            TokenKind::Table => {
                self.advance();
                Type::Table(Vec::new())
            }
            TokenKind::Fiber => {
                self.advance();
                let inner = if self.current.kind == TokenKind::Colon {
                    self.advance();
                    self.parse_type().map(Box::new)
                } else {
                    None
                };
                Type::Fiber(inner)
            }
            _ => return None,
        };

        if is_array {
            Some(Type::Array(Box::new(ty)))
        } else {
            Some(ty)
        }
    }

    fn parse_table_literal(&mut self) -> Option<Expr> {
        let span = self.current.span.clone();
        if self.current.kind == TokenKind::Table {
            self.advance();
        }
        if self.current.kind != TokenKind::LeftBrace { return None; }
        self.advance(); // past '{'

        if self.current.kind != TokenKind::Columns { return None; }
        self.advance();
        if self.current.kind != TokenKind::Colon && self.current.kind != TokenKind::Equal { return None; }
        self.advance();
        if self.current.kind != TokenKind::LeftBracket { return None; }
        self.advance();

        let mut columns = Vec::new();
        while self.current.kind != TokenKind::RightBracket && self.current.kind != TokenKind::EOF {
            let col_name = if let Some(id) = self.parse_identifier_as_string_id(false) {
                id
            } else {
                return None;
            };
            self.advance();
            if self.current.kind != TokenKind::DoubleColon { return None; }
            self.advance();

            let col_ty = self.parse_type()?;
            
            let mut is_auto = false;
            if self.current.kind == TokenKind::AtAuto {
                is_auto = true;
                self.advance();
            }

            columns.push(crate::parser::ast::ColumnDef {
                name: col_name,
                ty: col_ty,
                is_auto,
            });

            if self.current.kind == TokenKind::Comma {
                self.advance();
            }
        }
        if self.current.kind == TokenKind::RightBracket {
            self.advance();
        }
        
        if self.current.kind == TokenKind::Comma {
            self.advance();
        }

        if self.current.kind != TokenKind::Rows { return None; }
        self.advance();
        if self.current.kind != TokenKind::Colon && self.current.kind != TokenKind::Equal { return None; }
        self.advance();
        if self.current.kind != TokenKind::LeftBracket { return None; }
        self.advance();

        let mut rows = Vec::new();
        if self.current.kind == TokenKind::Empty {
            self.advance();
        } else {
        while self.current.kind != TokenKind::RightBracket && self.current.kind != TokenKind::EOF {
            let mut row_vals = Vec::new();
            if self.current.kind == TokenKind::LeftParen || self.current.kind == TokenKind::LeftBracket {
                let close_kind = if self.current.kind == TokenKind::LeftParen { TokenKind::RightParen } else { TokenKind::RightBracket };
                self.advance();
                
                while self.current.kind != close_kind && self.current.kind != TokenKind::EOF {
                    if let Some(val) = self.parse_expression(Precedence::Lowest) {
                        row_vals.push(val);
                    }
                    self.advance();
                    if self.current.kind == TokenKind::Comma {
                        self.advance();
                    }
                }
                if self.current.kind == close_kind {
                    self.advance();
                }
            }
            
            rows.push(row_vals);
            
            if self.current.kind == TokenKind::Comma {
                self.advance();
            }
        }
        }
        
        if self.current.kind == TokenKind::RightBracket {
            self.advance();
        }
        
        if self.current.kind == TokenKind::RightBrace {
            self.advance();
        }

        Some(Expr {
            kind: crate::parser::ast::ExprKind::TableLiteral { columns, rows },
            span,
        })
    }

    fn parse_lambda_infix(&mut self, left: Expr) -> Option<Expr> {
    let span = left.span.clone();
    self.advance(); // past '->'
    
    let params: Vec<(Type, StringId)> = match left.kind {
        crate::parser::ast::ExprKind::Identifier(id) => {
            vec![(Type::Unknown, id)]
        }
        crate::parser::ast::ExprKind::Tuple(exprs) => {
            let mut ids = Vec::new();
            for e in exprs {
                if let crate::parser::ast::ExprKind::Identifier(id) = e.kind {
                    ids.push((Type::Unknown, id));
                } else {
                    return None;
                }
            }
            ids
        }
            _ => return None,
        };
    
        let body = self.parse_expression(Precedence::Lowest)?;
    
        Some(Expr {
            kind: crate::parser::ast::ExprKind::Lambda {
                params,
                return_type: None,
                body: Box::new(body),
            },
            span,
        })
    }

    fn parse_map_literal(&mut self) -> Option<Expr> {
        let span = self.current.span.clone();
        if self.current.kind == TokenKind::Map {
            self.advance();
        }
        if self.current.kind != TokenKind::LeftBrace { return None; }
        self.advance(); // past '{'
        
        if self.current.kind != TokenKind::Schema { return None; }
        self.advance();
        if self.current.kind != TokenKind::Equal { return None; }
        self.advance();
        if self.current.kind != TokenKind::LeftBracket { return None; }
        self.advance();
        
        let k_ty = match self.current.kind {
            TokenKind::TypeI => Type::Int,
            TokenKind::TypeF => Type::Float,
            TokenKind::TypeS => Type::String,
            TokenKind::TypeB => Type::Bool,
            _ => return None,
        };
        self.advance();
        if self.current.kind != TokenKind::Bridge { return None; }
        self.advance();
        let v_ty = match self.current.kind {
            TokenKind::TypeI => Type::Int,
            TokenKind::TypeF => Type::Float,
            TokenKind::TypeS => Type::String,
            TokenKind::TypeB => Type::Bool,
            _ => return None,
        };
        self.advance();
        if self.current.kind != TokenKind::RightBracket { return None; }
        self.advance();
        
        if self.current.kind != TokenKind::Data { return None; }
        self.advance();
        if self.current.kind != TokenKind::Equal { return None; }
        self.advance();
        if self.current.kind != TokenKind::LeftBracket { return None; }
        self.advance();
        
        let mut elements = Vec::new();
        if self.current.kind == TokenKind::Empty {
            self.advance();
            if self.current.kind != TokenKind::RightBracket { return None; }
            self.advance();
        } else {
            while self.current.kind != TokenKind::RightBracket && self.current.kind != TokenKind::EOF {
                let key_expr = self.parse_expression(Precedence::Lowest)?;
                self.advance();
                if self.current.kind != TokenKind::DoubleColon { return None; }
                self.advance();
                
                let val_expr = self.parse_expression(Precedence::Lowest)?;
                elements.push((key_expr, val_expr));
                
                self.advance();
                if self.current.kind == TokenKind::Comma {
                    self.advance();
                }
            }
            if self.current.kind == TokenKind::RightBracket {
                self.advance();
            }
        }
        
        if self.current.kind != TokenKind::RightBrace { return None; }
        self.advance();
        
        Some(Expr {
            kind: crate::parser::ast::ExprKind::MapLiteral {
                key_type: k_ty,
                value_type: v_ty,
                elements,
            },
            span,
        })
    }
}