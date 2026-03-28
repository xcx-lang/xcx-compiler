pub mod vm;
pub mod jit;
pub mod repl;
#[cfg(test)]
mod tests;

use crate::parser::ast::{Stmt, Expr};
use crate::backend::vm::{OpCode, Value, FunctionChunk, MethodKind, SetData};
use crate::lexer::token::TokenKind;
use crate::sema::interner::{Interner, StringId};
use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::RwLock;

pub struct Compiler {
    pub globals: HashMap<StringId, usize>,
    pub func_indices: HashMap<StringId, usize>,
    pub functions: Vec<FunctionChunk>,
    pub constants: Vec<Value>,
    pub string_constants: HashMap<String, usize>,
}

pub struct CompileContext<'a> {
    pub constants: &'a mut Vec<Value>,
    pub string_constants: &'a mut HashMap<String, usize>,
    pub functions: &'a mut Vec<FunctionChunk>,
    pub func_indices: &'a HashMap<StringId, usize>,
    pub globals: &'a HashMap<StringId, usize>,
    pub interner: &'a mut Interner,
}

impl<'a> CompileContext<'a> {
    pub fn add_constant(&mut self, val: Value) -> u32 {
        if val.is_string() {
            let s = val.as_string();
            if let Some(&idx) = self.string_constants.get(s.as_ref()) {
                return idx as u32;
            }
            let idx = self.constants.len();
            self.string_constants.insert(s.to_string(), idx);
            self.constants.push(val);
            return idx as u32;
        }
        self.constants.push(val);
        (self.constants.len() - 1) as u32
    }
}

pub struct FunctionCompiler {
    pub bytecode: Vec<OpCode>,
    pub spans: Vec<crate::lexer::token::Span>,
    pub scopes: Vec<HashMap<StringId, usize>>,
    pub next_local: usize,
    pub loop_stack: Vec<(usize, Vec<usize>, Vec<usize>, Option<usize>)>,
    pub parent_locals: Option<HashMap<StringId, usize>>,
    pub captures: Vec<StringId>,
    pub is_main: bool,
    pub is_table_lambda: bool,
    pub max_locals_used: usize,
}

impl FunctionCompiler {
    pub fn new(is_main: bool, parent_locals: Option<HashMap<StringId, usize>>) -> Self {
        Self {
            bytecode: Vec::new(),
            spans: Vec::new(),
            scopes: vec![HashMap::new()],
            next_local: 0,
            max_locals_used: 0,
            loop_stack: Vec::new(),
            parent_locals,
            captures: Vec::new(),
            is_main,
            is_table_lambda: false,
        }
    }

    fn emit(&mut self, op: OpCode, span: &crate::lexer::token::Span) {
        self.bytecode.push(op);
        self.spans.push(span.clone());
    }

    pub fn enter_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    pub fn exit_scope(&mut self) {
        self.scopes.pop();
    }

       pub fn lookup_local(&mut self, id: &StringId) -> Option<usize> {
        for scope in self.scopes.iter().rev() {
            if let Some(&slot) = scope.get(id) {
                return Some(slot);
            }
        }
        
        if let Some(parent) = &self.parent_locals {
            if parent.contains_key(id) {
                if let Some(pos) = self.captures.iter().position(|c| c == id) {
                    return Some(1 + pos);
                } else {
                    let pos = self.captures.len();
                    self.captures.push(*id);
                    let slot = 1 + pos;
                    self.scopes[0].insert(*id, slot);
                    if slot >= self.next_local { self.next_local = slot + 1; }
                    return Some(slot);
                }
            }
        }
        None
    }

    fn collect_captures(&self, expr: &Expr, parent_locals: &HashMap<StringId, usize>, out: &mut Vec<StringId>) {
        use crate::parser::ast::ExprKind;
        match &expr.kind {
            ExprKind::Identifier(id) => {
                if parent_locals.contains_key(id) && !out.contains(id) {
                    out.push(*id);
                }
            }
            ExprKind::Binary { left, right, .. } => {
                self.collect_captures(left, parent_locals, out);
                self.collect_captures(right, parent_locals, out);
            }
            ExprKind::Unary { right, .. } => {
                self.collect_captures(right, parent_locals, out);
            }
            ExprKind::FunctionCall { args, .. } => {
                for arg in args { self.collect_captures(arg, parent_locals, out); }
            }
            ExprKind::MethodCall { receiver, args, .. } => {
                self.collect_captures(receiver, parent_locals, out);
                for arg in args { self.collect_captures(arg, parent_locals, out); }
            }
            ExprKind::ArrayLiteral { elements } => {
                for e in elements { self.collect_captures(e, parent_locals, out); }
            }
            ExprKind::MapLiteral { elements, .. } => {
                for (k, v) in elements {
                    self.collect_captures(k, parent_locals, out);
                    self.collect_captures(v, parent_locals, out);
                }
            }
            ExprKind::SetLiteral { elements, range, .. } => {
                for e in elements { self.collect_captures(e, parent_locals, out); }
                if let Some(r) = range {
                    self.collect_captures(&r.start, parent_locals, out);
                    self.collect_captures(&r.end, parent_locals, out);
                    if let Some(s) = &r.step { self.collect_captures(s, parent_locals, out); }
                }
            }
            ExprKind::Index { receiver, index } => {
                self.collect_captures(receiver, parent_locals, out);
                self.collect_captures(index, parent_locals, out);
            }
            ExprKind::MemberAccess { receiver, .. } => {
                self.collect_captures(receiver, parent_locals, out);
            }
            ExprKind::Lambda { body, .. } => {
                self.collect_captures(body, parent_locals, out);
            }
            _ => {}
        }
    }

    fn define_local(&mut self, id: StringId, slot: usize) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(id, slot);
        }
    }

    fn convert_to_flat_locals(&self) -> HashMap<StringId, usize> {
        let mut flat = HashMap::new();
        for scope in &self.scopes {
            for (&id, &slot) in scope {
                flat.insert(id, slot);
            }
        }
        flat
    }

    fn map_method_kind(&self, name: &str) -> Option<MethodKind> {
        match name {
            "push" => Some(MethodKind::Push),
            "pop" => Some(MethodKind::Pop),
            "len" => Some(MethodKind::Len),
            "count" => Some(MethodKind::Count),
            "size" => Some(MethodKind::Size),
            "clear" => Some(MethodKind::Clear),
            "contains" => Some(MethodKind::Contains),
            "isEmpty" => Some(MethodKind::IsEmpty),
            "get" => Some(MethodKind::Get),
            "insert" => Some(MethodKind::Insert),
            "update" => Some(MethodKind::Update),
            "delete" => Some(MethodKind::Delete),
            "find" => Some(MethodKind::Find),
            "join" => Some(MethodKind::Join),
            "show" => Some(MethodKind::Show),
            "sort" => Some(MethodKind::Sort),
            "reverse" => Some(MethodKind::Reverse),
            "add" => Some(MethodKind::Add),
            "remove" => Some(MethodKind::Remove),
            "has" => Some(MethodKind::Has),
            "length" => Some(MethodKind::Length),
            "upper" => Some(MethodKind::Upper),
            "lower" => Some(MethodKind::Lower),
            "trim" => Some(MethodKind::Trim),
            "indexOf" => Some(MethodKind::IndexOf),
            "lastIndexOf" => Some(MethodKind::LastIndexOf),
            "replace" => Some(MethodKind::Replace),
            "slice" => Some(MethodKind::Slice),
            "split" => Some(MethodKind::Split),
            "startsWith" | "starts_with" => Some(MethodKind::StartsWith),
            "endsWith" | "ends_with" => Some(MethodKind::EndsWith),
            "toInt" | "to_int" => Some(MethodKind::ToInt),
            "toFloat" | "to_float" => Some(MethodKind::ToFloat),
            "set" => Some(MethodKind::Set),
            "keys" => Some(MethodKind::Keys),
            "values" => Some(MethodKind::Values),
            "where" => Some(MethodKind::Where),
            "year" => Some(MethodKind::Year),
            "month" => Some(MethodKind::Month),
            "day" => Some(MethodKind::Day),
            "hour" => Some(MethodKind::Hour),
            "minute" => Some(MethodKind::Minute),
            "second" => Some(MethodKind::Second),
            "format" => Some(MethodKind::Format),
            "exists" => Some(MethodKind::Exists),
            "append" => Some(MethodKind::Append),
            "inject" => Some(MethodKind::Inject),
            "to_str" | "to_string" | "toString" => Some(MethodKind::ToStr),
            "next" => Some(MethodKind::Next),
            "run" => Some(MethodKind::Run),
            "isDone" => Some(MethodKind::IsDone),
            "close" => Some(MethodKind::Close),
            _ => None,
        }
    }

    fn get_default_value(&self, ty: &crate::parser::ast::Type, ctx: &mut CompileContext) -> Value {
        match ty {
            crate::parser::ast::Type::Int => Value::from_i64(0),
            crate::parser::ast::Type::Float => Value::from_f64(0.0),
            crate::parser::ast::Type::String => Value::from_string("".to_string().into()),
            crate::parser::ast::Type::Bool => Value::from_bool(false),
            crate::parser::ast::Type::Array(_) => Value::from_array(Arc::new(RwLock::new(Vec::new()))),
            crate::parser::ast::Type::Set(_) => Value::from_set(Arc::new(RwLock::new(SetData { elements: std::collections::BTreeSet::new(), cache: None }))),
            crate::parser::ast::Type::Map(_, _) => Value::from_map(Arc::new(RwLock::new(Vec::new()))),
            crate::parser::ast::Type::Date => Value::from_date(0),
            crate::parser::ast::Type::Table(cols) => {
                let vm_cols = cols.iter().map(|c| crate::backend::vm::VMColumn {
                    name: ctx.interner.lookup(c.name).to_string(),
                    ty: c.ty.clone(),
                    is_auto: c.is_auto,
                }).collect();
                Value::from_table(Arc::new(RwLock::new(
                    crate::backend::vm::TableData { columns: vm_cols, rows: Vec::new() }
                )))
            }
            crate::parser::ast::Type::Json => Value::from_json(Arc::new(RwLock::new(serde_json::Value::Null))),
            crate::parser::ast::Type::Builtin(_) => Value::from_string("builtin".to_string().into()),
            crate::parser::ast::Type::Unknown => Value::from_i64(0),
            crate::parser::ast::Type::Fiber(_) => Value::from_bool(false),
        }
    }

    pub fn push_reg(&mut self) -> u8 {
        let r = self.next_local as u8;
        self.next_local += 1;
        if self.next_local > self.max_locals_used {
            self.max_locals_used = self.next_local;
        }
        r
    }

    pub fn pop_reg(&mut self) {
        self.next_local -= 1;
    }

    pub fn compile_expr(&mut self, expr: &Expr, ctx: &mut CompileContext) -> u8 {
        match &expr.kind {
            crate::parser::ast::ExprKind::IntLiteral(v) => {
                let i = ctx.add_constant(Value::from_i64(*v));
                let dst = self.push_reg();
                self.emit(OpCode::LoadConst { dst, idx: i }, &expr.span);
                dst
            }
            crate::parser::ast::ExprKind::FloatLiteral(v) => {
                let i = ctx.add_constant(Value::from_f64(*v));
                let dst = self.push_reg();
                self.emit(OpCode::LoadConst { dst, idx: i }, &expr.span);
                dst
            }
            crate::parser::ast::ExprKind::StringLiteral(id) => {
                let s = ctx.interner.lookup(*id).to_string();
                let i = ctx.add_constant(Value::from_string(Arc::new(s)));
                let dst = self.push_reg();
                self.emit(OpCode::LoadConst { dst, idx: i }, &expr.span);
                dst
            }
            crate::parser::ast::ExprKind::BoolLiteral(v) => {
                let i = ctx.add_constant(Value::from_bool(*v));
                let dst = self.push_reg();
                self.emit(OpCode::LoadConst { dst, idx: i }, &expr.span);
                dst
            }
            crate::parser::ast::ExprKind::Identifier(id) => {
                if let Some(slot) = self.lookup_local(id) {
                    let dst = self.push_reg();
                    self.emit(OpCode::Move { dst, src: slot as u8 }, &expr.span);
                    dst
                } else if let Some(&idx) = ctx.globals.get(id) {
                    let dst = self.push_reg();
                    self.emit(OpCode::GetVar { dst, idx: idx as u32 }, &expr.span);
                    dst
                } else if self.is_table_lambda {
                    // In a table lambda, unknown identifiers are treated as row members.
                    // The row object is always the first parameter (R0).
                    let dst = self.push_reg();
                    let mi = ctx.add_constant(Value::from_string(Arc::new(ctx.interner.lookup(*id).to_string())));
                    self.emit(OpCode::MethodCallCustom { dst, method_name_idx: mi, base: 0, arg_count: 0 }, &expr.span);
                    dst
                } else if let Some(&fid) = ctx.func_indices.get(id) {
                    let dst = self.push_reg();
                    let i = ctx.add_constant(Value::from_function(fid as u32));
                    self.emit(OpCode::LoadConst { dst, idx: i }, &expr.span);
                    dst
                } else {
                    // Default fallback: return identifier name as a string constant
                    let dst = self.push_reg();
                    let name = ctx.interner.lookup(*id).to_string();
                    let i = ctx.add_constant(Value::from_string(Arc::new(name)));
                    self.emit(OpCode::LoadConst { dst, idx: i }, &expr.span);
                    dst
                }
            }
            crate::parser::ast::ExprKind::FunctionCall { name, args } => {
                let n = ctx.interner.lookup(*name);
                if n == "json.parse" && args.len() == 1 {
                    let src = self.compile_expr(&args[0], ctx);
                    let dst = src; // Reuse register
                    self.emit(OpCode::JsonParse { dst, src }, &expr.span);
                    dst
                } else if n == "terminal.input" {
                    let dst = self.push_reg();
                    self.emit(OpCode::Input { dst }, &expr.span);
                    dst
                } else if n == "i" && args.len() == 1 {
                    let src = self.compile_expr(&args[0], ctx);
                    let dst = src;
                    self.emit(OpCode::CastInt { dst, src }, &expr.span);
                    dst
                } else if n == "f" && args.len() == 1 {
                    let src = self.compile_expr(&args[0], ctx);
                    let dst = src;
                    self.emit(OpCode::CastFloat { dst, src }, &expr.span);
                    dst
                } else if n == "s" && args.len() == 1 {
                    let src = self.compile_expr(&args[0], ctx);
                    let dst = src;
                    self.emit(OpCode::CastString { dst, src }, &expr.span);
                    dst
                } else if n == "b" && args.len() == 1 {
                    let src = self.compile_expr(&args[0], ctx);
                    let dst = src;
                    self.emit(OpCode::CastBool { dst, src }, &expr.span);
                    dst
                } else {
                    let base = self.next_local as u8;
                    for arg in args {
                        self.compile_expr(arg, ctx);
                    }
                    let dst = base;
                    if let Some(&fid) = ctx.func_indices.get(name) {
                        if ctx.functions[fid].is_fiber {
                            self.emit(OpCode::FiberCreate { dst, func_idx: fid as u32, base, arg_count: args.len() as u8 }, &expr.span);
                        } else {
                            self.emit(OpCode::Call { dst, func_idx: fid as u32, base, arg_count: args.len() as u8 }, &expr.span);
                        }
                    } else {
                        // Dynamic call? Not supported yet in 2.2, but we can emit a placeholder
                        self.emit(OpCode::Halt, &expr.span);
                    }
                    self.next_local = (base + 1) as usize;
                    dst
                }
            }
            crate::parser::ast::ExprKind::ArrayLiteral { elements } => {
                let base = self.next_local as u8;
                for e in elements {
                    self.compile_expr(e, ctx);
                }
                let dst = base;
                self.emit(OpCode::ArrayInit { dst, base, count: elements.len() as u32 }, &expr.span);
                self.next_local = (base + 1) as usize;
                dst
            }
            crate::parser::ast::ExprKind::SetLiteral { elements, range, .. } => {
                if let Some(r) = range {
                    let start = self.compile_expr(&r.start, ctx);
                    let end   = self.compile_expr(&r.end, ctx);
                    let (step, has_step_reg) = if let Some(s) = &r.step {
                        let step_reg = self.compile_expr(s, ctx);
                        let h_idx = ctx.add_constant(Value::from_bool(true));
                        let h_reg = self.push_reg();
                        self.emit(OpCode::LoadConst { dst: h_reg, idx: h_idx }, &expr.span);
                        (step_reg, h_reg)
                    } else {
                        let dummy = self.push_reg(); // just to have something
                        let f_idx = ctx.add_constant(Value::from_bool(false));
                        let f_reg = self.push_reg();
                        self.emit(OpCode::LoadConst { dst: f_reg, idx: f_idx }, &expr.span);
                        (dummy, f_reg)
                    };
                    let dst = start;
                    self.emit(OpCode::SetRange { dst, start, end, step, has_step: has_step_reg }, &expr.span);
                    self.next_local = (dst + 1) as usize;
                    dst
                } else {
                    let base = self.next_local as u8;
                    for e in elements {
                        self.compile_expr(e, ctx);
                    }
                    let dst = base;
                    self.emit(OpCode::SetInit { dst, base, count: elements.len() as u32 }, &expr.span);
                    self.next_local = (base + 1) as usize;
                    dst
                }
            }
            crate::parser::ast::ExprKind::MapLiteral { elements, .. } => {
                let base = self.next_local as u8;
                for (k, v) in elements {
                    self.compile_expr(k, ctx);
                    self.compile_expr(v, ctx);
                }
                let dst = base;
                self.emit(OpCode::MapInit { dst, base, count: elements.len() as u32 }, &expr.span);
                self.next_local = (base + 1) as usize;
                dst
            }
            crate::parser::ast::ExprKind::RandomChoice { set } => {
                let src = self.compile_expr(set, ctx);
                let dst = src;
                self.emit(OpCode::RandomChoice { dst, src }, &expr.span);
                dst
            }
            crate::parser::ast::ExprKind::DateLiteral { date_string, format } => {
                let date_str = ctx.interner.lookup(*date_string).to_string();
                let date = if let Some(fmt_id) = format {
                    let fmt_str = ctx.interner.lookup(*fmt_id).to_string();
                    let chrono_fmt = fmt_str
                        .replace("YYYY", "%Y").replace("MM", "%m").replace("DD", "%d")
                        .replace("M", "%-m").replace("D", "%-d");
                    chrono::NaiveDate::parse_from_str(&date_str, &chrono_fmt)
                        .unwrap_or_else(|_| chrono::NaiveDate::from_ymd_opt(1970, 1, 1).unwrap())
                } else {
                    chrono::NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
                        .unwrap_or_else(|_| chrono::NaiveDate::from_ymd_opt(1970, 1, 1).unwrap())
                };
                let dt = date.and_hms_opt(0, 0, 0).unwrap();
                let i = ctx.add_constant(Value::from_date(dt.and_utc().timestamp_millis()));
                let dst = self.push_reg();
                self.emit(OpCode::LoadConst { dst, idx: i }, &expr.span);
                dst
            }
            crate::parser::ast::ExprKind::MethodCall { receiver, method, args } => {
                let method_name = ctx.interner.lookup(*method).to_string();
                let mut is_store = false;
                let mut is_date = false;
                let mut is_json = false;
                let mut is_env = false;
                let mut is_crypto = false;
                if let crate::parser::ast::ExprKind::Identifier(rid) = &receiver.kind {
                    let rname = ctx.interner.lookup(*rid);
                    if rname == "store" { is_store = true; }
                    if rname == "date" { is_date = true; }
                    if rname == "json" { is_json = true; }
                    if rname == "env" { is_env = true; }
                    if rname == "crypto" { is_crypto = true; }
                }

                if is_store {
                    let base = self.next_local as u8;
                    for arg in args { self.compile_expr(arg, ctx); }
                    let dst = base; // arbitrary, many store ops don't return meaningful value or return null
                    match method_name.as_str() {
                        "write"  => self.emit(OpCode::StoreWrite { base }, &expr.span),
                        "read"   => self.emit(OpCode::StoreRead { dst, base }, &expr.span),
                        "append" => self.emit(OpCode::StoreAppend { base }, &expr.span),
                        "exists" => self.emit(OpCode::StoreExists { dst, base }, &expr.span),
                        "delete" => self.emit(OpCode::StoreDelete { base }, &expr.span),
                        _ => {}
                    }
                    self.next_local = (base + 1) as usize;
                    return dst;
                }
                if is_date && method_name == "now" {
                    let dst = self.push_reg();
                    self.emit(OpCode::DateNow { dst }, &expr.span);
                    return dst;
                }
                if is_json && method_name == "parse" {
                    let src = self.compile_expr(&args[0], ctx);
                    let dst = src;
                    self.emit(OpCode::JsonParse { dst, src }, &expr.span);
                    return dst;
                }
                if is_env {
                    let dst = self.push_reg();
                    if method_name == "get" {
                        if let Some(arg) = args.first() {
                            let src = self.compile_expr(arg, ctx);
                            self.emit(OpCode::EnvGet { dst, src }, &expr.span);
                            self.pop_reg(); // pop src if it was temp
                        }
                    } else if method_name == "args" {
                        self.emit(OpCode::EnvArgs { dst }, &expr.span);
                    }
                    return dst;
                }
                if is_crypto {
                    let base = self.next_local as u8;
                    for arg in args { self.compile_expr(arg, ctx); }
                    let dst = base;
                    match method_name.as_str() {
                        "hash"   => self.emit(OpCode::CryptoHash { dst, pass_src: base, alg_src: base + 1 }, &expr.span),
                        "verify" => self.emit(OpCode::CryptoVerify { dst, pass_src: base, hash_src: base + 1, alg_src: base + 2 }, &expr.span),
                        "token"  => self.emit(OpCode::CryptoToken { dst, len_src: base }, &expr.span),
                        _ => {}
                    }
                    self.next_local = (base + 1) as usize;
                    return dst;
                }

                // Normal Method Call
                let base = self.next_local as u8;
                self.compile_expr(receiver, ctx);

                if method_name == "where" && args.len() == 1 {
                    if !matches!(args[0].kind, crate::parser::ast::ExprKind::Lambda { .. }) {
                        // Special: Table.where() with shorthand predicate
                        // Wrap the expression in a synthetic lambda: row -> <expression>
                        let flat_locals = self.convert_to_flat_locals();
                        let mut captures = Vec::new();
                        self.collect_captures(&args[0], &flat_locals, &mut captures);

                        let mut sub = FunctionCompiler::new(false, Some(flat_locals));
                        sub.is_table_lambda = true;
                        
                        // Register captures in sub-compiler so they get assigned to R1, R2, ...
                        for id in &captures {
                            sub.lookup_local(id);
                        }
                        
                        sub.next_local = 1 + captures.len(); // R0=row, R1..RK=captures
                        
                        let res = sub.compile_expr(&args[0], ctx);
                        sub.emit(OpCode::Return { src: res }, &args[0].span);
                        
                        let captures_to_pass = sub.captures.clone();
                        
                        let fid = ctx.functions.len();
                        ctx.functions.push(FunctionChunk {
                            bytecode: Arc::new(sub.bytecode),
                            spans: Arc::new(sub.spans),
                            is_fiber: false,
                            max_locals: sub.max_locals_used.max(sub.next_local),
                        });
                        
                        let f_val = Value::from_function(fid as u32);
                        let f_idx = ctx.add_constant(f_val);
                        let f_reg = self.push_reg();
                        self.emit(OpCode::LoadConst { dst: f_reg, idx: f_idx }, &args[0].span);
                        
                        // Push captured variables as additional arguments
                        for &cap_id in &captures_to_pass {
                            if let Some(slot) = self.lookup_local(&cap_id) {
                                let r = self.push_reg();
                                self.emit(OpCode::Move { dst: r, src: slot as u8 }, &args[0].span);
                            } else {
                                // This should not happen since we just identified them as parent locals
                                self.push_reg(); 
                            }
                        }
                        
                        let dst = base;
                        self.emit(OpCode::MethodCall { dst, kind: MethodKind::Where, base, arg_count: (1 + captures_to_pass.len()) as u8 }, &expr.span);
                        self.next_local = (base + 1) as usize;
                        return dst;
                    }
                }

                for arg in args { self.compile_expr(arg, ctx); }
                let dst = base;
                if let Some(kind) = self.map_method_kind(&method_name) {
                    self.emit(OpCode::MethodCall { dst, kind, base, arg_count: args.len() as u8 }, &expr.span);
                } else {
                    let mi = ctx.add_constant(Value::from_string(method_name.into()));
                    self.emit(OpCode::MethodCallCustom { dst, method_name_idx: mi, base, arg_count: args.len() as u8 }, &expr.span);
                }
                self.next_local = (base + 1) as usize;
                dst
            }
            crate::parser::ast::ExprKind::Binary { left, op, right } => {
                let src1 = self.compile_expr(left, ctx);
                let src2 = self.compile_expr(right, ctx);
                let dst = src1; // Reuse src1 as dst
                match op {
                    TokenKind::Plus => self.emit(OpCode::Add { dst, src1, src2 }, &expr.span),
                    TokenKind::Minus => self.emit(OpCode::Sub { dst, src1, src2 }, &expr.span),
                    TokenKind::Star => self.emit(OpCode::Mul { dst, src1, src2 }, &expr.span),
                    TokenKind::Slash => self.emit(OpCode::Div { dst, src1, src2 }, &expr.span),
                    TokenKind::Percent => self.emit(OpCode::Mod { dst, src1, src2 }, &expr.span),
                    TokenKind::Caret => self.emit(OpCode::Pow { dst, src1, src2 }, &expr.span),
                    TokenKind::EqualEqual => self.emit(OpCode::Equal { dst, src1, src2 }, &expr.span),
                    TokenKind::BangEqual => self.emit(OpCode::NotEqual { dst, src1, src2 }, &expr.span),
                    TokenKind::Greater => self.emit(OpCode::Greater { dst, src1, src2 }, &expr.span),
                    TokenKind::Less => self.emit(OpCode::Less { dst, src1, src2 }, &expr.span),
                    TokenKind::GreaterEqual => self.emit(OpCode::GreaterEqual { dst, src1, src2 }, &expr.span),
                    TokenKind::LessEqual => self.emit(OpCode::LessEqual { dst, src1, src2 }, &expr.span),
                    TokenKind::And => self.emit(OpCode::And { dst, src1, src2 }, &expr.span),
                    TokenKind::Or => self.emit(OpCode::Or { dst, src1, src2 }, &expr.span),
                    TokenKind::Has => self.emit(OpCode::Has { dst, src1, src2 }, &expr.span),
                    TokenKind::Union => self.emit(OpCode::SetUnion { dst, src1, src2 }, &expr.span),
                    TokenKind::Intersection => self.emit(OpCode::SetIntersection { dst, src1, src2 }, &expr.span),
                    TokenKind::Difference => self.emit(OpCode::SetDifference { dst, src1, src2 }, &expr.span),
                    TokenKind::SymDifference => self.emit(OpCode::SetSymDifference { dst, src1, src2 }, &expr.span),
                    TokenKind::PlusPlus => self.emit(OpCode::IntConcat { dst, src1, src2 }, &expr.span),
                    _ => {}
                }
                self.next_local = (dst + 1) as usize;
                dst
            }
            crate::parser::ast::ExprKind::Unary { op, right } => {
                match op {
                    TokenKind::Not | TokenKind::Bang => {
                        let src = self.compile_expr(right, ctx);
                        let dst = src;
                        self.emit(OpCode::Not { dst, src }, &expr.span);
                        dst
                    }
                    TokenKind::Minus => {
                        let zero = if matches!(right.kind, crate::parser::ast::ExprKind::FloatLiteral(_)) {
                            ctx.add_constant(Value::from_f64(0.0))
                        } else {
                            ctx.add_constant(Value::from_i64(0))
                        };
                        let src1 = self.push_reg();
                        self.emit(OpCode::LoadConst { dst: src1, idx: zero }, &expr.span);
                        let src2 = self.compile_expr(right, ctx);
                        let dst = src1;
                        self.emit(OpCode::Sub { dst, src1, src2 }, &expr.span);
                        self.next_local = (dst + 1) as usize;
                        dst
                    }
                    _ => self.push_reg() // should not happen
                }
            }
            crate::parser::ast::ExprKind::Index { receiver, index } => {
                let base = self.next_local as u8;
                self.compile_expr(receiver, ctx);
                self.compile_expr(index, ctx);
                let dst = base;
                self.emit(OpCode::MethodCall { dst, kind: MethodKind::Get, base, arg_count: 1 }, &expr.span);
                self.next_local = (base + 1) as usize;
                dst
            }
            crate::parser::ast::ExprKind::Lambda { params, return_type: _, body } => {
                let mut sub = FunctionCompiler::new(false, None);
                for (i, (_, param_name)) in params.iter().enumerate() {
                    sub.define_local(*param_name, i);
                }
                sub.next_local = params.len();
                let res = sub.compile_expr(body, ctx);
                sub.emit(OpCode::Return { src: res }, &expr.span);
                
                let fid = ctx.functions.len();
                ctx.functions.push(FunctionChunk {
                    bytecode: Arc::new(sub.bytecode),
                    spans: Arc::new(sub.spans),
                    is_fiber: false,
                    max_locals: sub.max_locals_used.max(sub.next_local),
                });
                
                let f_val = Value::from_function(fid as u32);
                let f_idx = ctx.add_constant(f_val);
                let dst = self.push_reg();
                self.emit(OpCode::LoadConst { dst, idx: f_idx }, &expr.span);
                dst
            }
            crate::parser::ast::ExprKind::Tuple(exprs) => {
                let base = self.next_local as u8;
                for e in exprs { self.compile_expr(e, ctx); }
                let dst = base;
                self.emit(OpCode::ArrayInit { dst, base, count: exprs.len() as u32 }, &expr.span);
                self.next_local = (base + 1) as usize;
                dst
            }
            crate::parser::ast::ExprKind::ArrayOrSetLiteral { elements } => {
                let base = self.next_local as u8;
                for e in elements { self.compile_expr(e, ctx); }
                let dst = base;
                self.emit(OpCode::ArrayInit { dst, base, count: elements.len() as u32 }, &expr.span);
                self.next_local = (base + 1) as usize;
                dst
            }
            crate::parser::ast::ExprKind::TerminalCommand(cmd_id, arg) => {
                let cmd = ctx.interner.lookup(*cmd_id);
                if cmd == "exit" { self.emit(OpCode::TerminalExit, &expr.span); }
                else if cmd == "clear" { self.emit(OpCode::TerminalClear, &expr.span); }
                else if cmd == "run" {
                    if let Some(a) = arg {
                        let cmd_src = self.compile_expr(a, ctx);
                        let dst = self.push_reg();
                        self.emit(OpCode::TerminalRun { dst, cmd_src }, &expr.span);
                        self.pop_reg(); // pop cmd_src
                        return dst;
                    }
                }
                self.push_reg() // Return dummy

            }
            crate::parser::ast::ExprKind::MemberAccess { receiver, member } => {
                let base = self.next_local as u8;
                self.compile_expr(receiver, ctx);
                let member_name = ctx.interner.lookup(*member).to_string();
                let dst = base;
                if let Some(kind) = self.map_method_kind(&member_name) {
                    self.emit(OpCode::MethodCall { dst, kind, base, arg_count: 0 }, &expr.span);
                } else {
                    let mi = ctx.add_constant(Value::from_string(member_name.into()));
                    self.emit(OpCode::MethodCallCustom { dst, method_name_idx: mi, base, arg_count: 0 }, &expr.span);
                }
                self.next_local = (base + 1) as usize;
                dst
            }
            crate::parser::ast::ExprKind::TableLiteral { columns, rows } => {
                let base = self.next_local as u8;
                for row in rows { for val in row { self.compile_expr(val, ctx); } }
                let vm_cols = columns.iter().map(|c| crate::backend::vm::VMColumn {
                    name: ctx.interner.lookup(c.name).to_string(),
                    ty: c.ty.clone(),
                    is_auto: c.is_auto,
                }).collect();
                let skeleton = Value::from_table(Arc::new(RwLock::new(
                    crate::backend::vm::TableData { columns: vm_cols, rows: Vec::new() }
                )));
                let ci = ctx.add_constant(skeleton);
                let dst = base;
                self.emit(OpCode::TableInit { dst, skeleton_idx: ci, base, row_count: rows.len() as u32 }, &expr.span);
                self.next_local = (base + 1) as usize;
                dst
            }
            crate::parser::ast::ExprKind::RawBlock(id) => {
                let s = ctx.interner.lookup(*id).to_string();
                let i = ctx.add_constant(Value::from_string(s.into()));
                let dst = self.push_reg();
                self.emit(OpCode::LoadConst { dst, idx: i }, &expr.span);
                dst
            }
            crate::parser::ast::ExprKind::NetCall { method, url, body } => {
                let url_src = self.compile_expr(url, ctx);
                let body_src = if let Some(b) = body {
                    self.compile_expr(b, ctx)
                } else {
                    let f = ctx.add_constant(Value::from_bool(false));
                    let r = self.push_reg();
                    self.emit(OpCode::LoadConst { dst: r, idx: f }, &expr.span);
                    r
                };
                let method_idx = ctx.add_constant(Value::from_string(ctx.interner.lookup(*method).to_string().into()));
                let dst = url_src;
                self.emit(OpCode::HttpCall { dst, method_idx, url_src, body_src }, &expr.span);
                self.next_local = (dst + 1) as usize;
                dst
            }
            crate::parser::ast::ExprKind::NetRespond { status, body, headers } => {
                let status_src = self.compile_expr(status, ctx);
                let body_src   = self.compile_expr(body, ctx);
                let headers_src = if let Some(h) = headers {
                    self.compile_expr(h, ctx)
                } else {
                    let f = ctx.add_constant(Value::from_bool(false));
                    let r = self.push_reg();
                    self.emit(OpCode::LoadConst { dst: r, idx: f }, &expr.span);
                    r
                };
                self.emit(OpCode::HttpRespond { status_src, body_src, headers_src }, &expr.span);
                self.next_local = status_src as usize; // Cleanup temps
                self.push_reg() // Return dummy
            }
        }
    }



    pub fn compile_stmt(&mut self, stmt: &Stmt, ctx: &mut CompileContext) {
        match &stmt.kind {
            crate::parser::ast::StmtKind::VarDecl { ty, name, value, .. } => {
                let dst = if self.is_main && self.scopes.len() == 1 {
                    // Globals: Evaluate into a temp, then SetVar
                    let src = if let Some(val) = value {
                        self.compile_expr(val, ctx)
                    } else {
                        let default_val = self.get_default_value(ty, ctx);
                        let idx = ctx.add_constant(default_val);
                        let r = self.push_reg();
                        self.emit(OpCode::LoadConst { dst: r, idx }, &stmt.span);
                        r
                    };
                    let idx = *ctx.globals.get(name).expect("Global not registered");
                    self.emit(OpCode::SetVar { idx: idx as u32, src }, &stmt.span);
                    self.pop_reg();
                    return;
                } else {
                    let slot = self.push_reg() as usize;
                    self.define_local(*name, slot);
                    slot as u8
                };

                if let Some(val) = value {
                    let src = self.compile_expr(val, ctx);
                    if src != dst {
                        self.emit(OpCode::Move { dst, src }, &stmt.span);
                    }
                    self.next_local = (dst + 1) as usize; // Keep dst, pop src if it was temp
                } else {
                    let default_val = self.get_default_value(ty, ctx);
                    let idx = ctx.add_constant(default_val);
                    self.emit(OpCode::LoadConst { dst, idx }, &stmt.span);
                }
            }
            crate::parser::ast::StmtKind::Print(expr) => {
                let src = self.compile_expr(expr, ctx);
                self.emit(OpCode::Print { src }, &stmt.span);
                self.pop_reg();
            }
            crate::parser::ast::StmtKind::FunctionCallStmt { name, args } => {
                let base = self.next_local as u8;
                for arg in args { self.compile_expr(arg, ctx); }
                if let Some(&func_id) = ctx.func_indices.get(name) {
                    let dst = base; // discard result or store in base
                    self.emit(OpCode::Call { dst, func_idx: func_id as u32, base, arg_count: args.len() as u8 }, &stmt.span);
                }
                self.next_local = base as usize;
            }
            crate::parser::ast::StmtKind::Input(name) => {
                let dst = self.push_reg();
                self.emit(OpCode::Input { dst }, &stmt.span);
                if let Some(slot) = self.lookup_local(name) {
                    self.emit(OpCode::Move { dst: slot as u8, src: dst }, &stmt.span);
                } else if let Some(&global_idx) = ctx.globals.get(name) {
                    self.emit(OpCode::SetVar { idx: global_idx as u32, src: dst }, &stmt.span);
                }
                self.pop_reg();
            }
            crate::parser::ast::StmtKind::Assign { name, value } => {
                let mut optimized = false;
                if let crate::parser::ast::ExprKind::Binary { left, op, right } = &value.kind {
                    if *op == TokenKind::Plus {
                        let is_inc = match (&left.kind, &right.kind) {
                            (crate::parser::ast::ExprKind::Identifier(id), crate::parser::ast::ExprKind::IntLiteral(1)) if id == name => true,
                            (crate::parser::ast::ExprKind::IntLiteral(1), crate::parser::ast::ExprKind::Identifier(id)) if id == name => true,
                            _ => false,
                        };
                        if is_inc {
                            if let Some(slot) = self.lookup_local(name) {
                                self.emit(OpCode::IncLocal { reg: slot as u8 }, &stmt.span);
                                optimized = true;
                            } else if let Some(&global_idx) = ctx.globals.get(name) {
                                self.emit(OpCode::IncVar { idx: global_idx as u32 }, &stmt.span);
                                optimized = true;
                            }
                        }
                    }
                }
                if !optimized {
                    let src = self.compile_expr(value, ctx);
                    if let Some(slot) = self.lookup_local(name) {
                        self.emit(OpCode::Move { dst: slot as u8, src }, &stmt.span);
                    } else if let Some(&global_idx) = ctx.globals.get(name) {
                        self.emit(OpCode::SetVar { idx: global_idx as u32, src }, &stmt.span);
                    }
                    self.pop_reg();
                }
            }
            crate::parser::ast::StmtKind::If { condition, then_branch, else_ifs, else_branch } => {
                let mut end_jumps = Vec::new();
                let cond_reg = self.compile_expr(condition, ctx);
                let jmp_idx = self.bytecode.len();
                self.emit(OpCode::JumpIfFalse { src: cond_reg, target: 0 }, &stmt.span);
                self.pop_reg(); // pop condition result after test
                
                self.enter_scope();
                for s in then_branch { self.compile_stmt(s, ctx); }
                self.exit_scope();
                
                if !else_ifs.is_empty() || else_branch.is_some() {
                    end_jumps.push(self.bytecode.len());
                    self.emit(OpCode::Jump { target: 0 }, &stmt.span);
                }
                
                let jump_target = self.bytecode.len() as u32;
                if let OpCode::JumpIfFalse { ref mut target, .. } = self.bytecode[jmp_idx] {
                    *target = jump_target;
                }

                for (elif_cond, elif_branch) in else_ifs {
                    let elif_cond_reg = self.compile_expr(elif_cond, ctx);
                    let elif_jmp = self.bytecode.len();
                    self.emit(OpCode::JumpIfFalse { src: elif_cond_reg, target: 0 }, &stmt.span);
                    self.pop_reg();

                    self.enter_scope();
                    for s in elif_branch { self.compile_stmt(s, ctx); }
                    self.exit_scope();
                    
                    end_jumps.push(self.bytecode.len());
                    self.emit(OpCode::Jump { target: 0 }, &stmt.span);
                    
                    let elif_target = self.bytecode.len() as u32;
                    if let OpCode::JumpIfFalse { ref mut target, .. } = self.bytecode[elif_jmp] {
                        *target = elif_target;
                    }
                }
                
                if let Some(branch) = else_branch {
                    self.enter_scope();
                    for s in branch { self.compile_stmt(s, ctx); }
                    self.exit_scope();
                }
                
                let final_idx = self.bytecode.len() as u32;
                for idx in end_jumps {
                    if let OpCode::Jump { ref mut target } = self.bytecode[idx] {
                        *target = final_idx;
                    }
                }
            }
            crate::parser::ast::StmtKind::While { condition, body } => {
                let start_p = self.bytecode.len();
                self.loop_stack.push((start_p, Vec::new(), Vec::new(), None));
                
                let cond_reg = self.compile_expr(condition, ctx);
                let exit_jmp = self.bytecode.len();
                self.emit(OpCode::JumpIfFalse { src: cond_reg, target: 0 }, &stmt.span);
                self.pop_reg();
                
                self.enter_scope();
                for s in body { self.compile_stmt(s, ctx); }
                self.exit_scope();
                
                self.emit(OpCode::Jump { target: start_p as u32 }, &stmt.span);
                
                let exit_target = self.bytecode.len() as u32;
                if let OpCode::JumpIfFalse { ref mut target, .. } = self.bytecode[exit_jmp] {
                    *target = exit_target;
                }
                
                let (_, breaks, continues, _) = self.loop_stack.pop().unwrap();
                let end_label = self.bytecode.len() as u32;
                for b in breaks {
                    if let OpCode::Jump { ref mut target } = self.bytecode[b] { *target = end_label; }
                }
                for c in continues {
                    if let OpCode::Jump { ref mut target } = self.bytecode[c] { *target = start_p as u32; }
                }
            }
            crate::parser::ast::StmtKind::For { var_name, start, end, step, body, iter_type } => {
                match iter_type {
                    crate::parser::ast::ForIterType::Array => {
                        // 1. Compile the array expression (receiver)
                        let array_reg = self.compile_expr(start, ctx);  // receiver in array_reg

                        // 2. Reserve the argument slot (array_reg+1)
                        let arg_reg = self.push_reg();                  // must be array_reg+1
                        debug_assert_eq!(arg_reg, array_reg + 1, "Argument register not consecutive");

                        // 3. Get array size (for loop bound)
                        let size_reg = self.push_reg();
                        self.emit(OpCode::MethodCall { dst: size_reg, kind: MethodKind::Size, base: array_reg, arg_count: 0 }, &stmt.span);

                        // 4. Index register (starts at 0)
                        let zero_idx = ctx.add_constant(Value::from_i64(0));
                        let index_reg = self.push_reg();
                        self.emit(OpCode::LoadConst { dst: index_reg, idx: zero_idx }, &stmt.span);

                        // 5. Loop variable (the element)
                        let loop_var_reg = if let Some(s) = self.lookup_local(var_name) { s as u8 } else {
                            let s = self.push_reg();
                            self.define_local(*var_name, s as usize);
                            s
                        };

                        let start_label = self.bytecode.len();
                        self.enter_scope();
                        self.loop_stack.push((start_label, Vec::new(), Vec::new(), None));

                        let saved_next_local = self.next_local;

                        // Loop condition: index < size
                        let test_reg = self.push_reg();
                        self.emit(OpCode::Less { dst: test_reg, src1: index_reg, src2: size_reg }, &stmt.span);
                        let exit_jmp = self.bytecode.len();
                        self.emit(OpCode::JumpIfFalse { src: test_reg, target: 0 }, &stmt.span);
                        self.pop_reg(); // test_reg

                        // *** FIX: copy index to argument slot before calling .get ***
                        self.emit(OpCode::Move { dst: arg_reg, src: index_reg }, &stmt.span);
                        self.emit(OpCode::MethodCall { dst: loop_var_reg, kind: MethodKind::Get, base: array_reg, arg_count: 1 }, &stmt.span);

                        // Execute loop body
                        for s in body { self.compile_stmt(s, ctx); }

                        // Restore register stack after body
                        self.next_local = saved_next_local;

                        // Increment index and jump back
                        let cont_label = self.bytecode.len();
                        let len = self.bytecode.len();
                        if len > 0 {
                            if let OpCode::IncLocal { reg } = self.bytecode[len - 1] {
                                self.bytecode.pop();
                                self.emit(OpCode::IncLocalLoopNext { inc_reg: reg, reg: index_reg, limit_reg: size_reg, target: start_label as u32 }, &stmt.span);
                            } else {
                                self.emit(OpCode::IncLocal { reg: index_reg }, &stmt.span);
                                self.emit(OpCode::Jump { target: start_label as u32 }, &stmt.span);
                            }
                        }
                        

                        let end_label = self.bytecode.len() as u32;
                        if let OpCode::JumpIfFalse { ref mut target, .. } = self.bytecode[exit_jmp] {
                            *target = end_label;
                        }

                        let (_, breaks, continues, _) = self.loop_stack.pop().unwrap();
                        self.exit_scope();
                        for b in breaks {
                            if let OpCode::Jump { ref mut target } = self.bytecode[b] { *target = end_label; }
                        }
                        for c in continues {
                            if let OpCode::Jump { ref mut target } = self.bytecode[c] { *target = cont_label as u32; }
                        }

                        // Clean up – all registers up to array_reg are now unused
                        self.next_local = array_reg as usize;
                    }
                    crate::parser::ast::ForIterType::Set => {
                        // 1. Compile the set expression (receiver)
                        let set_reg = self.compile_expr(start, ctx);

                        // 2. Convert Set to Array using .values() method
                        let array_reg = self.push_reg();
                        self.emit(OpCode::MethodCall { dst: array_reg, kind: MethodKind::Values, base: set_reg, arg_count: 0 }, &stmt.span);

                        // 3. Delegate to Array iteration logic (re-using array_reg)
                        let arg_reg = self.push_reg();
                        let size_reg = self.push_reg();
                        self.emit(OpCode::MethodCall { dst: size_reg, kind: MethodKind::Size, base: array_reg, arg_count: 0 }, &stmt.span);

                        let zero_idx = ctx.add_constant(Value::from_i64(0));
                        let index_reg = self.push_reg();
                        self.emit(OpCode::LoadConst { dst: index_reg, idx: zero_idx }, &stmt.span);

                        let loop_var_reg = if let Some(s) = self.lookup_local(var_name) { s as u8 } else {
                            let s = self.push_reg();
                            self.define_local(*var_name, s as usize);
                            s
                        };

                        let start_label = self.bytecode.len();
                        self.enter_scope();
                        self.loop_stack.push((start_label, Vec::new(), Vec::new(), None));

                        let saved_next_local = self.next_local;

                        let test_reg = self.push_reg();
                        self.emit(OpCode::Less { dst: test_reg, src1: index_reg, src2: size_reg }, &stmt.span);
                        let exit_jmp = self.bytecode.len();
                        self.emit(OpCode::JumpIfFalse { src: test_reg, target: 0 }, &stmt.span);
                        self.pop_reg();

                        self.emit(OpCode::Move { dst: arg_reg, src: index_reg }, &stmt.span);
                        self.emit(OpCode::MethodCall { dst: loop_var_reg, kind: MethodKind::Get, base: array_reg, arg_count: 1 }, &stmt.span);

                        for s in body { self.compile_stmt(s, ctx); }

                        self.next_local = saved_next_local;

                        let cont_label = self.bytecode.len();
                        self.emit(OpCode::IncLocal { reg: index_reg }, &stmt.span);
                        self.emit(OpCode::Jump { target: start_label as u32 }, &stmt.span);

                        let end_label = self.bytecode.len() as u32;
                        if let OpCode::JumpIfFalse { ref mut target, .. } = self.bytecode[exit_jmp] {
                            *target = end_label;
                        }

                        let (_, breaks, continues, _) = self.loop_stack.pop().unwrap();
                        self.exit_scope();
                        for b in breaks {
                            if let OpCode::Jump { ref mut target } = self.bytecode[b] { *target = end_label; }
                        }
                        for c in continues {
                            if let OpCode::Jump { ref mut target } = self.bytecode[c] { *target = cont_label as u32; }
                        }

                        // Clean up – all registers up to set_reg are now unused
                        self.next_local = set_reg as usize;
                    }

                    crate::parser::ast::ForIterType::Range => {
                        let start_reg = self.compile_expr(start, ctx);
                        // ✅ NOWY KOD
                        let loop_var_reg = self.push_reg();  // ZAWSZE nowy rejestr!
                        self.define_local(*var_name, loop_var_reg as usize);
                        self.emit(OpCode::Move { dst: loop_var_reg, src: start_reg }, &stmt.span);
                        let limit_reg = self.compile_expr(end, ctx);
                        
                        let start_p = self.bytecode.len();
                        self.enter_scope();
                        self.loop_stack.push((start_p, Vec::new(), Vec::new(), None));
                        
                        // Save the register state before loop body to prevent register corruption
                        let saved_next_local = self.next_local;
                        
                        let test_reg = self.push_reg();
                        self.emit(OpCode::LessEqual { dst: test_reg, src1: loop_var_reg, src2: limit_reg }, &stmt.span);
                        let exit_jmp = self.bytecode.len();
                        self.emit(OpCode::JumpIfFalse { src: test_reg, target: 0 }, &stmt.span);
                        self.pop_reg();
                        
                        let body_p = self.bytecode.len();
                        for s in body { self.compile_stmt(s, ctx); }
                        
                        // Restore register state to prevent temp registers from corrupting loop variables
                        self.next_local = saved_next_local;
                        
                        let cont_label = self.bytecode.len();
                        if step.is_none() {
                            let len = self.bytecode.len();
                            let mut fused = false;
                            if len > 0 {
                                if let OpCode::IncVar { idx } = self.bytecode[len - 1] {
                                    self.bytecode.pop();
                                    self.spans.pop();
                                    self.emit(OpCode::IncVarLoopNext { g_idx: idx, reg: loop_var_reg, limit_reg, target: body_p as u32 }, &stmt.span);
                                    fused = true;
                                }
                            }
                            if !fused {
                                self.emit(OpCode::LoopNext { reg: loop_var_reg, limit_reg, target: body_p as u32 }, &stmt.span);
                            }
                        } else {
                            let step_reg = self.compile_expr(step.as_ref().unwrap(), ctx);
                            self.emit(OpCode::Add { dst: loop_var_reg, src1: loop_var_reg, src2: step_reg }, &stmt.span);
                            self.emit(OpCode::Jump { target: start_p as u32 }, &stmt.span);
                            self.pop_reg();
                        }
                        
                        let end_label = self.bytecode.len() as u32;
                        if let OpCode::JumpIfFalse { ref mut target, .. } = self.bytecode[exit_jmp] {
                            *target = end_label;
                        }
                        
                        let (_, breaks, continues, _) = self.loop_stack.pop().unwrap();
                        self.exit_scope();
                        for b in breaks {
                            if let OpCode::Jump { ref mut target } = self.bytecode[b] { *target = end_label; }
                        }
                        for c in continues {
                            if let OpCode::Jump { ref mut target } = self.bytecode[c] { *target = cont_label as u32; }
                        }
                        self.next_local = (loop_var_reg + 1) as usize;
                    }
                    crate::parser::ast::ForIterType::Fiber => {
                        let fiber_reg = self.compile_expr(start, ctx);
                        // ✅ NOWY KOD
                        let loop_var_reg = self.push_reg();  // ZAWSZE nowy rejestr!
                        self.define_local(*var_name, loop_var_reg as usize);
                        let start_label = self.bytecode.len();
                        self.enter_scope();
                        self.loop_stack.push((start_label, Vec::new(), Vec::new(), Some(fiber_reg as usize)));
                        
                        // Save the register state before loop body to prevent register corruption
                        let saved_next_local = self.next_local;
                        
                        let test_reg = self.push_reg();
                        self.emit(OpCode::MethodCall { dst: test_reg, kind: MethodKind::IsDone, base: fiber_reg, arg_count: 0 }, &stmt.span);
                        let exit_jmp = self.bytecode.len();
                        self.emit(OpCode::JumpIfTrue { src: test_reg, target: 0 }, &stmt.span);
                        self.pop_reg();
                        
                        self.emit(OpCode::MethodCall { dst: loop_var_reg, kind: MethodKind::Next, base: fiber_reg, arg_count: 0 }, &stmt.span);
                        for s in body { self.compile_stmt(s, ctx); }
                        
                        // Restore register state to prevent temp registers from corrupting loop variables
                        self.next_local = saved_next_local;
                        
                        let cont_label = self.bytecode.len();
                        self.emit(OpCode::Jump { target: start_label as u32 }, &stmt.span);
                        
                        let end_label = self.bytecode.len() as u32;
                        if let OpCode::JumpIfTrue { ref mut target, .. } = self.bytecode[exit_jmp] {
                            *target = end_label;
                        }
                        
                        let (_, breaks, continues, _) = self.loop_stack.pop().unwrap();
                        self.exit_scope();
                        for b in breaks {
                            if let OpCode::Jump { ref mut target } = self.bytecode[b] { *target = end_label; }
                        }
                        for c in continues {
                            if let OpCode::Jump { ref mut target } = self.bytecode[c] { *target = cont_label as u32; }
                        }
                        self.next_local = fiber_reg as usize;
                    }
                }
            }
            crate::parser::ast::StmtKind::Break => {
                if let Some(&(_, _, _, Some(fiber_reg_idx))) = self.loop_stack.last() {
                    self.emit(OpCode::MethodCall { dst: 0, kind: MethodKind::Close, base: fiber_reg_idx as u8, arg_count: 0 }, &stmt.span);
                }
                let jmp = self.bytecode.len();
                self.emit(OpCode::Jump { target: 0 }, &stmt.span);
                if let Some(l) = self.loop_stack.last_mut() { l.1.push(jmp); }
            }
            crate::parser::ast::StmtKind::Continue => {
                let jmp = self.bytecode.len();
                self.emit(OpCode::Jump { target: 0 }, &stmt.span);
                if let Some(l) = self.loop_stack.last_mut() { l.2.push(jmp); }
            }
            crate::parser::ast::StmtKind::ExprStmt(expr) => {
                self.compile_expr(expr, ctx);
                self.pop_reg();
            }
            crate::parser::ast::StmtKind::Halt { level, message } => {
                let src = self.compile_expr(message, ctx);
                match level {
                    crate::parser::ast::HaltLevel::Alert => self.emit(OpCode::HaltAlert { src }, &stmt.span),
                    crate::parser::ast::HaltLevel::Error => self.emit(OpCode::HaltError { src }, &stmt.span),
                    crate::parser::ast::HaltLevel::Fatal => self.emit(OpCode::HaltFatal { src }, &stmt.span),
                }
                self.pop_reg();
            }
            crate::parser::ast::StmtKind::Return(expr) => {
                if let Some(e) = expr {
                    let src = self.compile_expr(e, ctx);
                    self.emit(OpCode::Return { src }, &stmt.span);
                    self.pop_reg();
                } else {
                    self.emit(OpCode::ReturnVoid, &stmt.span);
                }
            }
            crate::parser::ast::StmtKind::FunctionDef { name, params, body, .. } => {
                let mut fc = FunctionCompiler::new(false, None);
                for (i, (_, pname)) in params.iter().enumerate() {
                    fc.define_local(*pname, i);
                }
                fc.next_local = params.len();
                for s in body { fc.compile_stmt(s, ctx); }
                if fc.bytecode.is_empty() || !matches!(fc.bytecode.last(), Some(OpCode::Return { .. }) | Some(OpCode::ReturnVoid)) {
                    fc.emit(OpCode::ReturnVoid, &stmt.span);
                }
                let chunk = FunctionChunk {
                    bytecode: Arc::new(fc.bytecode),
                    spans: Arc::new(fc.spans),
                    is_fiber: false,
                    max_locals: fc.max_locals_used.max(fc.next_local),
                };
                let fid = ctx.func_indices.get(name).copied().unwrap_or(0);
                ctx.functions[fid] = chunk;
            }
            crate::parser::ast::StmtKind::FiberDef { name, params, body, .. } => {
                let mut fc = FunctionCompiler::new(false, None);
                for (i, (_, pname)) in params.iter().enumerate() {
                    fc.define_local(*pname, i);
                }
                fc.next_local = params.len();
                for s in body { fc.compile_stmt(s, ctx); }
                if fc.bytecode.is_empty() || !matches!(fc.bytecode.last(), Some(OpCode::Return { .. }) | Some(OpCode::ReturnVoid)) {
                    fc.emit(OpCode::ReturnVoid, &stmt.span);
                }
                let chunk = FunctionChunk {
                    bytecode: Arc::new(fc.bytecode),
                    spans: Arc::new(fc.spans),
                    is_fiber: true,
                    max_locals: fc.max_locals_used.max(fc.next_local),
                };
                let fid = ctx.func_indices.get(name).copied().unwrap_or(0);
                ctx.functions[fid] = chunk;
            }
            crate::parser::ast::StmtKind::FiberDecl { name, fiber_name, args, .. } => {
                let base = self.next_local as u8;
                for arg in args { self.compile_expr(arg, ctx); }
                let f_idx = ctx.func_indices.get(fiber_name).copied().unwrap_or(0);
                let dst = if let Some(s) = self.lookup_local(name) { s as u8 } else {
                    let s = self.push_reg();
                    self.define_local(*name, s as usize);
                    s
                };
                self.emit(OpCode::FiberCreate { dst, func_idx: f_idx as u32, base, arg_count: args.len() as u8 }, &stmt.span);
                self.next_local = (dst + 1) as usize;
            }
            crate::parser::ast::StmtKind::JsonBind { json, path, target } => {
                let json_src = self.compile_expr(json, ctx);
                let path_src = self.compile_expr(path, ctx);
                if let Some(local_idx) = self.lookup_local(target) {
                    self.emit(OpCode::JsonBindLocal { dst: local_idx as u8, json_src, path_src }, &stmt.span);
                } else {
                    let idx = ctx.globals.get(target).copied().unwrap_or(0);
                    self.emit(OpCode::JsonBind { idx: idx as u32, json_src, path_src }, &stmt.span);
                }
                self.next_local = json_src as usize;
            }
            crate::parser::ast::StmtKind::JsonInject { json, mapping, table } => {
                let json_src = self.compile_expr(json, ctx);
                let mapping_src = self.compile_expr(mapping, ctx);
                if let Some(local_idx) = self.lookup_local(table) {
                    self.emit(OpCode::JsonInjectLocal { table_reg: local_idx as u8, json_src, mapping_src }, &stmt.span);
                } else {
                    let idx = ctx.globals.get(table).copied().unwrap_or(0);
                    self.emit(OpCode::JsonInject { table_idx: idx as u32, json_src, mapping_src }, &stmt.span);
                }
                self.next_local = json_src as usize;
            }
            crate::parser::ast::StmtKind::Yield(expr) => {
                let src = self.compile_expr(expr, ctx);
                self.emit(OpCode::Yield { src }, &stmt.span);
                self.pop_reg();
            }
            crate::parser::ast::StmtKind::YieldFrom(expr) => {
                let fiber_reg = self.compile_expr(expr, ctx);
                let start_label = self.bytecode.len();
                let test_reg = self.push_reg();
                self.emit(OpCode::MethodCall { dst: test_reg, kind: MethodKind::IsDone, base: fiber_reg, arg_count: 0 }, &stmt.span);
                let exit_jmp = self.bytecode.len();
                self.emit(OpCode::JumpIfTrue { src: test_reg, target: 0 }, &stmt.span);
                self.pop_reg();

                let val_reg = self.push_reg();
                self.emit(OpCode::MethodCall { dst: val_reg, kind: MethodKind::Next, base: fiber_reg, arg_count: 0 }, &stmt.span);
                
                // If it just finished (isDone is true), it was a Return, so skip the Yield
                let test_reg = self.push_reg();
                self.emit(OpCode::MethodCall { dst: test_reg, kind: MethodKind::IsDone, base: fiber_reg, arg_count: 0 }, &stmt.span);
                let skip_jmp = self.bytecode.len();
                self.emit(OpCode::JumpIfTrue { src: test_reg, target: 0 }, &stmt.span);
                self.pop_reg(); // test_reg

                self.emit(OpCode::Yield { src: val_reg }, &stmt.span);
                
                let skip_target = self.bytecode.len() as u32;
                if let OpCode::JumpIfTrue { ref mut target, .. } = self.bytecode[skip_jmp] {
                    *target = skip_target;
                }
                self.pop_reg(); // val_reg
                
                self.emit(OpCode::Jump { target: start_label as u32 }, &stmt.span);
                let end_label = self.bytecode.len() as u32;
                if let OpCode::JumpIfTrue { ref mut target, .. } = self.bytecode[exit_jmp] {
                    *target = end_label;
                }
                self.next_local = fiber_reg as usize;
            }

            crate::parser::ast::StmtKind::YieldVoid => {
                self.emit(OpCode::YieldVoid, &stmt.span);
            }
            crate::parser::ast::StmtKind::Wait(expr) => {
                let src = self.compile_expr(expr, ctx);
                self.emit(OpCode::Wait { src }, &stmt.span);
                self.pop_reg();
            }
            crate::parser::ast::StmtKind::NetRequestStmt { method, url, headers, body, timeout, target } => {
                let mut elements = Vec::new();
                elements.push((crate::parser::ast::Expr { kind: crate::parser::ast::ExprKind::StringLiteral(ctx.interner.intern("method")), span: crate::lexer::token::Span { line: 0, col: 0, len: 0 } }, *method.clone()));
                elements.push((crate::parser::ast::Expr { kind: crate::parser::ast::ExprKind::StringLiteral(ctx.interner.intern("url")), span: crate::lexer::token::Span { line: 0, col: 0, len: 0 } }, *url.clone()));
                if let Some(h) = headers {
                    elements.push((crate::parser::ast::Expr { kind: crate::parser::ast::ExprKind::StringLiteral(ctx.interner.intern("headers")), span: crate::lexer::token::Span { line: 0, col: 0, len: 0 } }, *h.clone()));
                }
                if let Some(b) = body {
                    elements.push((crate::parser::ast::Expr { kind: crate::parser::ast::ExprKind::StringLiteral(ctx.interner.intern("body")), span: crate::lexer::token::Span { line: 0, col: 0, len: 0 } }, *b.clone()));
                }
                if let Some(t) = timeout {
                    elements.push((crate::parser::ast::Expr { kind: crate::parser::ast::ExprKind::StringLiteral(ctx.interner.intern("timeout")), span: crate::lexer::token::Span { line: 0, col: 0, len: 0 } }, *t.clone()));
                }
                let map_expr = crate::parser::ast::Expr {
                    kind: crate::parser::ast::ExprKind::MapLiteral {
                        key_type: crate::parser::ast::Type::String,
                        value_type: crate::parser::ast::Type::Json,
                        elements,
                    },
                    span: crate::lexer::token::Span { line: 0, col: 0, len: 0 },
                };
                let arg_src = self.compile_expr(&map_expr, ctx);
                let dst = if let Some(slot) = self.lookup_local(target) { slot as u8 } else {
                    let s = self.next_local;
                    self.define_local(*target, s);
                    self.next_local += 1;
                    s as u8
                };
                self.emit(OpCode::HttpRequest { dst, arg_src }, &stmt.span);
                self.next_local = (dst + 1) as usize;
            }
            crate::parser::ast::StmtKind::Serve { name, port, host, workers, routes } => {
                let port_src    = self.compile_expr(port, ctx);
                let host_src    = if let Some(h) = host { self.compile_expr(h, ctx) } 
                                  else { let i = ctx.add_constant(Value::from_bool(false)); let r = self.push_reg(); self.emit(OpCode::LoadConst { dst: r, idx: i }, &stmt.span); r };
                let workers_src = if let Some(w) = workers { self.compile_expr(w, ctx) } 
                                  else { let i = ctx.add_constant(Value::from_bool(false)); let r = self.push_reg(); self.emit(OpCode::LoadConst { dst: r, idx: i }, &stmt.span); r };
                let routes_src  = self.compile_expr(routes, ctx);
                
                let func_idx = ctx.func_indices.get(name).copied().unwrap_or(0);
                self.emit(OpCode::HttpServe { func_idx: func_idx as u32, port_src, host_src, workers_src, routes_src }, &stmt.span);
                self.next_local = port_src as usize;
            }
            crate::parser::ast::StmtKind::Include { .. } => {
                // Handled in pre-processor
            }
        }
    }
}

fn register_globals_recursive(
    stmts: &[Stmt],
    globals: &mut std::collections::HashMap<StringId, usize>,
    func_indices: &mut std::collections::HashMap<StringId, usize>,
    functions: &mut Vec<FunctionChunk>,
    is_main_script: bool,
) {
    for stmt in stmts {
        match &stmt.kind {
            crate::parser::ast::StmtKind::FunctionDef { name, body, .. } => {
                let idx = functions.len();
                func_indices.insert(*name, idx);
                functions.push(FunctionChunk {
                    bytecode: Arc::new(Vec::new()),
                    spans: Arc::new(Vec::new()),
                    is_fiber: false,
                    max_locals: 0,
                });
                register_globals_recursive(body, globals, func_indices, functions, false);
            }
            crate::parser::ast::StmtKind::FiberDef { name, body, .. } => {
                let idx = functions.len();
                func_indices.insert(*name, idx);
                functions.push(FunctionChunk {
                    bytecode: Arc::new(Vec::new()),
                    spans: Arc::new(Vec::new()),
                    is_fiber: true,
                    max_locals: 0,
                });
                register_globals_recursive(body, globals, func_indices, functions, false);
            }
            crate::parser::ast::StmtKind::VarDecl { name, .. } if is_main_script => {
                if !globals.contains_key(name) {
                    let idx = globals.len();
                    globals.insert(*name, idx);
                }
            }
            crate::parser::ast::StmtKind::FiberDecl { name, .. } if is_main_script => {
                if !globals.contains_key(name) {
                    let idx = globals.len();
                    globals.insert(*name, idx);
                }
            }
            crate::parser::ast::StmtKind::If { then_branch, else_ifs, else_branch, .. } => {
                register_globals_recursive(then_branch, globals, func_indices, functions, false);
                for (_, elif_branch) in else_ifs {
                    register_globals_recursive(elif_branch, globals, func_indices, functions, false);
                }
                if let Some(eb) = else_branch {
                    register_globals_recursive(eb, globals, func_indices, functions, false);
                }
            }
            crate::parser::ast::StmtKind::While { body, .. } => {
                register_globals_recursive(body, globals, func_indices, functions, false);
            }
            crate::parser::ast::StmtKind::For { body, .. } => {
                register_globals_recursive(body, globals, func_indices, functions, false);
            }
            _ => {}
        }
    }
}

impl Compiler {
    #[allow(dead_code)]
    pub fn get_global_idx(&self, name: StringId) -> usize {
        *self.globals.get(&name).expect("Global not found")
    }

    pub fn new() -> Self {
        Self {
            globals: HashMap::new(),
            func_indices: HashMap::new(),
            functions: Vec::new(),
            constants: Vec::new(),
            string_constants: HashMap::new(),
        }
    }

    pub fn compile(&mut self, program: &crate::parser::ast::Program, interner: &mut Interner) -> (FunctionChunk, Arc<Vec<Value>>, Arc<Vec<FunctionChunk>>) {
        let built_ins = ["json", "date", "random", "store"];
        for (i, name) in built_ins.iter().enumerate() {
            let id = interner.intern(name);
            self.globals.insert(id, i);
        }
        register_globals_recursive(&program.stmts, &mut self.globals, &mut self.func_indices, &mut self.functions, true);
        let mut ctx = CompileContext {
            constants: &mut self.constants,
            string_constants: &mut self.string_constants,
            functions: &mut self.functions,
            func_indices: &self.func_indices,
            globals: &self.globals,
            interner,
        };
        let mut main_compiler = FunctionCompiler::new(true, None);
        let dummy_span = crate::lexer::token::Span { line: 0, col: 0, len: 0 };
        for (i, name) in ["json", "date", "random", "store"].iter().enumerate() {
            let val = ctx.add_constant(Value::from_string(name.to_string().into()));
            let dst = main_compiler.push_reg();
            main_compiler.emit(OpCode::LoadConst { dst, idx: val }, &dummy_span);
            main_compiler.emit(OpCode::SetVar { idx: i as u32, src: dst }, &dummy_span);
            main_compiler.pop_reg();
        }
        for stmt in &program.stmts {
            match &stmt.kind {
                crate::parser::ast::StmtKind::FunctionDef { name, params, body, .. } => {
                    let fid = *self.func_indices.get(name).unwrap();
                    let chunk = compile_function_helper(params, body, false, &mut ctx);
                    ctx.functions[fid] = chunk;
                }
                crate::parser::ast::StmtKind::FiberDef { name, params, body, .. } => {
                    let fid = *self.func_indices.get(name).unwrap();
                    let chunk = compile_function_helper(params, body, true, &mut ctx);
                    ctx.functions[fid] = chunk;
                }
                _ => main_compiler.compile_stmt(stmt, &mut ctx),
            }
        }
        main_compiler.emit(OpCode::Halt, &dummy_span);
        let main_chunk = FunctionChunk {
            bytecode: Arc::new(main_compiler.bytecode),
            spans: Arc::new(main_compiler.spans),
            is_fiber: false,
            max_locals: main_compiler.max_locals_used.max(main_compiler.next_local),
        };
        (main_chunk, Arc::new(std::mem::take(&mut self.constants)), Arc::new(std::mem::take(&mut self.functions)))
    }
}

fn compile_function_helper(
    params: &[(crate::parser::ast::Type, StringId)],
    body: &[Stmt],
    is_fiber: bool,
    ctx: &mut CompileContext,
) -> FunctionChunk {
    let mut compiler = FunctionCompiler::new(false, None);
    for (i, (_, param_name)) in params.iter().enumerate() {
        compiler.define_local(*param_name, i);
    }
    compiler.next_local = params.len();
    for s in body {
        compiler.compile_stmt(s, ctx);
    }
    if !compiler.bytecode.last().map_or(false, |op| {
        matches!(op, OpCode::Return { .. } | OpCode::ReturnVoid)
    }) {
        let dummy_span = crate::lexer::token::Span { line: 0, col: 0, len: 0 };
        compiler.emit(OpCode::ReturnVoid, &dummy_span);
    }
    FunctionChunk {
        bytecode: Arc::new(compiler.bytecode),
        spans: Arc::new(compiler.spans),
        is_fiber,
        max_locals: compiler.max_locals_used.max(compiler.next_local),
    }
}