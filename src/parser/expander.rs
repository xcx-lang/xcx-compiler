use crate::parser::ast::{Program, Stmt, StmtKind, Expr, ExprKind};
use crate::sema::interner::{Interner, StringId};
use std::path::{Path, PathBuf};
use std::collections::{HashSet, HashMap};

pub struct Expander<'a> {
    interner: &'a mut Interner,
    visiting_files: HashSet<PathBuf>,
    included_files: HashSet<PathBuf>,
    aliases: HashMap<StringId, String>,
    include_paths: Vec<PathBuf>,
}

impl<'a> Expander<'a> {
    pub fn new(interner: &'a mut Interner) -> Self {
        Self {
            interner,
            visiting_files: HashSet::new(),
            included_files: HashSet::new(),
            aliases: HashMap::new(),
            include_paths: Vec::new(),
        }
    }

    pub fn add_include_path(&mut self, path: PathBuf) {
        self.include_paths.push(path);
    }

    pub fn expand(&mut self, program: Program, current_dir: &Path) -> Result<Program, String> {
        let mut stmts = Vec::new();
        for stmt in program.stmts {
            match stmt.kind {
                StmtKind::Include { path, alias } => {
                    let path_str = self.interner.lookup(path).to_string();
                    
                    let mut resolved_path = None;
                    
                    // 1. Try relative to current_dir
                    let full_path = current_dir.join(&path_str);
                    if let Ok(cp) = full_path.canonicalize() {
                        resolved_path = Some(cp);
                    }
                    
                    // 2. Try in include_paths if not found and not absolute
                    if resolved_path.is_none() && !Path::new(&path_str).is_absolute() {
                        for include_path in &self.include_paths {
                            let trial = include_path.join(&path_str);
                            if let Ok(cp) = trial.canonicalize() {
                                resolved_path = Some(cp);
                                break;
                            }
                        }
                    }

                    let canonical_path = resolved_path.ok_or_else(|| {
                        format!("File not found: {} (tried: {} and library paths)", path_str, full_path.display())
                    })?;

                    if self.visiting_files.contains(&canonical_path) {
                        return Err(format!("Circular dependency detected: {}", path_str));
                    }

                    if alias.is_none() && self.included_files.contains(&canonical_path) {
                        continue;
                    }

                    if alias.is_none() {
                        self.included_files.insert(canonical_path.clone());
                    }

                    self.visiting_files.insert(canonical_path.clone());

                    let source = std::fs::read_to_string(&canonical_path).map_err(|_| format!("Could not read file: {}", path_str))?;
                    let scanner = crate::lexer::scanner::Scanner::new(&source);
                    let mut parser = crate::parser::pratt::Parser::new_with_interner(&source, scanner, (*self.interner).clone());
                    let sub_program = parser.parse_program();
                    *self.interner = parser.into_interner();

                    let new_current_dir = canonical_path.parent().unwrap_or(Path::new("."));
                    
                    if let Some(alias_id) = alias {
                        let alias_str = self.interner.lookup(alias_id).to_string();
                        self.aliases.insert(alias_id, alias_str.clone());
                        let mut expanded_sub = self.expand(sub_program, new_current_dir)?;
                        self.prefix_program(&mut expanded_sub, &alias_str);
                        stmts.extend(expanded_sub.stmts);
                    } else {
                        let expanded_sub = self.expand(sub_program, new_current_dir)?;
                        stmts.extend(expanded_sub.stmts);
                    }

                    self.visiting_files.remove(&canonical_path);
                }
                _ => {
                    stmts.push(self.expand_stmt(stmt, current_dir)?);
                }
            }
        }
        Ok(Program { stmts })
    }

    fn expand_stmt(&mut self, mut stmt: Stmt, _current_dir: &Path) -> Result<Stmt, String> {
        self.expand_stmt_inplace(&mut stmt);
        Ok(stmt)
    }

    fn expand_stmt_inplace(&mut self, stmt: &mut Stmt) {
        match &mut stmt.kind {
            StmtKind::VarDecl { value, .. } => { if let Some(v) = value { self.expand_expr_inplace(v); } }
            StmtKind::Assign { value, .. } => self.expand_expr_inplace(value),
            StmtKind::FunctionDef { body, .. } => { for s in body.iter_mut() { self.expand_stmt_inplace(s); } }
            StmtKind::FiberDef { body, .. } => { for s in body.iter_mut() { self.expand_stmt_inplace(s); } }
            StmtKind::Print(e) | StmtKind::TerminalWrite(e) | StmtKind::ExprStmt(e) | StmtKind::Return(Some(e)) | StmtKind::Yield(e) => self.expand_expr_inplace(e),
            StmtKind::FiberDecl { args, .. } => {
                for a in args.iter_mut() { self.expand_expr_inplace(a.expr_mut()); }
            }
            StmtKind::YieldFrom(e) => self.expand_expr_inplace(e),
            StmtKind::If { condition, then_branch, else_ifs, else_branch } => {
                self.expand_expr_inplace(condition);
                for s in then_branch.iter_mut() { self.expand_stmt_inplace(s); }
                for (c, b) in else_ifs.iter_mut() { self.expand_expr_inplace(c); for s in b.iter_mut() { self.expand_stmt_inplace(s); } }
                if let Some(b) = else_branch { for s in b.iter_mut() { self.expand_stmt_inplace(s); } }
            }
            StmtKind::While { condition, body } => { self.expand_expr_inplace(condition); for s in body.iter_mut() { self.expand_stmt_inplace(s); } }
            StmtKind::For { start, end, step, body, .. } => {
               self.expand_expr_inplace(start); self.expand_expr_inplace(end);
               if let Some(s) = step { self.expand_expr_inplace(s); }
               for s in body.iter_mut() { self.expand_stmt_inplace(s); }
            }
            StmtKind::FunctionCallStmt { args, .. } => { for a in args.iter_mut() { self.expand_expr_inplace(a.expr_mut()); } }
            StmtKind::JsonBind { json, path, .. } => { self.expand_expr_inplace(json); self.expand_expr_inplace(path); }
            StmtKind::JsonInject { json, mapping, .. } => { self.expand_expr_inplace(json); self.expand_expr_inplace(mapping); }
            StmtKind::NetRequestStmt { method, url, headers, body, timeout, .. } => {
                self.expand_expr_inplace(method); self.expand_expr_inplace(url);
                if let Some(h) = headers { self.expand_expr_inplace(h); }
                if let Some(b) = body { self.expand_expr_inplace(b); }
                if let Some(t) = timeout { self.expand_expr_inplace(t); }
            }
            StmtKind::Serve { port, host, workers, routes, .. } => {
                self.expand_expr_inplace(port); 
                if let Some(h) = host { self.expand_expr_inplace(h); }
                if let Some(w) = workers { self.expand_expr_inplace(w); }
                self.expand_expr_inplace(routes);
            }
            StmtKind::Wait(e) | StmtKind::Halt { message: e, .. } => self.expand_expr_inplace(e),
            _ => {}
        }
    }

    fn expand_expr_inplace(&mut self, expr: &mut Expr) {
        match &mut expr.kind {
            ExprKind::Binary { left, right, .. } => { self.expand_expr_inplace(left); self.expand_expr_inplace(right); }
            ExprKind::Unary { right, .. } => self.expand_expr_inplace(right),
            ExprKind::FunctionCall { args, .. } => { for a in args.iter_mut() { self.expand_expr_inplace(a.expr_mut()); } }
            ExprKind::MethodCall { receiver, method, args, wait_after: _ } => {
                self.expand_expr_inplace(receiver);
                for a in args.iter_mut() { self.expand_expr_inplace(a.expr_mut()); }
                
                let mut rewrite = None;
                if let ExprKind::Identifier(id) = &receiver.kind {
                    let id_str = self.interner.lookup(*id).to_string();
                    let member_name = self.interner.lookup(*method).to_string();
                    
                    if let Some(prefix) = self.aliases.get(id) {
                        let new_id = self.interner.intern(&format!("{}.{}", prefix, member_name));
                        rewrite = Some(new_id);
                    } else if !id_str.contains('.') {
                        for prefix in self.aliases.values() {
                            if id_str == *prefix {
                                let new_id = self.interner.intern(&format!("{}.{}", prefix, member_name));
                                rewrite = Some(new_id);
                                break;
                            }
                        }
                    }
                }
                if let Some(new_id) = rewrite {
                    expr.kind = ExprKind::FunctionCall { name: new_id, args: std::mem::take(args) };
                }
            }
            ExprKind::Identifier(id) => {
                let id_str = self.interner.lookup(*id).to_string();
                if !id_str.contains('.') {
                    for prefix in self.aliases.values() {
                        if id_str == "PI" || id_str == "E" || id_str == "TAU" || id_str == "PHI" || 
                           id_str == "SQRT2" || id_str == "LN2" || id_str == "LN10" || 
                           id_str == "INF" || id_str == "INT_MAX" || id_str == "INT_MIN" {
                            let new_id_str = format!("{}.{}", prefix, id_str);
                            *id = self.interner.intern(&new_id_str);
                            break;
                        }
                    }
                }
            }
            ExprKind::MemberAccess { receiver, member } => {
                self.expand_expr_inplace(receiver);
                
                let mut rewrite = None;
                if let ExprKind::Identifier(id) = &receiver.kind {
                    let id_str = self.interner.lookup(*id).to_string();
                    let member_name = self.interner.lookup(*member).to_string();
                    
                    let mut should_prefix = self.aliases.contains_key(id);
                    if !should_prefix {
                        for prefix in self.aliases.values() {
                            if id_str == *prefix || id_str.starts_with(&format!("{}.", prefix)) {
                                should_prefix = true;
                                break;
                            }
                        }
                    }
                    
                    if id_str == "math" {
                        should_prefix = true;
                    }

                    if should_prefix {
                        let new_id = self.interner.intern(&format!("{}.{}", id_str, member_name));
                        rewrite = Some(new_id);
                    }
                }
                if let Some(new_id) = rewrite {
                    expr.kind = ExprKind::Identifier(new_id);
                }
            }
            ExprKind::Index { receiver, index } => { self.expand_expr_inplace(receiver); self.expand_expr_inplace(index); }
            ExprKind::ArrayLiteral { elements } | ExprKind::SetLiteral { elements, .. } | ExprKind::Tuple(elements) | ExprKind::ArrayOrSetLiteral { elements } => {
                for e in elements.iter_mut() { self.expand_expr_inplace(e); }
            }
            ExprKind::Lambda { body, .. } => self.expand_expr_inplace(body),
            ExprKind::TerminalCommand(_, args) => {
                for a in args { self.expand_expr_inplace(a); }
            }
            _ => {}
        }
    }

    fn prefix_program(&mut self, program: &mut Program, prefix: &str) {
        let mut top_level_names = HashSet::new();
        for stmt in &program.stmts {
            match &stmt.kind {
                StmtKind::VarDecl { name, .. } => { top_level_names.insert(*name); }
                StmtKind::FunctionDef { name, .. } => { top_level_names.insert(*name); }
                StmtKind::FiberDef { name, .. } => { top_level_names.insert(*name); }
                _ => {}
            }
        }

        for stmt in &mut program.stmts {
            self.prefix_stmt(stmt, prefix, &top_level_names);
        }
    }

    fn prefix_stmt(&mut self, stmt: &mut Stmt, prefix: &str, top_level_names: &HashSet<StringId>) {
        self.prefix_stmt_impl(stmt, prefix, top_level_names);
    }

    fn prefix_stmt_impl(&mut self, stmt: &mut Stmt, prefix: &str, top_level_names: &HashSet<StringId>) {
        match &mut stmt.kind {
            StmtKind::VarDecl { name, value, .. } => {
                if top_level_names.contains(name) {
                    *name = self.prefix_id(*name, prefix);
                }
                if let Some(val) = value {
                    self.prefix_expr_impl(val, prefix, top_level_names);
                }
            }
            StmtKind::Assign { name, value } => {
                if top_level_names.contains(name) {
                    *name = self.prefix_id(*name, prefix);
                }
                self.prefix_expr_impl(value, prefix, top_level_names);
            }
            StmtKind::FunctionDef { name, params, body, .. } => {
                if top_level_names.contains(name) {
                    *name = self.prefix_id(*name, prefix);
                }
                for s in body {
                    self.prefix_stmt_impl(s, prefix, top_level_names);
                }
                let _ = params;
            }
            StmtKind::FiberDef { name, params, body, .. } => {
                if top_level_names.contains(name) {
                    *name = self.prefix_id(*name, prefix);
                }
                for s in body {
                    self.prefix_stmt_impl(s, prefix, top_level_names);
                }
                let _ = params;
            }
            // prefix the fiber_name reference in FiberDecl so that
            // `fiber:json: f = bubble_sort_f(arr)` inside algorithms.xcx becomes
            // `fiber:json: f = algo.bubble_sort_f(arr)` after prefixing.
            // Without this, the FiberDef is renamed to algo.bubble_sort_f but
            // the FiberDecl still references the old bare name → checker error.
            StmtKind::FiberDecl { fiber_name, args, .. } => {
                if top_level_names.contains(fiber_name) {
                    *fiber_name = self.prefix_id(*fiber_name, prefix);
                }
                for arg in args.iter_mut() {
                    self.prefix_expr_impl(arg.expr_mut(), prefix, top_level_names);
                }
            }
            // prefix fiber constructor calls inside YieldFrom.
            // `yield from quick_sort_f(arr, 0, n-1)` is a YieldFrom(FunctionCall).
            // The FunctionCall name must be prefixed just like any other top-level reference.
            StmtKind::YieldFrom(expr) => {
                self.prefix_expr_impl(expr, prefix, top_level_names);
            }
            StmtKind::Print(expr) | StmtKind::TerminalWrite(expr) | StmtKind::ExprStmt(expr) | StmtKind::Return(Some(expr)) | StmtKind::Yield(expr) => {
                self.prefix_expr_impl(expr, prefix, top_level_names);
            }
            StmtKind::If { condition, then_branch, else_ifs, else_branch } => {
                self.prefix_expr_impl(condition, prefix, top_level_names);
                for s in then_branch { self.prefix_stmt_impl(s, prefix, top_level_names); }
                for (cond, branch) in else_ifs {
                    self.prefix_expr_impl(cond, prefix, top_level_names);
                    for s in branch { self.prefix_stmt_impl(s, prefix, top_level_names); }
                }
                if let Some(branch) = else_branch {
                    for s in branch { self.prefix_stmt_impl(s, prefix, top_level_names); }
                }
            }
            StmtKind::While { condition, body } => {
                self.prefix_expr_impl(condition, prefix, top_level_names);
                for s in body { self.prefix_stmt_impl(s, prefix, top_level_names); }
            }
            StmtKind::For { var_name, start, end, step, body, .. } => {
                if top_level_names.contains(var_name) {
                    *var_name = self.prefix_id(*var_name, prefix);
                }
                self.prefix_expr_impl(start, prefix, top_level_names);
                self.prefix_expr_impl(end, prefix, top_level_names);
                if let Some(s) = step { self.prefix_expr_impl(s, prefix, top_level_names); }
                for s in body { self.prefix_stmt_impl(s, prefix, top_level_names); }
            }
            StmtKind::FunctionCallStmt { name, args } => {
                if top_level_names.contains(name) {
                    *name = self.prefix_id(*name, prefix);
                }
                for arg in args { self.prefix_expr_impl(arg.expr_mut(), prefix, top_level_names); }
            }
            StmtKind::Halt { message, .. } => {
                self.prefix_expr_impl(message, prefix, top_level_names);
            }
            StmtKind::Input(id, _) => {
                if top_level_names.contains(id) {
                    *id = self.prefix_id(*id, prefix);
                }
            }
            StmtKind::JsonBind { json, path, target } => {
                self.prefix_expr_impl(json, prefix, top_level_names);
                self.prefix_expr_impl(path, prefix, top_level_names);
                let _ = target;
            }
            StmtKind::JsonInject { json, mapping, table } => {
                self.prefix_expr_impl(json, prefix, top_level_names);
                self.prefix_expr_impl(mapping, prefix, top_level_names);
                if top_level_names.contains(table) {
                    *table = self.prefix_id(*table, prefix);
                }
            }
            StmtKind::NetRequestStmt { method, url, headers, body, timeout, target } => {
                self.prefix_expr_impl(method, prefix, top_level_names);
                self.prefix_expr_impl(url, prefix, top_level_names);
                if let Some(h) = headers { self.prefix_expr_impl(h, prefix, top_level_names); }
                if let Some(b) = body { self.prefix_expr_impl(b, prefix, top_level_names); }
                if let Some(t) = timeout { self.prefix_expr_impl(t, prefix, top_level_names); }
                if top_level_names.contains(target) {
                    *target = self.prefix_id(*target, prefix);
                }
            }
            StmtKind::Serve { port, host, workers, routes, .. } => {
                self.prefix_expr_impl(port, prefix, top_level_names);
                if let Some(h) = host { self.prefix_expr_impl(h, prefix, top_level_names); }
                if let Some(w) = workers { self.prefix_expr_impl(w, prefix, top_level_names); }
                self.prefix_expr_impl(routes, prefix, top_level_names);
            }
            StmtKind::Wait(expr) => self.prefix_expr_impl(expr, prefix, top_level_names),
            _ => {}
        }
    }


    fn prefix_expr_impl(&mut self, expr: &mut Expr, prefix: &str, top_level_names: &HashSet<StringId>) {
        match &mut expr.kind {
            ExprKind::Identifier(id) => {
                if top_level_names.contains(id) {
                    *id = self.prefix_id(*id, prefix);
                }
            }
            ExprKind::Binary { left, right, .. } => {
                self.prefix_expr_impl(left, prefix, top_level_names);
                self.prefix_expr_impl(right, prefix, top_level_names);
            }
            ExprKind::Unary { right, .. } => {
                self.prefix_expr_impl(right, prefix, top_level_names);
            }
            ExprKind::FunctionCall { name, args } => {
                if top_level_names.contains(name) {
                    *name = self.prefix_id(*name, prefix);
                }
                for arg in args { self.prefix_expr_impl(arg.expr_mut(), prefix, top_level_names); }
            }
            ExprKind::MethodCall { receiver, method, args, .. } => {
                self.prefix_expr_impl(receiver, prefix, top_level_names);
                for arg in args { self.prefix_expr_impl(arg.expr_mut(), prefix, top_level_names); }
                let _ = method;
            }
            ExprKind::MemberAccess { receiver, member } => {
                self.prefix_expr_impl(receiver, prefix, top_level_names);
                let _ = member;
            }
            ExprKind::Index { receiver, index } => {
                self.prefix_expr_impl(receiver, prefix, top_level_names);
                self.prefix_expr_impl(index, prefix, top_level_names);
            }
            ExprKind::Lambda { params, body, .. } => {
                self.prefix_expr_impl(body, prefix, top_level_names);
                let _ = params;
            }
            ExprKind::Tuple(elements) => {
                for e in elements { self.prefix_expr_impl(e, prefix, top_level_names); }
            }
            ExprKind::ArrayLiteral { elements } | ExprKind::SetLiteral { elements, .. } => {
                for elem in elements { self.prefix_expr_impl(elem, prefix, top_level_names); }
            }
            ExprKind::RandomChoice { set } => {
                self.prefix_expr_impl(set, prefix, top_level_names);
            }
            ExprKind::TerminalCommand(_, args) => {
                for a in args { self.prefix_expr_impl(a, prefix, top_level_names); }
            }
            _ => {}
        }
    }

    fn prefix_id(&mut self, id: StringId, prefix: &str) -> StringId {
        let name = self.interner.lookup(id).to_string();

        match name.as_str() {
            "json" | "date" | "store" | "halt" | "terminal" | "net" | "env" | "crypto" | "EMPTY" | "math" | "random" => return id,
            _ => {}
        }
        match name.as_str() {
            "i" | "f" | "s" | "b" | "from" | "main" => return id,
            _ => {}
        }

        let prefix_with_dot = format!("{}.", prefix);
        if name.starts_with(&prefix_with_dot) {
            return id;
        }

        let prefixed = format!("{}.{}", prefix, name);
        self.interner.intern(&prefixed)
    }
}