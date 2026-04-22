//symbol_table.rs
use std::collections::{HashMap, HashSet};
use crate::parser::ast::Type;

#[derive(Clone)]
/// Hierarchical symbol table for tracking variable types and constant status.
/// Supports nested scopes and parent-linkage for cross-context resolution
/// during semantic analysis and code generation.
pub struct SymbolTable<'a> {
    parent: Option<&'a SymbolTable<'a>>,
    scopes: Vec<HashMap<String, Type>>,
    consts: Vec<HashSet<String>>,
}

impl<'a> SymbolTable<'a> {
    pub fn new() -> Self {
        Self {
            parent: None,
            scopes: vec![HashMap::new()],
            consts: vec![HashSet::new()],
        }
    }

    pub fn new_with_parent(parent: &'a SymbolTable<'a>) -> Self {
        Self {
            parent: Some(parent),
            scopes: vec![HashMap::new()],
            consts: vec![HashSet::new()],
        }
    }

    pub fn enter_scope(&mut self) {
        self.scopes.push(HashMap::new());
        self.consts.push(HashSet::new());
    }

    pub fn exit_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
            self.consts.pop();
        }
    }

    pub fn has(&self, name: &str) -> bool {
        let name = name.trim();
        let mut curr = Some(self);
        while let Some(st) = curr {
            for scope in st.scopes.iter().rev() {
                if scope.contains_key(name) {
                    return true;
                }
            }
            curr = st.parent;
        }
        false
    }

    pub fn has_in_current_scope(&self, name: &str) -> bool {
        let name = name.trim();
        self.scopes.last().map(|s| s.contains_key(name)).unwrap_or(false)
    }

    pub fn define(&mut self, name: String, ty: Type, is_const: bool) {
        let name = name.trim().to_string();
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.clone(), ty);
        }
        if is_const {
            if let Some(c_scope) = self.consts.last_mut() {
                c_scope.insert(name);
            }
        }
    }

    pub fn lookup(&self, name: &str) -> Option<Type> {
        let name = name.trim();
        let mut curr = Some(self);
        while let Some(st) = curr {
            for scope in st.scopes.iter().rev() {
                if let Some(ty) = scope.get(name) {
                    return Some(ty.clone());
                }
            }
            curr = st.parent;
        }
        None
    }
    
    pub fn is_const(&self, name: &str) -> bool {
        let name = name.trim();
        let mut curr = Some(self);
        while let Some(st) = curr {
            for (i, scope) in st.scopes.iter().enumerate().rev() {
                if scope.contains_key(name) {
                    return st.consts[i].contains(name);
                }
            }
            curr = st.parent;
        }
        false
    }

    pub fn _copy_globals(&self) -> Self {
        let mut curr = self;
        while let Some(p) = curr.parent {
            curr = p;
        }
        Self {
            parent: None,
            scopes: vec![curr.scopes[0].clone()],
            consts: vec![curr.consts[0].clone()],
        }
    }
}
