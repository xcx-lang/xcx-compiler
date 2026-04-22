use crate::parser::ast::{Program, Stmt, Expr, Type, SetType, ExprKind, ColumnDef, Argument, ColumnAttribute};
use crate::lexer::token::{TokenKind, Span};
use crate::sema::interner::Interner;
use crate::sema::symbol_table::SymbolTable;
use std::collections::HashMap;
use std::borrow::Cow;

/// Core semantic analyzer and type checker for the XCX language.
/// Maintains state about current fiber context, loop depth, and interner
/// to perform thorough validation of the AST before compilation.
pub struct Checker<'a> {
    interner: &'a Interner,
    loop_depth: usize,
    functions: HashMap<String, FunctionSignature>,
    fiber_context: Option<Option<Type>>,
    is_table_lambda: bool,
    fiber_has_yield: bool,
    in_yield_expr: bool,
    last_expr_was_db_io: bool,
}

#[derive(Debug, PartialEq)]
pub enum TypeErrorKind {
    UndefinedVariable(String),
    RedefinedVariable(String),
    ConstReassignment(String),
    TypeMismatch { expected: Type, actual: Type },
    InvalidBinaryOp { op: TokenKind, left: Type, right: Type },
    BreakOutsideLoop,
    ContinueOutsideLoop,
    YieldOutsideFiber,
    FiberTypeMismatch,
    ReturnTypeMismatchInFiber,
    WherePredicateNameCollision { var_name: String, column_name: String },
    IndexAccessNotSupported(Type),
    PropertyNotFound { base_type: Type, property: String },
    MethodNotFound { base_type: Type, method: String },
    TableRowCountMismatch { expected: usize, actual: usize },
    InvalidArgumentCount { expected: usize, actual: usize },
    CannotIterateOverVoidFiber,
    CannotRunTypedFiber,
    Other(String),
}

impl TypeErrorKind {
    pub fn to_diagnostic_message(&self) -> String {
        match self {
            TypeErrorKind::UndefinedVariable(name) =>
                format!("[S101] Undefined variable: {}", name),
            TypeErrorKind::RedefinedVariable(name) =>
                format!("[S102] Redefined variable: {}", name),
            TypeErrorKind::TypeMismatch { expected, actual } =>
                format!("[S103] Type mismatch: expected {}, got {}", expected, actual),
            TypeErrorKind::InvalidBinaryOp { op, left, right } =>
                format!("[S104] Invalid operation {:?} between {} and {}", op, left, right),
            TypeErrorKind::ConstReassignment(name) =>
                format!("[S105] Cannot reassign to constant variable: {}", name),
            TypeErrorKind::BreakOutsideLoop =>
                "[S106] Break statement outside of loop".to_string(),
            TypeErrorKind::ContinueOutsideLoop =>
                "[S107] Continue statement outside of loop".to_string(),
            TypeErrorKind::IndexAccessNotSupported(ty) =>
                format!("[S108] Index access not supported for type {}", ty),
            TypeErrorKind::PropertyNotFound { base_type, property } =>
                format!("[S109] Property '{}' not found on type {}", property, base_type),
            TypeErrorKind::MethodNotFound { base_type, method } =>
                format!("[S110] Method '{}' not found on type {}", method, base_type),
            TypeErrorKind::InvalidArgumentCount { expected, actual } =>
                format!("[S111] Incorrect number of arguments: expected {}, got {}", expected, actual),
            TypeErrorKind::YieldOutsideFiber =>
                "[S208] 'yield' used outside a fiber body".to_string(),
            TypeErrorKind::FiberTypeMismatch =>
                "[S209] Cannot use 'yield expr;' inside a void fiber — use 'yield;' instead".to_string(),
            TypeErrorKind::ReturnTypeMismatchInFiber =>
                "[S210] Typed fiber requires 'return expr;' not plain 'return;'".to_string(),
            TypeErrorKind::CannotIterateOverVoidFiber =>
                "[S211] Cannot iterate over a void fiber (fiber without yield)".to_string(),
            TypeErrorKind::CannotRunTypedFiber =>
                "[S212] Cannot call .run() on a typed fiber (use for loop instead)".to_string(),
            TypeErrorKind::WherePredicateNameCollision { var_name, column_name } =>
                format!("[S301] Variable name '{}' conflicts with column '{}' in .where() predicate — rename the local variable",
                    var_name, column_name),
            TypeErrorKind::TableRowCountMismatch { expected, actual } =>
                format!("[S302] Table row has {} columns, but schema expects {}", actual, expected),
            TypeErrorKind::Other(msg) => msg.clone(),
        }
    }
}

#[derive(Debug, Clone)]
struct FunctionSignature {
    params: Vec<Type>,
    return_type: Option<Type>,
    is_fiber: bool,
}

#[derive(Debug, PartialEq)]
pub struct TypeError {
    pub kind: TypeErrorKind,
    pub span: Span,
}

impl<'a> Checker<'a> {
    pub fn new(interner: &'a Interner) -> Self {
        Self {
            interner,
            loop_depth: 0,
            functions: std::collections::HashMap::new(),
            fiber_context: None,
            is_table_lambda: false,
            fiber_has_yield: false,
            in_yield_expr: false,
            last_expr_was_db_io: false,
        }
    }

    pub fn check(&mut self, program: &mut Program, symbols: &mut SymbolTable<'_>) -> Vec<TypeError> {
        let mut errors = Vec::new();

        symbols.define("input".to_string(), Type::Unknown, true);

        self.functions.insert("i".to_string(), FunctionSignature { params: vec![Type::Unknown], return_type: Some(Type::Int), is_fiber: false });
        self.functions.insert("f".to_string(), FunctionSignature { params: vec![Type::Unknown], return_type: Some(Type::Float), is_fiber: false });
        self.functions.insert("s".to_string(), FunctionSignature { params: vec![Type::Unknown], return_type: Some(Type::String), is_fiber: false });
        self.functions.insert("b".to_string(), FunctionSignature { params: vec![Type::Unknown], return_type: Some(Type::Bool), is_fiber: false });

        self.pre_scan_stmts(&program.stmts, symbols);

        for stmt in &mut program.stmts {
            self.check_stmt(stmt, symbols, &mut errors);
        }
        errors
    }

    fn pre_scan_stmts(&mut self, stmts: &[Stmt], symbols: &mut SymbolTable<'_>) {
        for stmt in stmts {
            match &stmt.kind {
                crate::parser::ast::StmtKind::FunctionDef { name, params, return_type, .. } => {
                    let name_str = self.interner.lookup(*name).trim().to_string();
                    let param_types = params.iter().map(|(ty, _)| ty.clone()).collect();
                    let sig = FunctionSignature {
                        params: param_types,
                        return_type: return_type.clone(),
                        is_fiber: false,
                    };
                    self.functions.insert(name_str.clone(), sig);
                    symbols.define(name_str, Type::Unknown, false);
                }
                crate::parser::ast::StmtKind::FiberDef { name, params, return_type, .. } => {
                    let name_str = self.interner.lookup(*name).trim().to_string();
                    let param_types = params.iter().map(|(ty, _)| ty.clone()).collect();
                    let sig = FunctionSignature {
                        params: param_types,
                        return_type: return_type.clone(),
                        is_fiber: true,
                    };
                    self.functions.insert(name_str.clone(), sig);
                    let var_type = Type::Fiber(return_type.as_ref().map(|t| Box::new(t.clone())));
                    symbols.define(name_str, var_type, false);
                }
                _ => {}
            }
        }
    }

    fn collect_pred_idents(&self, expr: &Expr, out: &mut Vec<String>) {
        match &expr.kind {
            ExprKind::Identifier(id) => {
                out.push(self.interner.lookup(*id).trim().to_string());
            }
            ExprKind::Binary { left, right, .. } => {
                self.collect_pred_idents(left, out);
                self.collect_pred_idents(right, out);
            }
            ExprKind::Unary { right, .. } => {
                self.collect_pred_idents(right, out);
            }
            ExprKind::MethodCall { receiver, args, .. } => {
                self.collect_pred_idents(receiver, out);
                for a in args { self.collect_pred_idents(a.expr(), out); }
            }
            ExprKind::FunctionCall { args, .. } => {
                for a in args { self.collect_pred_idents(a.expr(), out); }
            }
            ExprKind::Lambda { body, .. } => {
                self.collect_pred_idents(body, out);
            }
            _ => {}
        }
    }

    fn check_stmt(&mut self, stmt: &mut Stmt, symbols: &mut SymbolTable<'_>, errors: &mut Vec<TypeError>) {
        let span = stmt.span.clone();
        match &mut stmt.kind {
            crate::parser::ast::StmtKind::VarDecl { is_const, ty, name, value } => {
                let name_str_raw = self.interner.lookup(*name);
                let name_str = name_str_raw.trim().to_string();
                if symbols.has_in_current_scope(&name_str) {
                    errors.push(TypeError { kind: TypeErrorKind::RedefinedVariable(name_str.clone()), span: span.clone() });
                }
                if let Some(val) = value {
                    let val_ty = self.check_expr_with_context(val, symbols, errors, Some(ty.clone()));
                    if *ty != Type::Unknown && val_ty != Type::Unknown {
                        if !self.is_compatible(ty, &val_ty) {
                            errors.push(TypeError {
                                kind: TypeErrorKind::TypeMismatch {
                                    expected: ty.clone(),
                                    actual: val_ty.clone(),
                                },
                                span: val.span.clone(),
                            });
                        }
                        if let (Type::Table(e_cols), Type::Table(a_cols)) = (&*ty, &val_ty) {
                            if e_cols.is_empty() && !a_cols.is_empty() {
                                *ty = val_ty.clone();
                            }
                        }
                    }
                    if *ty == Type::Unknown { *ty = val_ty; }
                }
                let var_ty = ty.clone();
                symbols.define(name_str, var_ty, *is_const);
            }
            crate::parser::ast::StmtKind::Input(name, ty) => {
                let name_str = self.interner.lookup(*name).trim().to_string();
                if let Some(resolved_ty) = symbols.lookup(&name_str) {
                    *ty = resolved_ty;
                } else {
                    errors.push(TypeError { kind: TypeErrorKind::UndefinedVariable(name_str), span });
                }
            }
            crate::parser::ast::StmtKind::Print(expr) | crate::parser::ast::StmtKind::TerminalWrite(expr) => {
                self.check_expr(expr, symbols, errors);
            }
            crate::parser::ast::StmtKind::Halt { message, .. } => {
                self.check_expr(message, symbols, errors);
            }
            crate::parser::ast::StmtKind::FunctionDef { name, params, return_type, body } => {
                let name_str = self.interner.lookup(*name).trim().to_string();
                if !symbols.has(&name_str) {
                    symbols.define(name_str.clone(), Type::Unknown, false);
                }

                let mut func_symbols = SymbolTable::new_with_parent(symbols);
                let prev_ctx = self.fiber_context.take();
                self.fiber_context = Some(return_type.clone());
                
                func_symbols.enter_scope();
                func_symbols.define(name_str, Type::Unknown, false);

                for (ty, param_name) in params {
                    let p_name_str = self.interner.lookup(*param_name).trim().to_string();
                    func_symbols.define(p_name_str, ty.clone(), false);
                }
                for s in body {
                    self.check_stmt(s, &mut func_symbols, errors);
                }
                self.fiber_context = prev_ctx;
            }
            crate::parser::ast::StmtKind::Return(expr) => {
                let context = self.fiber_context.clone();
                match context {
                    Some(Some(expected)) => {
                        if let Some(e) = expr {
                            let actual = self.check_expr(e, symbols, errors);
                            if actual != Type::Unknown && !self.is_compatible(&expected, &actual) {
                                errors.push(TypeError {
                                    kind: TypeErrorKind::TypeMismatch { expected, actual },
                                    span: e.span.clone(),
                                });
                            }
                        } else {
                            errors.push(TypeError {
                                kind: TypeErrorKind::ReturnTypeMismatchInFiber,
                                span: span.clone(),
                            });
                        }
                    }
                    Some(None) => {
                        if let Some(e) = expr {
                            let _ = self.check_expr(e, symbols, errors);
                        }
                    }
                    None => {
                        if let Some(e) = expr {
                            self.check_expr(e, symbols, errors);
                        }
                    }
                }
            }
            crate::parser::ast::StmtKind::ExprStmt(expr) => {
                let ty = self.check_expr(expr, symbols, errors);
                // Rule D401: remove() must have .where()
                if let Type::DatabaseOperation(crate::parser::ast::DatabaseOpKind::Remove, _) = ty {
                    errors.push(TypeError { 
                        kind: TypeErrorKind::Other("Rule D401: remove() requires .where() filter".to_string()), 
                        span: expr.span.clone() 
                    });
                }
            }
            crate::parser::ast::StmtKind::If { condition, then_branch, else_ifs, else_branch } => {
                let cond_ty = self.check_expr(condition, symbols, errors);
                if cond_ty != Type::Bool && cond_ty != Type::Unknown {
                    errors.push(TypeError {
                        kind: TypeErrorKind::TypeMismatch { expected: Type::Bool, actual: cond_ty },
                        span: condition.span.clone()
                    });
                }
                symbols.enter_scope();
                for stmt in then_branch {
                    self.check_stmt(stmt, symbols, errors);
                }
                symbols.exit_scope();
                for (elif_cond, elif_branch) in else_ifs {
                    let elif_ty = self.check_expr(elif_cond, symbols, errors);
                    if elif_ty != Type::Bool && elif_ty != Type::Unknown {
                        errors.push(TypeError {
                            kind: TypeErrorKind::TypeMismatch { expected: Type::Bool, actual: elif_ty },
                            span: elif_cond.span.clone()
                        });
                    }
                    symbols.enter_scope();
                    for stmt in elif_branch {
                        self.check_stmt(stmt, symbols, errors);
                    }
                    symbols.exit_scope();
                }
                if let Some(branch) = else_branch {
                    symbols.enter_scope();
                    for stmt in branch {
                        self.check_stmt(stmt, symbols, errors);
                    }
                    symbols.exit_scope();
                }
            }
            crate::parser::ast::StmtKind::While { condition, body } => {
                let cond_ty = self.check_expr(condition, symbols, errors);
                if cond_ty != Type::Bool && cond_ty != Type::Unknown {
                    errors.push(TypeError { kind: TypeErrorKind::TypeMismatch { expected: Type::Bool, actual: cond_ty }, span: condition.span.clone() });
                }
                self.loop_depth += 1;
                symbols.enter_scope();
                for s in body {
                    self.check_stmt(s, symbols, errors);
                }
                symbols.exit_scope();
                self.loop_depth -= 1;
            }
            crate::parser::ast::StmtKind::For { var_name, start, end, step, body, iter_type } => {
                let start_ty = self.check_expr(start, symbols, errors);

                if *iter_type != crate::parser::ast::ForIterType::Range {
                    let inner = match start_ty {
                        Type::Array(inner) => {
                            *iter_type = crate::parser::ast::ForIterType::Array;
                            *inner
                        }
                        Type::Set(st) => {
                            *iter_type = crate::parser::ast::ForIterType::Set;

                            match st {
                                crate::parser::ast::SetType::N | crate::parser::ast::SetType::Z => Type::Int,
                                crate::parser::ast::SetType::Q => Type::Float,
                                crate::parser::ast::SetType::S | crate::parser::ast::SetType::C => Type::String,
                                crate::parser::ast::SetType::B => Type::Bool,
                            }
                        }
                        Type::Table(cols) => {
                            *iter_type = crate::parser::ast::ForIterType::Array;
                            Type::Table(cols.clone())
                        }
                        Type::Fiber(inner) => {
                            *iter_type = crate::parser::ast::ForIterType::Fiber;
                            if let Some(t) = inner {
                                *t.clone()
                            } else {
                                errors.push(TypeError {
                                    kind: TypeErrorKind::CannotIterateOverVoidFiber,
                                    span: span.clone(),
                                });
                                Type::Unknown
                            }
                        }
                        Type::Unknown => Type::Unknown,
                        _ => {
                            errors.push(TypeError {
                                kind: TypeErrorKind::TypeMismatch {
                                    expected: Type::Array(Box::new(Type::Int)),
                                    actual: start_ty
                                },
                                span: start.span.clone()
                            });
                            Type::Unknown
                        }
                    };

                    if let Some(step_expr) = step {
                        let step_ty = self.check_expr(step_expr, symbols, errors);
                        if step_ty != Type::Int && step_ty != Type::Unknown {
                            errors.push(TypeError { kind: TypeErrorKind::TypeMismatch { expected: Type::Int, actual: step_ty }, span: step_expr.span.clone() });
                        }
                    }

                    symbols.enter_scope();
                    let name_str = self.interner.lookup(*var_name).trim().to_string();
                    symbols.define(name_str, inner, false);
                    self.loop_depth += 1;
                    for s in body {
                        self.check_stmt(s, symbols, errors);
                    }
                    self.loop_depth -= 1;
                    symbols.exit_scope();
                } else {
                    if start_ty != Type::Int && start_ty != Type::Unknown {
                        errors.push(TypeError { kind: TypeErrorKind::TypeMismatch { expected: Type::Int, actual: start_ty }, span: start.span.clone() });
                    }
                    let e_ty = self.check_expr(end, symbols, errors);
                    if e_ty != Type::Int && e_ty != Type::Unknown {
                        errors.push(TypeError { kind: TypeErrorKind::TypeMismatch { expected: Type::Int, actual: e_ty }, span: end.span.clone() });
                    }
                    if let Some(step_expr) = step {
                        let step_ty = self.check_expr(step_expr, symbols, errors);
                        if step_ty != Type::Int && step_ty != Type::Unknown {
                            errors.push(TypeError { kind: TypeErrorKind::TypeMismatch { expected: Type::Int, actual: step_ty }, span: step_expr.span.clone() });
                        }
                    }
                    symbols.enter_scope();
                    let name_str = self.interner.lookup(*var_name).trim().to_string();
                    symbols.define(name_str, Type::Int, false);
                    self.loop_depth += 1;
                    for s in body {
                        self.check_stmt(s, symbols, errors);
                    }
                    self.loop_depth -= 1;
                    symbols.exit_scope();
                }
            }
            crate::parser::ast::StmtKind::Break => {
                if self.loop_depth == 0 {
                    errors.push(TypeError { kind: TypeErrorKind::BreakOutsideLoop, span });
                }
            }
            crate::parser::ast::StmtKind::Continue => {
                if self.loop_depth == 0 {
                    errors.push(TypeError { kind: TypeErrorKind::ContinueOutsideLoop, span });
                }
            }
            crate::parser::ast::StmtKind::Assign { name, value } => {
                let name_str = self.interner.lookup(*name).trim().to_string();
                let var_ty = match symbols.lookup(&name_str) {
                    Some(ty) => ty,
                    None => {
                        errors.push(TypeError { kind: TypeErrorKind::UndefinedVariable(name_str.clone()), span: span.clone() });
                        Type::Unknown
                    }
                };
                if var_ty != Type::Unknown && symbols.is_const(&name_str) {
                    errors.push(TypeError { kind: TypeErrorKind::ConstReassignment(name_str.clone()), span: span.clone() });
                }
                let val_ty = self.check_expr_with_context(value, symbols, errors, if var_ty != Type::Unknown { Some(var_ty.clone()) } else { None });
                if var_ty != Type::Unknown && val_ty != Type::Unknown {
                    if !self.is_compatible(&var_ty, &val_ty) {
                        errors.push(TypeError {
                            kind: TypeErrorKind::TypeMismatch { expected: var_ty.clone(), actual: val_ty.clone() },
                            span: value.span.clone()
                        });
                    }
                    if let (Type::Table(e_cols), Type::Table(a_cols)) = (&var_ty, &val_ty) {
                        if e_cols.is_empty() && !a_cols.is_empty() {
                            symbols.define(self.interner.lookup(*name).trim().to_string(), val_ty.clone(), symbols.is_const(&name_str));
                        }
                    }
                }
            }
            crate::parser::ast::StmtKind::Include { .. } => {}
            crate::parser::ast::StmtKind::FunctionCallStmt { name, args } => {
                let name_str = self.interner.lookup(*name).trim().to_string();
                let resolved_sig = self.functions.get(&name_str).cloned();

                self.check_argument_list(args, symbols, errors, false);

                if let Some(sig) = resolved_sig {
                    if args.len() != sig.params.len() {
                        errors.push(TypeError {
                            kind: TypeErrorKind::InvalidArgumentCount { expected: sig.params.len(), actual: args.len() },
                            span: span.clone(),
                        });
                    }
                } else if !symbols.has(&name_str) {
                    errors.push(TypeError { kind: TypeErrorKind::UndefinedVariable(name_str.to_string()), span });
                }
            }
            crate::parser::ast::StmtKind::JsonBind { json, path, target } => {
                let j_ty = self.check_expr(json, symbols, errors);
                if j_ty != Type::Json && j_ty != Type::Unknown {
                    errors.push(TypeError { kind: TypeErrorKind::TypeMismatch { expected: Type::Json, actual: j_ty }, span: json.span.clone() });
                }
                let p_ty = self.check_expr(path, symbols, errors);
                if p_ty != Type::String && p_ty != Type::Unknown {
                    errors.push(TypeError { kind: TypeErrorKind::TypeMismatch { expected: Type::String, actual: p_ty }, span: path.span.clone() });
                }
                let name_str = self.interner.lookup(*target).trim().to_string();
                if !symbols.has(&name_str) {
                    errors.push(TypeError { kind: TypeErrorKind::UndefinedVariable(name_str), span: span.clone() });
                }
            }
            crate::parser::ast::StmtKind::JsonInject { json, mapping, table } => {
                let j_ty = self.check_expr(json, symbols, errors);
                if j_ty != Type::Json && j_ty != Type::Unknown {
                    errors.push(TypeError { kind: TypeErrorKind::TypeMismatch { expected: Type::Json, actual: j_ty }, span: json.span.clone() });
                }
                let m_ty = self.check_expr(mapping, symbols, errors);
                if m_ty != Type::Unknown && !matches!(m_ty, Type::Map(_, _)) {
                    errors.push(TypeError { kind: TypeErrorKind::TypeMismatch { expected: Type::Map(Box::new(Type::String), Box::new(Type::String)), actual: m_ty }, span: mapping.span.clone() });
                }
                let table_str = self.interner.lookup(*table).trim().to_string();
                if !symbols.has(&table_str) {
                    errors.push(TypeError { kind: TypeErrorKind::UndefinedVariable(table_str), span: span.clone() });
                }
            }
            crate::parser::ast::StmtKind::FiberDef { name, params, return_type, body } => {
                let name_str = self.interner.lookup(*name).trim().to_string();
                let var_type = Type::Fiber(return_type.as_ref().map(|t| Box::new(t.clone())));
                if !symbols.has(&name_str) {
                    symbols.define(name_str.clone(), var_type.clone(), false);
                }

                let prev_fiber_ctx = self.fiber_context.take();
                let prev_has_yield = self.fiber_has_yield;
                self.fiber_has_yield = false;
                self.fiber_context = Some(return_type.clone());

                let mut child = SymbolTable::new_with_parent(symbols);
                child.enter_scope();
                child.define(name_str, var_type, false);

                for (ty, pname) in params {
                    let pname_str = self.interner.lookup(*pname).trim().to_string();
                    child.define(pname_str, ty.clone(), false);
                }
                let prev_loop = self.loop_depth;
                self.loop_depth = 0;
                for s in body {
                    self.check_stmt(s, &mut child, errors);
                }
                self.loop_depth = prev_loop;
                self.fiber_context = prev_fiber_ctx;
                self.fiber_has_yield = prev_has_yield;
                let _ = name;
            }
            crate::parser::ast::StmtKind::FiberDecl { inner_type, name, fiber_name, args } => {
                let fiber_name_str = self.interner.lookup(*fiber_name).trim().to_string();

                self.check_argument_list(args, symbols, errors, false);

                let resolved_sig: Option<FunctionSignature> = if let Some(sig) = self.functions.get(&fiber_name_str).cloned() {
                    Some(sig)
                } else if let Some(ty) = symbols.lookup(&fiber_name_str) {
                    match ty {
                        Type::Fiber(inner_ret) => {
                            Some(FunctionSignature {
                                params: vec![Type::Unknown; args.len()],
                                return_type: inner_ret.map(|t| *t),
                                is_fiber: true,
                            })
                        }
                        _ => None,
                    }
                } else {
                    None
                };

                if let Some(sig) = resolved_sig {
                    if !sig.is_fiber {
                        errors.push(TypeError {
                            kind: TypeErrorKind::UndefinedVariable(format!("{} is a func, not a fiber", fiber_name_str)),
                            span: span.clone(),
                        });
                    }
                } else {
                    errors.push(TypeError {
                        kind: TypeErrorKind::UndefinedVariable(format!("fiber '{}' not defined", fiber_name_str)),
                        span: span.clone(),
                    });
                }

                let name_str = self.interner.lookup(*name).trim().to_string();
                if symbols.has_in_current_scope(&name_str) {
                    errors.push(TypeError { kind: TypeErrorKind::RedefinedVariable(name_str.clone()), span: span.clone() });
                }
                let var_type = Type::Fiber(inner_type.as_ref().map(|t| Box::new(t.clone())));
                symbols.define(name_str, var_type, false);
            }
            crate::parser::ast::StmtKind::Yield(expr) => {
                self.fiber_has_yield = true;
                let context = self.fiber_context.clone();
                match context {
                    None => {
                        errors.push(TypeError { kind: TypeErrorKind::YieldOutsideFiber, span: span.clone() });
                    }
                    Some(None) => {
                        let prev_yield = self.in_yield_expr;
                        self.in_yield_expr = true;
                        self.last_expr_was_db_io = false;
                        let _ = self.check_expr(expr, symbols, errors);
                        self.in_yield_expr = prev_yield;
                        if !self.last_expr_was_db_io {
                            errors.push(TypeError { kind: TypeErrorKind::FiberTypeMismatch, span: span.clone() });
                        }
                    }
                    Some(Some(expected_yield_ty)) => {
                        let prev_yield = self.in_yield_expr;
                        self.in_yield_expr = true;
                        self.last_expr_was_db_io = false;
                        let expr_ty = self.check_expr(expr, symbols, errors);
                        self.in_yield_expr = prev_yield;
                        // Rule D401: remove() must have .where()
                        if let Type::DatabaseOperation(crate::parser::ast::DatabaseOpKind::Remove, _) = expr_ty {
                            errors.push(TypeError { 
                                kind: TypeErrorKind::Other("Rule D401: remove() requires .where() filter before yielding".to_string()), 
                                span: expr.span.clone() 
                            });
                        }
                        if expr_ty != Type::Unknown && !self.is_compatible(&expected_yield_ty, &expr_ty) {
                            if !self.last_expr_was_db_io {
                                errors.push(TypeError {
                                    kind: TypeErrorKind::TypeMismatch { expected: expected_yield_ty.clone(), actual: expr_ty },
                                    span: expr.span.clone(),
                                });
                            }
                        }
                    }
                }
            }
            crate::parser::ast::StmtKind::YieldFrom(expr) => {
                self.fiber_has_yield = true;
                if self.fiber_context.is_none() {
                    errors.push(TypeError { kind: TypeErrorKind::YieldOutsideFiber, span: span.clone() });
                }
                let expr_ty = self.check_expr(expr, symbols, errors);
                match &expr_ty {
                    Type::Fiber(_) | Type::Unknown => {
                        // Valid: Fiber type or unresolved (let VM handle it).
                    }
                    _ => {
                        errors.push(TypeError {
                            kind: TypeErrorKind::Other("'yield from' expects a fiber expression".to_string()),
                            span: expr.span.clone(),
                        });
                    }
                }
            }
            crate::parser::ast::StmtKind::YieldVoid => {
                if self.fiber_context.is_none() {
                    errors.push(TypeError { kind: TypeErrorKind::YieldOutsideFiber, span: span.clone() });
                }
            }
            crate::parser::ast::StmtKind::NetRequestStmt { method, url, headers, body, timeout, target } => {
                self.check_expr(method, symbols, errors);
                self.check_expr(url, symbols, errors);
                if let Some(h) = headers { self.check_expr(h, symbols, errors); }
                if let Some(b) = body { self.check_expr(b, symbols, errors); }
                if let Some(t) = timeout { self.check_expr(t, symbols, errors); }
                let name_str = self.interner.lookup(*target).trim().to_string();
                symbols.define(name_str, Type::Json, false);
            }
            crate::parser::ast::StmtKind::Serve { name: _, port, host, workers, routes } => {
                self.check_expr(port, symbols, errors);
                if let Some(h) = host { self.check_expr(h, symbols, errors); }
                if let Some(w) = workers { self.check_expr(w, symbols, errors); }
                self.check_routes_expr(routes, symbols, errors);
            }
            crate::parser::ast::StmtKind::Wait(expr) => {
                self.check_expr(expr, symbols, errors);
            }
        }
    }

    fn check_routes_expr(&mut self, expr: &Expr, symbols: &mut SymbolTable<'_>, errors: &mut Vec<TypeError>) {
        match &expr.kind {
            ExprKind::ArrayLiteral { elements } => {
                for elem in elements {
                    self.check_routes_expr(elem, symbols, errors);
                }
            }
            ExprKind::Tuple(elements) => {
                for elem in elements {
                    self.check_routes_expr(elem, symbols, errors);
                }
            }
            ExprKind::Binary { right, .. } => {
                self.check_routes_expr(right, symbols, errors);
            }
            ExprKind::Identifier(id) => {
                let name = self.interner.lookup(*id).trim();
                if self.functions.get(name).is_none() && !symbols.has(name) {
                    match name {
                        "*" | "_" => {}
                        _ => {
                            errors.push(TypeError {
                                kind: TypeErrorKind::UndefinedVariable(name.to_string()),
                                span: expr.span.clone(),
                            });
                        }
                    }
                }
            }
            ExprKind::StringLiteral(_) => {}
            _ => {}
        }
    }

    fn check_expr(&mut self, expr: &Expr, symbols: &mut SymbolTable<'_>, errors: &mut Vec<TypeError>) -> Type {
        self.check_expr_with_context(expr, symbols, errors, None)
    }

    fn check_expr_with_context(&mut self, expr: &Expr, symbols: &mut SymbolTable<'_>, errors: &mut Vec<TypeError>, context: Option<Type>) -> Type {
        let span = expr.span.clone();
        match &expr.kind {
            &ExprKind::TerminalCommand(_, _) => Type::Unknown,
            &ExprKind::Tag(_) => Type::String,
            ExprKind::IntLiteral(_) => Type::Int,
            ExprKind::FloatLiteral(_) => Type::Float,
            ExprKind::StringLiteral(_) => Type::String,
            ExprKind::BoolLiteral(_) => Type::Bool,
            ExprKind::Identifier(id) => {
                let name_raw = self.interner.lookup(*id);
                let name = name_raw.trim();
                match name {
                    "json" | "date" | "store" | "halt" | "terminal" | "net" | "env" | "crypto" => return Type::Builtin(*id),
                    _ => {}
                }
                let name_raw = self.interner.lookup(*id);
                let name_trimmed = name_raw.trim().to_string();
                
                if let Some(ty) = symbols.lookup(&name_trimmed) { return ty; }
                
                if let Some(sig) = self.functions.get(&name_trimmed) {
                    if sig.is_fiber {
                        return Type::Fiber(sig.return_type.clone().map(Box::new));
                    }
                    return sig.return_type.clone().unwrap_or(Type::Unknown);
                }
                else if self.is_table_lambda {
                    if let Some(row_ty) = symbols.lookup("__row_tmp") {
                        if let Type::Table(cols) = row_ty {
                            for col in cols {
                                if self.interner.lookup(col.name) == name {
                                    return col.ty.clone();
                                }
                            }
                        }
                    }
                    errors.push(TypeError { kind: TypeErrorKind::UndefinedVariable(name.to_string()), span });
                    Type::Unknown
                } else {
                    errors.push(TypeError { kind: TypeErrorKind::UndefinedVariable(name.to_string()), span });
                    Type::Unknown
                }
            }
            crate::parser::ast::ExprKind::RawBlock(_) => Type::Json,
            crate::parser::ast::ExprKind::ArrayLiteral { elements } => {
                let mut expected_inner = match context {
                    Some(Type::Array(inner)) => Some(*inner.clone()),
                    _ => None,
                };

                if elements.is_empty() {
                    return Type::Array(Box::new(expected_inner.unwrap_or(Type::Int)));
                }

                if expected_inner.is_none() {
                    // Infer from first element
                    let first_ty = self.check_expr(&elements[0], symbols, errors);
                    expected_inner = Some(first_ty);
                } else {
                    // Check first element against context
                    let first_ty = self.check_expr_with_context(&elements[0], symbols, errors, expected_inner.clone());
                    if first_ty != Type::Unknown && !self.is_compatible(expected_inner.as_ref().unwrap(), &first_ty) {
                         errors.push(TypeError { 
                             kind: TypeErrorKind::TypeMismatch { expected: expected_inner.clone().unwrap(), actual: first_ty }, 
                             span: elements[0].span.clone() 
                         });
                    }
                }

                let target = expected_inner.unwrap();
                for elem in elements.iter().skip(1) {
                    let ty = self.check_expr_with_context(elem, symbols, errors, Some(target.clone()));
                    if target != Type::Unknown && ty != Type::Unknown && !self.is_compatible(&target, &ty) {
                        errors.push(TypeError { 
                            kind: TypeErrorKind::TypeMismatch { expected: target.clone(), actual: ty }, 
                            span: elem.span.clone() 
                        });
                    }
                }
                Type::Array(Box::new(target))
            }
            ExprKind::Binary { left, op, right } => {
                let l_ty = self.check_expr(left, symbols, errors);
                let r_ty = self.check_expr(right, symbols, errors);
                if l_ty == Type::Unknown || r_ty == Type::Unknown { return Type::Unknown; }
                match op {
                    TokenKind::Plus => {
                        if l_ty == Type::Date && r_ty == Type::Int { return Type::Date; }
                        if l_ty == Type::String || r_ty == Type::String {
                            Type::String
                        } else if (l_ty == Type::Int || l_ty == Type::Float) && (r_ty == Type::Int || r_ty == Type::Float) {
                            if l_ty == Type::Float || r_ty == Type::Float { Type::Float } else { Type::Int }
                        } else {
                            errors.push(TypeError { kind: TypeErrorKind::InvalidBinaryOp { op: op.clone(), left: l_ty, right: r_ty }, span: span.clone() });
                            Type::Unknown
                        }
                    }
                    TokenKind::PlusPlus => {
                        if (l_ty == Type::Int || l_ty == Type::Unknown) && (r_ty == Type::Int || r_ty == Type::Unknown) {
                            Type::Int
                        } else {
                            Type::String
                        }
                    }
                    TokenKind::Minus | TokenKind::Star | TokenKind::Slash | TokenKind::Percent | TokenKind::Caret => {
                        if op == &TokenKind::Minus {
                            match (&l_ty, &r_ty) {
                                (Type::Date, Type::Int) => return Type::Date,
                                (Type::Date, Type::Date) => return Type::Int,
                                (Type::Int, Type::Date) => return Type::Int,
                                (Type::Set(_), Type::Set(_)) if l_ty == r_ty => return l_ty.clone(),
                                _ => {}
                            }
                        }
                        if (l_ty == Type::Int || l_ty == Type::Float) && (r_ty == Type::Int || r_ty == Type::Float) {
                            if l_ty == Type::Float || r_ty == Type::Float { Type::Float } else { Type::Int }
                        } else if matches!(l_ty, Type::Set(_)) && l_ty == r_ty && op == &TokenKind::Minus {
                            l_ty.clone()
                        } else {
                            errors.push(TypeError { kind: TypeErrorKind::InvalidBinaryOp { op: op.clone(), left: l_ty, right: r_ty }, span: span.clone() });
                            Type::Unknown
                        }
                    }
                    TokenKind::EqualEqual | TokenKind::BangEqual | TokenKind::Greater | TokenKind::Less | TokenKind::GreaterEqual | TokenKind::LessEqual => {
                        if l_ty == Type::Json || r_ty == Type::Json { return Type::Bool; }
                        if !self.is_compatible(&l_ty, &r_ty) && !self.is_compatible(&r_ty, &l_ty) {
                            errors.push(TypeError { kind: TypeErrorKind::TypeMismatch { expected: l_ty, actual: r_ty }, span: span.clone() });
                        }
                        Type::Bool
                    }
                    TokenKind::And | TokenKind::Or => {
                        if l_ty != Type::Bool { errors.push(TypeError { kind: TypeErrorKind::TypeMismatch { expected: Type::Bool, actual: l_ty }, span: left.span.clone() }); }
                        if r_ty != Type::Bool { errors.push(TypeError { kind: TypeErrorKind::TypeMismatch { expected: Type::Bool, actual: r_ty }, span: right.span.clone() }); }
                        Type::Bool
                    }
                    TokenKind::Has => {
                        if l_ty == Type::String && r_ty == Type::String { return Type::Bool; }
                        let inner_ty = match &r_ty {
                            Type::Array(inner) => Some((**inner).clone()),
                            Type::Set(st) => Some(match st {
                                crate::parser::ast::SetType::N | crate::parser::ast::SetType::Z => Type::Int,
                                crate::parser::ast::SetType::Q => Type::Float,
                                crate::parser::ast::SetType::S | crate::parser::ast::SetType::C => Type::String,
                                crate::parser::ast::SetType::B => Type::Bool,
                            }),
                            _ => None,
                        };
                        if let Some(expected) = inner_ty {
                            if !self.is_compatible(&expected, &l_ty) {
                                errors.push(TypeError { kind: TypeErrorKind::TypeMismatch { expected, actual: l_ty }, span: left.span.clone() });
                            }
                        } else {
                            errors.push(TypeError { kind: TypeErrorKind::InvalidBinaryOp { op: op.clone(), left: l_ty, right: r_ty.clone() }, span: span.clone() });
                        }
                        Type::Bool
                    }
                    TokenKind::Union | TokenKind::Intersection | TokenKind::Difference | TokenKind::SymDifference => {
                        if matches!(l_ty, Type::Set(_)) && l_ty == r_ty {
                            l_ty.clone()
                        } else {
                            errors.push(TypeError { kind: TypeErrorKind::InvalidBinaryOp { op: op.clone(), left: l_ty, right: r_ty }, span: span.clone() });
                            Type::Unknown
                        }
                    }
                    TokenKind::DoubleColon => {
                        Type::Map(Box::new(l_ty), Box::new(r_ty))
                    }
                    TokenKind::Equal => {
                        if !self.is_compatible(&l_ty, &r_ty) {
                            errors.push(TypeError { kind: TypeErrorKind::TypeMismatch { expected: l_ty.clone(), actual: r_ty.clone() }, span: span.clone() });
                        }
                        l_ty.clone()
                    }
                    _ => {
                        errors.push(TypeError { kind: TypeErrorKind::InvalidBinaryOp { op: op.clone(), left: l_ty, right: r_ty }, span: span.clone() });
                        Type::Unknown
                    }
                }
            }
            ExprKind::Unary { op, right } => {
                let r_ty = self.check_expr(right, symbols, errors);
                if r_ty == Type::Unknown { return Type::Unknown; }
                match op {
                    TokenKind::Minus => {
                        if r_ty != Type::Int && r_ty != Type::Float {
                            errors.push(TypeError { kind: TypeErrorKind::TypeMismatch { expected: Type::Int, actual: r_ty }, span: right.span.clone() });
                            Type::Unknown
                        } else { r_ty }
                    }
                    TokenKind::Not | TokenKind::Bang => {
                        if r_ty != Type::Bool {
                            errors.push(TypeError { kind: TypeErrorKind::TypeMismatch { expected: Type::Bool, actual: r_ty }, span: right.span.clone() });
                        }
                        Type::Bool
                    }
                    _ => {
                        errors.push(TypeError { kind: TypeErrorKind::TypeMismatch { expected: Type::Int, actual: r_ty }, span: span.clone() });
                        Type::Unknown
                    }
                }
            }
            crate::parser::ast::ExprKind::FunctionCall { name, args } => {
                let name_str = self.interner.lookup(*name).trim().to_string();
                let mut resolved_sig = self.functions.get(&name_str).cloned();

                if resolved_sig.is_none() {
                    if let Some(ty) = symbols.lookup(&name_str) {
                        match ty {
                            Type::Fiber(inner) => {
                                resolved_sig = Some(FunctionSignature {
                                    params: vec![Type::Unknown; args.len()],
                                    return_type: inner.map(|t| *t),
                                    is_fiber: true,
                                });
                            }
                            _ => {
                                resolved_sig = Some(FunctionSignature {
                                    params: vec![Type::Unknown; args.len()],
                                    return_type: Some(Type::Unknown),
                                    is_fiber: false,
                                });
                            }
                        }
                    }
                }

                if let Some(sig) = resolved_sig {
                    let params = sig.params.clone();
                    let ret = sig.return_type.clone().unwrap_or(Type::Unknown);
                    if args.len() != sig.params.len() {
                        errors.push(TypeError {
                            kind: TypeErrorKind::InvalidArgumentCount { expected: sig.params.len(), actual: args.len() },
                            span: span.clone(),
                        });
                    }
                    for (arg, expected) in args.iter().zip(params) {
                        let arg_ty = self.check_expr(arg.expr(), symbols, errors);
                        if arg_ty != Type::Unknown && !self.is_compatible(&expected, &arg_ty) {
                            errors.push(TypeError { kind: TypeErrorKind::TypeMismatch { expected, actual: arg_ty }, span: arg.expr().span.clone() });
                        }
                    }
                    // Check extra args if any (though already reported if mismatch)
                    for arg in args.iter().skip(sig.params.len()) {
                        self.check_expr(arg.expr(), symbols, errors);
                    }
                    // A fiber constructor call returns a Fiber instance, not the yield type.
                    if sig.is_fiber {
                        Type::Fiber(Some(Box::new(ret)))
                    } else {
                        ret
                    }
                } else {
                    for arg in args {
                        self.check_expr(arg.expr(), symbols, errors);
                    }
                    errors.push(TypeError { kind: TypeErrorKind::UndefinedVariable(name_str.to_string()), span: span.clone() });
                    Type::Unknown
                }
            }
            crate::parser::ast::ExprKind::SetLiteral { set_type, elements, range } => {
                let expected = match set_type {
                    crate::parser::ast::SetType::N | crate::parser::ast::SetType::Z => Type::Int,
                    crate::parser::ast::SetType::Q => Type::Float,
                    crate::parser::ast::SetType::S => Type::String,
                    crate::parser::ast::SetType::C => Type::String,
                    crate::parser::ast::SetType::B => Type::Bool,
                };
                for elem in elements {
                    let ty = self.check_expr(elem, symbols, errors);
                    if ty != Type::Unknown && ty != expected {
                        errors.push(TypeError { kind: TypeErrorKind::TypeMismatch { expected: expected.clone(), actual: ty }, span: elem.span.clone() });
                    }
                }
                if let Some(r) = range {
                    let s_ty = self.check_expr(&r.start, symbols, errors);
                    let e_ty = self.check_expr(&r.end, symbols, errors);
                    if s_ty != Type::Unknown && s_ty != expected {
                        errors.push(TypeError { kind: TypeErrorKind::TypeMismatch { expected: expected.clone(), actual: s_ty }, span: r.start.span.clone() });
                    }
                    if e_ty != Type::Unknown && e_ty != expected {
                        errors.push(TypeError { kind: TypeErrorKind::TypeMismatch { expected: expected.clone(), actual: e_ty }, span: r.end.span.clone() });
                    }
                    if let Some(step_expr) = &r.step {
                        let step_ty = self.check_expr(step_expr, symbols, errors);
                        if step_ty != Type::Unknown {
                            let step_ok = if set_type == &crate::parser::ast::SetType::C {
                                step_ty == Type::Int
                            } else if set_type == &crate::parser::ast::SetType::Q {
                                step_ty == Type::Float || step_ty == Type::Int
                            } else {
                                step_ty == expected
                            };
                            if !step_ok {
                                errors.push(TypeError {
                                    kind: TypeErrorKind::TypeMismatch { expected: if set_type == &crate::parser::ast::SetType::C { Type::Int } else { expected.clone() }, actual: step_ty },
                                    span: step_expr.span.clone()
                                });
                            }
                        }
                    }
                }
                Type::Set(set_type.clone())
            }
            crate::parser::ast::ExprKind::ArrayOrSetLiteral { elements } => {
                match context {
                    Some(Type::Array(inner)) => {
                        for e in elements {
                            self.check_expr_with_context(e, symbols, errors, Some(*inner.clone()));
                        }
                        Type::Array(inner)
                    }
                    Some(Type::Set(st)) => {
                        let inner = match &st {
                            SetType::N | SetType::Z => Type::Int,
                            SetType::Q => Type::Float,
                            SetType::S | SetType::C => Type::String,
                            SetType::B => Type::Bool,
                        };
                        for e in elements {
                            self.check_expr_with_context(e, symbols, errors, Some(inner.clone()));
                        }
                        Type::Set(st)
                    }
                    _ => {
                        if elements.is_empty() {
                            return Type::Array(Box::new(Type::Int));
                        }
                        let first_ty = self.check_expr_with_context(&elements[0], symbols, errors, context.clone());
                        for e in elements.iter().skip(1) {
                            let ty = self.check_expr_with_context(e, symbols, errors, context.clone());
                            if first_ty != Type::Unknown && ty != Type::Unknown && !self.is_compatible(&first_ty, &ty) {
                                let is_db_param = match &context {
                                    Some(Type::Array(_)) => false,
                                    _ => self.last_expr_was_db_io
                                };
                                
                                if !is_db_param {
                                    errors.push(TypeError { 
                                        kind: TypeErrorKind::TypeMismatch { expected: first_ty.clone(), actual: ty }, 
                                        span: e.span.clone() 
                                    });
                                }
                            }
                        }
                        Type::Array(Box::new(first_ty))
                    }
                }
            }
            crate::parser::ast::ExprKind::RandomChoice { set } => {
                let s_ty = self.check_expr(set, symbols, errors);
                match s_ty {
                    Type::Set(st) => match st {
                        crate::parser::ast::SetType::N | crate::parser::ast::SetType::Z => Type::Int,
                        crate::parser::ast::SetType::Q => Type::Float,
                        crate::parser::ast::SetType::S | crate::parser::ast::SetType::C => Type::String,
                        crate::parser::ast::SetType::B => Type::Bool,
                    },
                    Type::Array(inner) => *inner.clone(),
                    Type::Unknown => Type::Unknown,
                    _ => {
                        errors.push(TypeError { kind: TypeErrorKind::TypeMismatch { expected: Type::Set(crate::parser::ast::SetType::N), actual: s_ty }, span: set.span.clone() });
                        Type::Unknown
                    }
                }
            }
            crate::parser::ast::ExprKind::RandomInt { min, max, step } => {
                let min_ty = self.check_expr(min, symbols, errors);
                let max_ty = self.check_expr(max, symbols, errors);
                if min_ty != Type::Int && min_ty != Type::Unknown {
                    errors.push(TypeError { kind: TypeErrorKind::TypeMismatch { expected: Type::Int, actual: min_ty }, span: min.span.clone() });
                }
                if max_ty != Type::Int && max_ty != Type::Unknown {
                    errors.push(TypeError { kind: TypeErrorKind::TypeMismatch { expected: Type::Int, actual: max_ty }, span: max.span.clone() });
                }
                if let Some(s) = step {
                    let s_ty = self.check_expr(s, symbols, errors);
                    if s_ty != Type::Int && s_ty != Type::Unknown {
                        errors.push(TypeError { kind: TypeErrorKind::TypeMismatch { expected: Type::Int, actual: s_ty }, span: s.span.clone() });
                    }
                }
                Type::Int
            }
            crate::parser::ast::ExprKind::RandomFloat { min, max, step } => {
                let min_ty = self.check_expr(min, symbols, errors);
                let max_ty = self.check_expr(max, symbols, errors);
                if min_ty != Type::Float && min_ty != Type::Unknown {
                    errors.push(TypeError { kind: TypeErrorKind::TypeMismatch { expected: Type::Float, actual: min_ty }, span: min.span.clone() });
                }
                if max_ty != Type::Float && max_ty != Type::Unknown {
                    errors.push(TypeError { kind: TypeErrorKind::TypeMismatch { expected: Type::Float, actual: max_ty }, span: max.span.clone() });
                }
                if let Some(s) = step {
                    let s_ty = self.check_expr(s, symbols, errors);
                    if s_ty != Type::Float && s_ty != Type::Unknown && s_ty != Type::Int {
                        errors.push(TypeError { kind: TypeErrorKind::TypeMismatch { expected: Type::Float, actual: s_ty }, span: s.span.clone() });
                    }
                }
                Type::Float
            }
            crate::parser::ast::ExprKind::MapLiteral { key_type, value_type, elements } => {
                for (k, v) in elements {
                    let k_ty = self.check_expr(k, symbols, errors);
                    if k_ty != Type::Unknown && k_ty != *key_type {
                        errors.push(TypeError { kind: TypeErrorKind::TypeMismatch { expected: key_type.clone(), actual: k_ty }, span: k.span.clone() });
                    }
                    let v_ty = self.check_expr(v, symbols, errors);
                    if v_ty != Type::Unknown && !self.is_compatible(value_type, &v_ty) {
                        errors.push(TypeError { kind: TypeErrorKind::TypeMismatch { expected: value_type.clone(), actual: v_ty }, span: v.span.clone() });
                    }
                }
                Type::Map(Box::new(key_type.clone()), Box::new(value_type.clone()))
            }
            crate::parser::ast::ExprKind::DateLiteral { .. } => Type::Date,
            crate::parser::ast::ExprKind::TableLiteral { columns, rows } => {
                let non_auto_cols: Vec<_> = columns.iter().filter(|c| !c.is_auto()).collect();
                let non_auto_count = non_auto_cols.len();
                
                for row in rows {
                    if row.len() != non_auto_count {
                        errors.push(TypeError { 
                            kind: TypeErrorKind::TableRowCountMismatch { expected: non_auto_count, actual: row.len() },
                            span: span.clone() 
                        });
                    }
                    for (i, val) in row.iter().enumerate() {
                        let ty = self.check_expr(val, symbols, errors);
                        if i < non_auto_count {
                            let expected = &non_auto_cols[i].ty;
                            if ty != Type::Unknown && !self.is_compatible(expected, &ty) {
                                errors.push(TypeError {
                                    kind: TypeErrorKind::TypeMismatch { expected: expected.clone(), actual: ty },
                                    span: val.span.clone()
                                });
                            }
                        }
                    }
                }
                Type::Table(columns.clone())
            }
            crate::parser::ast::ExprKind::DatabaseLiteral(fields) => {
                for (_, v) in fields {
                    self.check_expr(v, symbols, errors);
                }
                Type::Database
            }
            crate::parser::ast::ExprKind::Yield(expr) => {
                if self.fiber_context.is_none() {
                    errors.push(TypeError { kind: TypeErrorKind::YieldOutsideFiber, span: span.clone() });
                }
                self.fiber_has_yield = true;
                let prev_yield = self.in_yield_expr;
                self.in_yield_expr = true;
                let ty = self.check_expr(expr, symbols, errors);
                self.in_yield_expr = prev_yield;
                ty
            }
            crate::parser::ast::ExprKind::As { expr, name } => {
                let ty = self.check_expr(expr, symbols, errors);
                let name_str = self.interner.lookup(*name).trim().to_string();
                symbols.define(name_str, ty.clone(), false);
                ty
            }
            crate::parser::ast::ExprKind::Index { receiver, index } => {
                let rec_ty = self.check_expr(receiver, symbols, errors);
                let idx_ty = self.check_expr(index, symbols, errors);
                match rec_ty {
                    Type::Array(inner) => {
                        if idx_ty != Type::Int && idx_ty != Type::Unknown {
                            errors.push(TypeError { kind: TypeErrorKind::TypeMismatch { expected: Type::Int, actual: idx_ty }, span: index.span.clone() });
                        }
                        *inner
                    }
                    Type::Table(columns) => {
                        if idx_ty != Type::Int && idx_ty != Type::Unknown {
                            errors.push(TypeError { kind: TypeErrorKind::TypeMismatch { expected: Type::Int, actual: idx_ty }, span: index.span.clone() });
                        }
                        Type::Table(columns)
                    }
                    Type::Builtin(id) if self.interner.lookup(id) == "net" => Type::Json,
                    Type::Json => Type::Json,
                    Type::Unknown => Type::Unknown,
                    _ => {
                        errors.push(TypeError { kind: TypeErrorKind::IndexAccessNotSupported(rec_ty), span });
                        Type::Unknown
                    }
                }
            }
            crate::parser::ast::ExprKind::NetCall { method: _, url, body } => {
                let u_ty = self.check_expr(url, symbols, errors);
                if u_ty != Type::String && u_ty != Type::Unknown {
                    errors.push(TypeError { kind: TypeErrorKind::TypeMismatch { expected: Type::String, actual: u_ty }, span: url.span.clone() });
                }
                if let Some(b) = body {
                    let b_ty = self.check_expr(b, symbols, errors);
                    if b_ty != Type::Json && b_ty != Type::String && b_ty != Type::Unknown {
                        errors.push(TypeError { kind: TypeErrorKind::TypeMismatch { expected: Type::Json, actual: b_ty }, span: b.span.clone() });
                    }
                }
                Type::Json
            }
            crate::parser::ast::ExprKind::NetRespond { status, body, headers } => {
                let s_ty = self.check_expr(status, symbols, errors);
                if s_ty != Type::Int && s_ty != Type::Unknown {
                    errors.push(TypeError { kind: TypeErrorKind::TypeMismatch { expected: Type::Int, actual: s_ty }, span: status.span.clone() });
                }
                let b_ty = self.check_expr(body, symbols, errors);
                if b_ty != Type::Json && b_ty != Type::String && b_ty != Type::Unknown {
                    errors.push(TypeError {
                        kind: TypeErrorKind::Other(format!("net.respond body must be String or Json, got {:?}", b_ty)),
                        span: body.span.clone()
                    });
                }
                if let Some(h) = headers {
                    let h_ty = self.check_expr(h, symbols, errors);
                    let expected_map = Type::Map(Box::new(Type::String), Box::new(Type::String));
                    let expected_map_array = Type::Array(Box::new(expected_map.clone()));
                    let expected_str_array = Type::Array(Box::new(Type::String));
                    
                    let ok = self.is_compatible(&expected_map, &h_ty) ||
                             self.is_compatible(&expected_map_array, &h_ty) ||
                             self.is_compatible(&expected_str_array, &h_ty);
                    
                    if h_ty != Type::Unknown && !ok {
                        errors.push(TypeError { kind: TypeErrorKind::TypeMismatch { expected: expected_map, actual: h_ty }, span: h.span.clone() });
                    }
                }
                Type::Json
            }
            crate::parser::ast::ExprKind::Lambda { .. } => Type::Unknown,
            crate::parser::ast::ExprKind::Tuple(exprs) => {
                for e in exprs { self.check_expr(e, symbols, errors); }
                Type::Array(Box::new(Type::Unknown))
            }
            crate::parser::ast::ExprKind::MemberAccess { receiver, member } => {
                let rec_ty = self.check_expr(receiver, symbols, errors);
                let member_str = self.interner.lookup(*member).trim();
                match rec_ty {
                    Type::Table(ref cols) => {
                        if let Some(col) = cols.iter().find(|c| self.interner.lookup(c.name) == member_str) {
                            col.ty.clone()
                        } else {
                            match member_str {
                                "count" | "size" | "length" => Type::Int,
                                _ => {
                                    errors.push(TypeError { 
                                        kind: TypeErrorKind::PropertyNotFound { base_type: Type::Table(cols.clone()), property: member_str.to_string() }, 
                                        span: span.clone() 
                                    });
                                    Type::Unknown
                                }
                            }
                        }
                    }
                    Type::Date => {
                        match member_str {
                            "year" | "month" | "day" | "hour" | "minute" | "second" => Type::Int,
                            _ => {
                                errors.push(TypeError { 
                                    kind: TypeErrorKind::PropertyNotFound { base_type: Type::Date, property: member_str.to_string() }, 
                                    span: span.clone() 
                                });
                                Type::Unknown
                            }
                        }
                    }
                    Type::Builtin(bid) => {
                        let bname = self.interner.lookup(bid).trim();
                        if bname == "date" && member_str == "now" {
                            Type::Date
                        } else if bname == "net" {
                            match member_str {
                                "request" | "query" | "headers" | "body" | "form" => Type::Json,
                                "method" | "url" | "path" => Type::String,
                                "ip" | "remote_addr" => Type::String,
                                _ => Type::Json,
                            }
                        } else {
                            errors.push(TypeError { kind: TypeErrorKind::UndefinedVariable(format!("property {} for builtin service {}", member_str, bname)), span: span.clone() });
                            Type::Unknown
                        }
                    }
                    Type::Array(_) | Type::Map(_, _) | Type::Set(_) | Type::String | Type::Json => {
                        if member_str == "length" || member_str == "size" || member_str == "count" {
                            Type::Int
                        } else if matches!(rec_ty, Type::Json) {
                            match member_str {
                                "status" | "code" => Type::Int,
                                "ok" => Type::Bool,
                                "body" | "json" | "headers" => Type::Json,
                                "method" | "path" | "query" | "url" | "text" => Type::String,
                                _ => Type::Json,
                            }
                        } else {
                            errors.push(TypeError { 
                                kind: TypeErrorKind::PropertyNotFound { base_type: rec_ty, property: member_str.to_string() }, 
                                span: span.clone() 
                            });
                            Type::Unknown
                        }
                    }
                    _ => {
                        if let ExprKind::Identifier(rec_id) = &receiver.kind {
                            let rec_str = self.interner.lookup(*rec_id).trim();
                            let namespaced_name = format!("{}.{}", rec_str, member_str);
                            if let Some(ty) = symbols.lookup(&namespaced_name) {
                                return ty.clone();
                            }
                        }
                        if rec_ty == Type::Unknown { return Type::Unknown; }
                        errors.push(TypeError {
                            kind: TypeErrorKind::PropertyNotFound { base_type: rec_ty, property: member_str.to_string() },
                            span: span.clone()
                        });
                        Type::Unknown
                    }
                }
            }
            crate::parser::ast::ExprKind::MethodCall { receiver, method, args, wait_after: _ } => {
                let rec_ty = self.check_expr(receiver, symbols, errors);
                let method_str = self.interner.lookup(*method).trim();
                match rec_ty {
                    Type::Int | Type::Float => {
                        match method_str {
                            "toStr" | "toString" | "format" => {
                                if !args.is_empty() {
                                    errors.push(TypeError { kind: TypeErrorKind::InvalidArgumentCount { expected: 0, actual: args.len() }, span: span.clone() });
                                }
                                Type::String
                            }
                            _ => {
                                errors.push(TypeError { 
                                    kind: TypeErrorKind::MethodNotFound { base_type: rec_ty, method: method_str.to_string() }, 
                                    span: span.clone() 
                                });
                                Type::Unknown
                            }
                        }
                    }
                    Type::Table(ref cols) => {
                        let is_add_or_insert = method_str == "add" || method_str == "insert" || method_str == "save";
                        
                        if is_add_or_insert {
                            self.check_table_operation_args(cols, args, symbols, errors, &span, method_str);
                        }
                        
                        if method_str == "save" {
                             if !cols.iter().any(|c| c.attributes.contains(&ColumnAttribute::PrimaryKey)) {
                                 errors.push(TypeError { kind: TypeErrorKind::Other("Method 'save()' requires a @pk column in the table schema".to_string()), span: span.clone() });
                             }
                        }

                        match method_str {
                            "where" | "on" => {
                                if let Some(arg_node) = args.first() {
                                    let arg = arg_node.expr();
                                    let col_names: Vec<String> = cols.iter()
                                        .map(|c| self.interner.lookup(c.name).trim().to_string())
                                        .collect();
                                    let prev_lambda = self.is_table_lambda;
                                    self.is_table_lambda = true;
                                    
                                    symbols.enter_scope();
                                    symbols.define("__row_tmp".to_string(), Type::Table(cols.clone()), false);
                                    
                                    let mut pred_idents = Vec::new();
                                    self.collect_pred_idents(arg, &mut pred_idents);
                                    for ident_name in &pred_idents {
                                        if symbols.has(ident_name) && ident_name != "__row_tmp" && col_names.contains(ident_name) {
                                            errors.push(TypeError {
                                                kind: TypeErrorKind::WherePredicateNameCollision {
                                                    var_name: ident_name.clone(),
                                                    column_name: ident_name.clone(),
                                                },
                                                span: arg.span.clone(),
                                            });
                                        }
                                    }

                                    let pred_ty = self.check_expr(arg, symbols, errors);
                                    if pred_ty != Type::Bool && pred_ty != Type::Unknown {
                                        errors.push(TypeError {
                                            kind: TypeErrorKind::TypeMismatch { expected: Type::Bool, actual: pred_ty },
                                            span: arg.span.clone(),
                                        });
                                    }
                                    
                                    symbols.exit_scope();
                                    self.is_table_lambda = prev_lambda;
                                }
                                Type::Table(cols.clone())
                            }
                            "join" | "show" | "delete" | "get" | "at" => {
                                if method_str == "get" || method_str == "delete" || method_str == "at" {
                                    if let Some(arg_node) = args.first() {
                                        let arg = arg_node.expr();
                                        let arg_ty = self.check_expr(arg, symbols, errors);
                                        if arg_ty != Type::Int && arg_ty != Type::Unknown {
                                            errors.push(TypeError {
                                                kind: TypeErrorKind::TypeMismatch { expected: Type::Int, actual: arg_ty },
                                                span: arg.span.clone(),
                                            });
                                        }
                                    } else {
                                        errors.push(TypeError {
                                            kind: TypeErrorKind::InvalidArgumentCount { expected: 1, actual: 0 },
                                            span: span.clone(),
                                        });
                                    }
                                } else {
                                    for arg in args {
                                        self.check_expr(arg.expr(), symbols, errors);
                                    }
                                }
                                if method_str == "get" || method_str == "at" {
                                    Type::Table(cols.clone())
                                } else if method_str == "join" {
                                    if let Some(other_arg) = args.get(0) {
                                        let other_ty = self.check_expr(other_arg.expr(), symbols, errors);
                                        if args.len() == 3 {
                                            let key1_ty = self.check_expr(args[1].expr(), symbols, errors);
                                            let key2_ty = self.check_expr(args[2].expr(), symbols, errors);
                                            if key1_ty != Type::String && key1_ty != Type::Unknown {
                                                errors.push(TypeError { kind: TypeErrorKind::TypeMismatch { expected: Type::String, actual: key1_ty }, span: args[1].expr().span.clone() });
                                            }
                                            if key2_ty != Type::String && key2_ty != Type::Unknown {
                                                errors.push(TypeError { kind: TypeErrorKind::TypeMismatch { expected: Type::String, actual: key2_ty }, span: args[2].expr().span.clone() });
                                            }
                                        } else if args.len() == 2 {
                                            self.check_expr(args[1].expr(), symbols, errors);
                                        }

                                        match other_ty {
                                            Type::Table(other_cols) => {
                                                let mut combined = cols.clone();
                                                for oc in other_cols {
                                                    if let Some(existing) = combined.iter_mut().find(|c| c.name == oc.name) {
                                                        *existing = oc;
                                                    } else {
                                                        combined.push(oc);
                                                    }
                                                }
                                                Type::Table(combined)
                                            }
                                            Type::Unknown => Type::Table(cols.clone()),
                                            _ => {
                                                errors.push(TypeError {
                                                    kind: TypeErrorKind::TypeMismatch { expected: Type::Table(vec![]), actual: other_ty },
                                                    span: other_arg.expr().span.clone(),
                                                });
                                                Type::Table(cols.clone())
                                            }
                                        }
                                    } else {
                                        errors.push(TypeError {
                                            kind: TypeErrorKind::InvalidArgumentCount { expected: 1, actual: 0 },
                                            span: span.clone(),
                                        });
                                        Type::Table(cols.clone())
                                    }
                                } else {
                                    Type::Bool
                                }
                            }
                            "add" | "insert" | "update" => {
                                let start_idx = if method_str == "update" { 1 } else { 0 };

                                if method_str == "update" {
                                    if let Some(idx_arg) = args.first() {
                                        let idx_ty = self.check_expr(idx_arg.expr(), symbols, errors);
                                        if idx_ty != Type::Int && idx_ty != Type::Unknown {
                                            errors.push(TypeError {
                                                kind: TypeErrorKind::TypeMismatch { expected: Type::Int, actual: idx_ty },
                                                span: idx_arg.expr().span.clone(),
                                            });
                                        }
                                    }
                                }

                                if !cols.is_empty() {
                                    self.check_table_operation_args(cols, &args[start_idx..], symbols, errors, &span, method_str);
                                } else {
                                    for arg in args.iter().skip(start_idx) {
                                        self.check_expr(arg.expr(), symbols, errors);
                                    }
                                }
                                Type::Bool
                            }
                            "count" => Type::Int,
                            "clear" => Type::Bool,
                            "toJson" => Type::Json,
                            "first" => Type::Json,
                            _ => {
                                errors.push(TypeError {
                                    kind: TypeErrorKind::MethodNotFound { base_type: Type::Table(cols.clone()), method: method_str.to_string() },
                                    span: span.clone()
                                });
                                Type::Unknown
                            }
                        }
                    }
                    Type::Builtin(bid) => {
                        let bname = self.interner.lookup(bid).trim();
                        match bname {
                            "json" => match method_str {
                                "parse" | "stringify" => Type::Json,
                                _ => {
                                    errors.push(TypeError { kind: TypeErrorKind::MethodNotFound { base_type: Type::Json, method: method_str.to_string() }, span: span.clone() });
                                    Type::Unknown
                                }
                            },
                            "env" => match method_str {
                                "get" => Type::String,
                                "args" => Type::Array(Box::new(Type::String)),
                                _ => {
                                    errors.push(TypeError { kind: TypeErrorKind::UndefinedVariable(format!("method {} for env builtin", method_str)), span: span.clone() });
                                    Type::Unknown
                                }
                            },
                            "crypto" => match method_str {
                                "hash" | "token" => Type::String,
                                "verify" => Type::Bool,
                                _ => {
                                    errors.push(TypeError { kind: TypeErrorKind::UndefinedVariable(format!("method {} for crypto builtin", method_str)), span: span.clone() });
                                    Type::Unknown
                                }
                            },
                            "date" => match method_str {
                                "now" => Type::Date,
                                _ => {
                                    errors.push(TypeError { kind: TypeErrorKind::UndefinedVariable(format!("method {} for date builtin", method_str)), span: span.clone() });
                                    Type::Unknown
                                }
                            },
                            "store" => match method_str {
                                "read" => Type::String,
                                "write" | "append" | "exists" | "delete" | "mkdir" | "zip" | "unzip" | "isDir" => Type::Bool,
                                "size" => Type::Int,
                                "list" | "glob" => Type::Array(Box::new(Type::String)),
                                _ => {
                                    errors.push(TypeError { kind: TypeErrorKind::UndefinedVariable(format!("method {} for store", method_str)), span: span.clone() });
                                    Type::Unknown
                                }
                            },
                            "halt" | "terminal" => Type::Bool,
                            _ => {
                                errors.push(TypeError { kind: TypeErrorKind::UndefinedVariable(format!("builtin service {}", bname)), span: span.clone() });
                                Type::Unknown
                            }
                        }
                    }
                    Type::Json => {
                        for arg in args { self.check_expr(arg.expr(), symbols, errors); }
                        match method_str {
                            "size" | "count" | "len" | "length" => Type::Int,
                            "exists" | "ok" | "status" => Type::Bool,
                            "get" | "parse" | "append" | "push" | "set" | "update" | "delete" | "remove" | "body" | "bind" | "inject" | "first" | "insertId" | "affected" => Type::Json,
                            "toStr" | "toString" | "to_str" | "format" => Type::String,
                            _ => {
                                errors.push(TypeError { 
                                    kind: TypeErrorKind::MethodNotFound { base_type: Type::Json, method: method_str.to_string() }, 
                                    span: span.clone() 
                                });
                                Type::Unknown
                            }
                        }
                    }
                    Type::Database => {
                        let io_methods = ["fetch", "insert", "save", "push", "query", "queryRaw", "remove", "truncate", "exec", "sync", "drop"];
                        if io_methods.contains(&method_str) {
                            self.last_expr_was_db_io = true;
                        }
                        if self.fiber_context.is_some() && !self.in_yield_expr && io_methods.contains(&method_str) {
                             errors.push(TypeError { 
                                 kind: TypeErrorKind::Other(format!("Database I/O method '{}' must be yielded inside a fiber", method_str)), 
                                 span: span.clone() 
                             });
                        }
                        match method_str {
                            "queryRaw" => {
                                for arg in args { self.check_expr_with_context(arg.expr(), symbols, errors, Some(Type::Array(Box::new(Type::Unknown)))); }
                                Type::Json
                            }
                            "fetch" | "sync" | "drop" | "has" | "truncate" | "remove" => {
                                if let Some(arg) = args.first() {
                                    let res_ty = self.check_expr(arg.expr(), symbols, errors);
                                    if !matches!(res_ty, Type::Table(_)) && res_ty != Type::Unknown {
                                        errors.push(TypeError { kind: TypeErrorKind::TypeMismatch { expected: Type::Table(vec![]), actual: res_ty.clone() }, span: arg.expr().span.clone() });
                                    }
                                    if method_str == "has" { 
                                        Type::Bool 
                                    } else if method_str == "remove" {
                                        let columns = if let Type::Table(cols) = res_ty { cols } else { vec![] };
                                        Type::DatabaseOperation(crate::parser::ast::DatabaseOpKind::Remove, columns)
                                    } else if method_str == "fetch" {
                                        res_ty 
                                    } else {
                                        Type::Json
                                    }
                                } else {
                                    errors.push(TypeError { kind: TypeErrorKind::InvalidArgumentCount { expected: 1, actual: 0 }, span: span.clone() });
                                    Type::Unknown
                                }
                            }
                            "insert" | "save" | "push" | "exec" => {
                                if let Some(table_arg) = args.first() {
                                    let ty = self.check_expr(table_arg.expr(), symbols, errors);
                                    if method_str == "exec" {
                                        if ty != Type::String && ty != Type::Unknown {
                                            errors.push(TypeError { kind: TypeErrorKind::TypeMismatch { expected: Type::String, actual: ty }, span: table_arg.expr().span.clone() });
                                        }
                                        if args.len() > 1 {
                                            self.check_expr_with_context(args[1].expr(), symbols, errors, Some(Type::Array(Box::new(Type::Unknown))));
                                        }
                                    } else {
                                        if let Type::Table(ref cols) = ty {
                                            if method_str == "push" || method_str == "save" {
                                                if args.len() != 1 {
                                                    errors.push(TypeError { kind: TypeErrorKind::InvalidArgumentCount { expected: 1, actual: args.len() }, span: span.clone() });
                                                }
                                            } else {
                                                self.check_table_operation_args(cols, &args[1..], symbols, errors, &span, method_str);
                                            }
                                        } else if ty != Type::Unknown {
                                            errors.push(TypeError { kind: TypeErrorKind::TypeMismatch { expected: Type::Table(vec![]), actual: ty }, span: table_arg.expr().span.clone() });
                                        }
                                    }
                                } else {
                                    errors.push(TypeError { kind: TypeErrorKind::InvalidArgumentCount { expected: 1, actual: 0 }, span: span.clone() });
                                }
                                Type::Json
                            }
                            "begin" | "commit" | "rollback" | "close" | "isOpen" => {
                                for arg in args { self.check_expr(arg.expr(), symbols, errors); }
                                Type::Bool
                            }
                            _ => {
                                errors.push(TypeError { kind: TypeErrorKind::MethodNotFound { base_type: Type::Database, method: method_str.to_string() }, span: span.clone() });
                                Type::Unknown
                            }
                        }
                    }
                    Type::DatabaseOperation(kind, cols) => {
                        match (kind, method_str) {
                            (crate::parser::ast::DatabaseOpKind::Remove, "where") => {
                                // .where() on remove executes the delete and returns Json results
                                if let Some(arg_node) = args.first() {
                                     let arg = arg_node.expr();
                                     // Reuse table lambda scope for where predicate
                                     {
                                         let (kind_val, cols) = (kind, cols); 
                                         let _kind = kind_val;
                                         let col_names: Vec<String> = cols.iter()
                                             .map(|c| self.interner.lookup(c.name).trim().to_string())
                                             .collect();
                                         let prev_lambda = self.is_table_lambda;
                                         self.is_table_lambda = true;
                                         
                                         symbols.enter_scope();
                                         symbols.define("__row_tmp".to_string(), Type::Table(cols.clone()), false);
                                         
                                         let mut pred_idents = Vec::new();
                                         self.collect_pred_idents(arg, &mut pred_idents);
                                         for ident_name in &pred_idents {
                                             if symbols.has(ident_name) && ident_name != "__row_tmp" && col_names.contains(ident_name) {
                                                 errors.push(TypeError {
                                                     kind: TypeErrorKind::WherePredicateNameCollision {
                                                         var_name: ident_name.clone(),
                                                         column_name: ident_name.clone(),
                                                     },
                                                     span: arg.span.clone(),
                                                 });
                                             }
                                         }

                                         let pred_ty = self.check_expr(arg, symbols, errors);
                                         if pred_ty != Type::Bool && pred_ty != Type::Unknown {
                                             errors.push(TypeError {
                                                 kind: TypeErrorKind::TypeMismatch { expected: Type::Bool, actual: pred_ty },
                                                 span: arg.span.clone(),
                                             });
                                         }
                                         
                                         symbols.exit_scope();
                                         self.is_table_lambda = prev_lambda;
                                     }
                                 }
                                Type::Json
                            }
                            _ => {
                                errors.push(TypeError {
                                    kind: TypeErrorKind::MethodNotFound { base_type: Type::DatabaseOperation(kind, cols.clone()), method: method_str.to_string() },
                                    span: span.clone()
                                });
                                Type::Unknown
                            }
                        }
                    }
                    Type::Fiber(ref inner) => {
                        match method_str {
                             "next" => {
                                 if let Some(inner_ty) = inner {
                                     for arg in args { self.check_expr(arg.expr(), symbols, errors); }
                                     (**inner_ty).clone()
                                 } else {
                                    errors.push(TypeError { kind: TypeErrorKind::Other("Cannot call .next() on a void fiber".to_string()), span: span.clone() });
                                    Type::Unknown
                                }
                            }
                            "run" => {
                                if inner.is_none() { Type::Bool }
                                else {
                                    errors.push(TypeError { kind: TypeErrorKind::CannotRunTypedFiber, span: span.clone() });
                                    Type::Unknown
                                }
                            }
                            "isDone" | "close" => Type::Bool,
                            _ => {
                                errors.push(TypeError { 
                                    kind: TypeErrorKind::MethodNotFound { base_type: Type::Fiber(inner.clone()), method: method_str.to_string() }, 
                                    span: span.clone() 
                                });
                                Type::Unknown
                            }
                        }
                    }
                    Type::Set(ref st) => {
                        let inner_ty = match st {
                            crate::parser::ast::SetType::N | crate::parser::ast::SetType::Z => Type::Int,
                            crate::parser::ast::SetType::Q => Type::Float,
                            crate::parser::ast::SetType::S | crate::parser::ast::SetType::C => Type::String,
                            crate::parser::ast::SetType::B => Type::Bool,
                        };
                        match method_str {
                            "size" | "count" | "length" => Type::Int,
                            "contains" | "add" | "remove" => {
                                if let Some(arg) = args.first() {
                                    let arg_ty = self.check_expr(arg.expr(), symbols, errors);
                                    if arg_ty != Type::Unknown && arg_ty != inner_ty {
                                        errors.push(TypeError { kind: TypeErrorKind::TypeMismatch { expected: inner_ty, actual: arg_ty }, span: arg.expr().span.clone() });
                                    }
                                }
                                Type::Bool
                            }
                            "isEmpty" | "clear" | "show" => Type::Bool,
                            _ => {
                                errors.push(TypeError { 
                                    kind: TypeErrorKind::MethodNotFound { base_type: Type::Set(st.clone()), method: method_str.to_string() }, 
                                    span: span.clone() 
                                });
                                Type::Unknown
                            }
                        }
                    }
                    Type::Array(ref inner) => {
                        match method_str {
                            "size" | "length" | "count" => Type::Int,
                            "isEmpty" => Type::Bool,
                            "get" | "delete" | "remove" | "contains" | "find" | "indexOf" | "pop" => {
                                if args.len() == 1 {
                                    let arg_ty = self.check_expr(args[0].expr(), symbols, errors);
                                    if (method_str == "get" || method_str == "delete" || method_str == "remove") && arg_ty != Type::Int && arg_ty != Type::Unknown {
                                        errors.push(TypeError { kind: TypeErrorKind::TypeMismatch { expected: Type::Int, actual: arg_ty }, span: args[0].expr().span.clone() });
                                    }
                                }
                                if method_str == "get" || method_str == "pop" { (**inner).clone() }
                                else if method_str == "find" || method_str == "indexOf" { Type::Int }
                                else { Type::Bool }
                            }
                            "push" | "insert" | "update" | "set" => {
                                for arg in args { self.check_expr(arg.expr(), symbols, errors); }
                                Type::Bool
                            }
                            "toStr" | "toString" | "toJson" | "show" | "clear" | "sort" | "reverse" => Type::Bool,
                            _ => {
                                errors.push(TypeError { 
                                    kind: TypeErrorKind::MethodNotFound { base_type: Type::Array(inner.clone()), method: method_str.to_string() }, 
                                    span: span.clone() 
                                });
                                Type::Unknown
                            }
                        }
                    }
                    Type::Map(ref k, ref v) => {
                        match method_str {
                            "size" | "count" | "isEmpty" | "clear" | "show" => {
                                if method_str == "size" || method_str == "count" { Type::Int } else { Type::Bool }
                            }
                            "get" | "contains" | "remove" | "delete" => {
                                if !args.is_empty() {
                                    let key_ty = self.check_expr(args[0].expr(), symbols, errors);
                                    if key_ty != Type::Unknown && !self.is_compatible(k, &key_ty) {
                                        errors.push(TypeError { kind: TypeErrorKind::TypeMismatch { expected: (**k).clone(), actual: key_ty }, span: args[0].expr().span.clone() });
                                    }
                                }
                                if method_str == "get" { (**v).clone() } else { Type::Bool }
                            }
                            "insert" | "set" | "update" => {
                                for arg in args { self.check_expr(arg.expr(), symbols, errors); }
                                Type::Bool
                            }
                            "keys" => Type::Array(k.clone()),
                            "values" => Type::Array(v.clone()),
                            "toStr" | "toString" | "to_str" => Type::String,
                            "toJson" => Type::Json,
                            _ => {
                                errors.push(TypeError { 
                                    kind: TypeErrorKind::MethodNotFound { base_type: Type::Map(k.clone(), v.clone()), method: method_str.to_string() }, 
                                    span: span.clone() 
                                });
                                Type::Unknown
                            }
                        }
                    }
                    Type::Date => {
                        for arg in args { self.check_expr(arg.expr(), symbols, errors); }
                        match method_str {
                            "format" => Type::String,
                            _ => {
                                errors.push(TypeError { 
                                    kind: TypeErrorKind::MethodNotFound { base_type: Type::Date, method: method_str.to_string() }, 
                                    span: span.clone() 
                                });
                                Type::Unknown
                            }
                        }
                    }
                    Type::String => {
                        match method_str {
                            "size" | "length" | "indexOf" | "lastIndexOf" => {
                                for arg in args { self.check_expr(arg.expr(), symbols, errors); }
                                Type::Int
                            }
                            "upper" | "lower" | "trim" => {
                                if !args.is_empty() {
                                    errors.push(TypeError { kind: TypeErrorKind::InvalidArgumentCount { expected: 0, actual: args.len() }, span: span.clone() });
                                }
                                Type::String
                            }
                            "toInt" | "toFloat" => {
                                if !args.is_empty() {
                                    errors.push(TypeError { kind: TypeErrorKind::InvalidArgumentCount { expected: 0, actual: args.len() }, span: span.clone() });
                                }
                                if method_str == "toInt" { Type::Int } else { Type::Float }
                            }
                            "startsWith" | "endsWith" | "char" | "charAt" | "replace" | "slice" => {
                                for arg in args { self.check_expr(arg.expr(), symbols, errors); }
                                if method_str == "startsWith" || method_str == "endsWith" { Type::Bool } else { Type::String }
                            }
                            "split" => {
                                for arg in args { self.check_expr(arg.expr(), symbols, errors); }
                                Type::Array(Box::new(Type::String))
                            }
                            _ => {
                                errors.push(TypeError { 
                                    kind: TypeErrorKind::MethodNotFound { base_type: Type::String, method: method_str.to_string() }, 
                                    span: span.clone() 
                                });
                                Type::Unknown
                            }
                        }
                    }
                    _ => {
                        if let ExprKind::Identifier(rec_id) = &receiver.kind {
                            let rec_str = self.interner.lookup(*rec_id).trim();
                            let namespaced_name = format!("{}.{}", rec_str, method_str);
                            if let Some(sig) = self.functions.get(&namespaced_name).cloned() {
                                for (i, arg) in args.iter().enumerate() {
                                    let arg_ty = self.check_expr(arg.expr(), symbols, errors);
                                    if i < sig.params.len() && !self.is_compatible(&sig.params[i], &arg_ty) {
                                        errors.push(TypeError { kind: TypeErrorKind::TypeMismatch { expected: sig.params[i].clone(), actual: arg_ty }, span: arg.expr().span.clone() });
                                    }
                                }
                                return sig.return_type.unwrap_or(Type::Unknown);
                            }
                        }
                        if rec_ty != Type::Unknown {
                            errors.push(TypeError { 
                                kind: TypeErrorKind::MethodNotFound { base_type: rec_ty, method: method_str.to_string() }, 
                                span: span.clone() 
                            });
                        }
                        Type::Unknown
                    }
                }
            }
        }
    }

    fn check_argument_list(&mut self, args: &Vec<crate::parser::ast::Argument>, symbols: &mut SymbolTable<'_>, errors: &mut Vec<TypeError>, allow_named: bool) {
        let mut seen_named = false;
        let mut seen_names = std::collections::HashSet::new();
        
        for arg_node in args {
            match arg_node {
                crate::parser::ast::Argument::Positional(expr) => {
                    if seen_named {
                        errors.push(TypeError {
                            kind: TypeErrorKind::Other("Positional arguments must come before named arguments".to_string()),
                            span: expr.span.clone(),
                        });
                    }
                    self.check_expr(expr, symbols, errors);
                }
                crate::parser::ast::Argument::Named(name, expr) => {
                    if !allow_named {
                        errors.push(TypeError {
                            kind: TypeErrorKind::Other(format!("Named arguments are not allowed here (method: {})", self.interner.lookup(*name).trim())),
                            span: expr.span.clone(),
                        });
                    }
                    seen_named = true;
                    let name_str = self.interner.lookup(*name).trim().to_string();
                    if !seen_names.insert(name_str.clone()) {
                        errors.push(TypeError {
                            kind: TypeErrorKind::Other(format!("Duplicate named argument: {}", name_str)),
                            span: expr.span.clone(),
                        });
                    }
                    self.check_expr(expr, symbols, errors);
                }
            }
        }
    }

    fn is_compatible(&self, expected: &Type, actual: &Type) -> bool {
        if actual == &Type::Unknown || expected == &Type::Unknown || expected == actual { return true; }
        if expected == &Type::Json || actual == &Type::Json { return true; }
        if let (Type::Builtin(id1), Type::Builtin(id2)) = (expected, actual) {
            return id1 == id2;
        }
        match (expected, actual) {
            (Type::Int, Type::Float) | (Type::Float, Type::Int) | (Type::Int, Type::Date) | (Type::Date, Type::Int) => true,
            (Type::Array(e), Type::Array(a)) => self.is_compatible(e, a),
            (Type::Set(st), Type::Array(inner)) | (Type::Array(inner), Type::Set(st)) => {
                let inner_target = match st {
                    SetType::N | SetType::Z => Type::Int,
                    SetType::Q => Type::Float,
                    SetType::S | SetType::C => Type::String,
                    SetType::B => Type::Bool,
                };
                &inner_target == inner.as_ref() || inner.as_ref() == &Type::Unknown
            }
            (Type::Set(e_st), Type::Set(a_st)) => {
                let e_base = match e_st {
                    SetType::N | SetType::Z => 1,
                    SetType::Q => 2,
                    SetType::S | SetType::C => 3,
                    SetType::B => 4,
                };
                let a_base = match a_st {
                    SetType::N | SetType::Z => 1,
                    SetType::Q => 2,
                    SetType::S | SetType::C => 3,
                    SetType::B => 4,
                };
                e_base == a_base
            }
            (Type::Map(ek, ev), Type::Map(ak, av)) => {
                self.is_compatible(ek, ak) && self.is_compatible(ev, av)
            }
            (Type::Fiber(et), Type::Fiber(at)) => {
                match (et, at) {
                    (None, None) => true,
                    (Some(e), Some(a)) => self.is_compatible(e, a),
                    _ => false,
                }
            }
            (Type::Table(e_cols), Type::Table(a_cols)) => {
                if e_cols.is_empty() || a_cols.is_empty() { return true; }
                if e_cols.len() != a_cols.len() { return false; }
                e_cols.iter().zip(a_cols.iter()).all(|(e, a)| self.is_compatible(&e.ty, &a.ty))
            }
            _ => false
        }
    }

    fn check_table_operation_args(
        &mut self,
        cols: &[ColumnDef],
        args: &[Argument],
        symbols: &mut SymbolTable,
        errors: &mut Vec<TypeError>,
        span: &Span,
        _method: &str,
    ) {
        if cols.is_empty() {
            for arg in args {
                self.check_expr(arg.expr(), symbols, errors);
            }
            return;
        }
        let non_auto_cols: Vec<_> = cols.iter().filter(|c| !c.is_auto()).collect();
        let mut positional_count = 0;
        let mut named_cols = std::collections::HashSet::new();
        let mut seen_named = false;

        // Support for unrolling a single array/tuple positional argument
        let mut processed_args = Cow::Borrowed(args);
        if args.len() == 1 {
            if let Argument::Positional(expr) = &args[0] {
                match &expr.kind {
                    ExprKind::ArrayLiteral { elements } | ExprKind::Tuple(elements) => {
                        if elements.len() == non_auto_cols.len() {
                            let mut unrolled = Vec::with_capacity(elements.len());
                            for e in elements {
                                unrolled.push(Argument::Positional(e.clone()));
                            }
                            processed_args = Cow::Owned(unrolled);
                        }
                    }
                    _ => {}
                }
            }
        }

        for arg in processed_args.as_ref() {
            match arg {
                Argument::Positional(expr) => {
                    if seen_named {
                        errors.push(TypeError {
                            kind: TypeErrorKind::Other("Positional arguments must come before named arguments".to_string()),
                            span: expr.span.clone(),
                        });
                    }
                    if positional_count >= non_auto_cols.len() {
                        errors.push(TypeError { kind: TypeErrorKind::Other("Too many positional arguments for table operation".to_string()), span: expr.span.clone() });
                    } else {
                        let expected = &non_auto_cols[positional_count].ty;
                        let actual = self.check_expr(expr, symbols, errors);
                        if actual != Type::Unknown && !self.is_compatible(expected, &actual) {
                            errors.push(TypeError { kind: TypeErrorKind::TypeMismatch { expected: expected.clone(), actual: actual.clone() }, span: expr.span.clone() });
                        }
                        positional_count += 1;
                    }
                }
                Argument::Named(name, expr) => {
                    seen_named = true;
                    let name_str = self.interner.lookup(*name).trim();
                    if let Some(col) = cols.iter().find(|c| self.interner.lookup(c.name).trim() == name_str) {
                        if col.is_auto() {
                            // Rule 1.4: @auto columns never allowed in named args for insert/add
                            errors.push(TypeError { kind: TypeErrorKind::Other(format!("Cannot provide value for @auto column '{}'", name_str)), span: expr.span.clone() });
                        }
                        if let Some(pos) = non_auto_cols.iter().position(|c| self.interner.lookup(c.name).trim() == name_str) {
                            if pos < positional_count {
                                errors.push(TypeError { kind: TypeErrorKind::Other(format!("Column '{}' already provided via positional argument", name_str)), span: expr.span.clone() });
                            }
                        }
                        if !named_cols.insert(name_str) {
                            errors.push(TypeError { kind: TypeErrorKind::Other(format!("Duplicate named argument: {}", name_str)), span: expr.span.clone() });
                        }
                        let actual = self.check_expr(expr, symbols, errors);
                        if actual != Type::Unknown && !self.is_compatible(&col.ty, &actual) {
                            errors.push(TypeError { kind: TypeErrorKind::TypeMismatch { expected: col.ty.clone(), actual: actual.clone() }, span: expr.span.clone() });
                        }
                    } else {
                        errors.push(TypeError { kind: TypeErrorKind::Other(format!("Column '{}' not found in table", name_str)), span: expr.span.clone() });
                    }
                }
            }
        }
        for col in &non_auto_cols {
            let col_name = self.interner.lookup(col.name).trim();
            let pos_idx = non_auto_cols.iter().position(|c| self.interner.lookup(c.name).trim() == col_name).unwrap();
            let covered_by_pos = pos_idx < positional_count;
            if !covered_by_pos && !named_cols.contains(col_name) {
                 let can_omit = col.attributes.iter().any(|a| matches!(a, ColumnAttribute::Optional | ColumnAttribute::Default(_)));
                 if !can_omit {
                     errors.push(TypeError { kind: TypeErrorKind::Other(format!("Missing value for required column '{}'", col_name)), span: span.clone() });
                 }
            }
        }
    }
}