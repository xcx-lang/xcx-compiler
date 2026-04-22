#[cfg(test)]
mod tests {
    use crate::parser::pratt::Parser;
    use crate::parser::ast::Type;
    use crate::lexer::token::TokenKind;

    #[test]
    fn test_var_decl() {
        let source = "i: x = 10;";
        let mut parser = Parser::new(source);
        let program = parser.parse_program();

        assert_eq!(program.stmts.len(), 1);
        match &program.stmts[0].kind {
            crate::parser::ast::StmtKind::VarDecl { ty, name: _name, value: _value, .. } => {
                assert_eq!(ty, &Type::Int);
            }
            _ => panic!("Expected VarDecl"),
        }
    }

    #[test]
    fn test_expression_precedence() {
        let source = "1 + 2 * 3 ^ 4 ^ 5;";
        let mut parser = Parser::new(source);
        let program = parser.parse_program();

        assert_eq!(program.stmts.len(), 1);
        if let crate::parser::ast::StmtKind::ExprStmt(expr) = &program.stmts[0].kind {
            if let crate::parser::ast::ExprKind::Binary { op, .. } = &expr.kind {
                assert_eq!(op, &TokenKind::Plus);
            } else {
                panic!("Expected Binary Plus at top level");
            }
        } else {
            panic!("Expected ExprStmt");
        }
    }

    #[test]
    fn test_comparisons() {
        let source = "i: a = b == c;";
        let mut parser = Parser::new(source);
        let program = parser.parse_program();

        assert_eq!(program.stmts.len(), 1);
        if let crate::parser::ast::StmtKind::VarDecl { value: Some(expr), .. } = &program.stmts[0].kind {
            if let crate::parser::ast::ExprKind::Binary { op, .. } = &expr.kind {
                assert_eq!(op, &TokenKind::EqualEqual);
            } else {
                panic!("Expected Binary EqualEqual");
            }
        } else {
            panic!("Expected VarDecl with Some value");
        }
    }

    #[test]
    fn test_func_decl_with_return_in_params() {
        let source = "func add(i: a, i: b -> i) { return a + b; };";
        let mut parser = Parser::new(source);
        let program = parser.parse_program();

        assert_eq!(program.stmts.len(), 1);
        if let crate::parser::ast::StmtKind::FunctionDef { name, params, return_type, .. } = &program.stmts[0].kind {
            let mut interner = parser.into_interner();
            let add_id = interner.intern("add");
            assert_eq!(*name, add_id);
            assert_eq!(params.len(), 2);
            assert_eq!(return_type, &Some(Type::Int));
        } else {
            panic!("Expected FunctionDef");
        }
    }

    #[test]
    fn test_json_block_as_array_identity() {
        let source = "json: data <<< [] >>>; data.push(1);";
        let mut parser = Parser::new(source);
        let program = parser.parse_program();

        assert_eq!(program.stmts.len(), 2);
        
        let mut interner = parser.into_interner();

        // Verify Stmt 1: VarDecl with json.parse wrap
        if let crate::parser::ast::StmtKind::VarDecl { value: Some(expr), .. } = &program.stmts[0].kind {
            if let crate::parser::ast::ExprKind::MethodCall { method, .. } = &expr.kind {
                assert_eq!(*method, interner.intern("parse"));
            } else {
                panic!("Expected MethodCall (json.parse)");
            }
        }

        // Verify Stmt 2: MethodCallExpr (push)
        if let crate::parser::ast::StmtKind::ExprStmt(expr) = &program.stmts[1].kind {
             if let crate::parser::ast::ExprKind::MethodCall { method, .. } = &expr.kind {
                assert_eq!(*method, interner.intern("push"));
            } else {
                panic!("Expected MethodCall (push)");
            }
        }
    }

    #[test]
    fn test_nested_method_call_boundaries() {
        let source = "f.set(\"content\", store.read(p)); f.set(\"path\", p);";
        let mut parser = Parser::new(source);
        let program = parser.parse_program();

        assert_eq!(program.stmts.len(), 2);
        
        let mut interner = parser.into_interner();

        // Stmt 1: f.set("content", store.read(p))
        if let crate::parser::ast::StmtKind::ExprStmt(expr) = &program.stmts[0].kind {
            if let crate::parser::ast::ExprKind::MethodCall { method, args, .. } = &expr.kind {
                assert_eq!(*method, interner.intern("set"));
                assert_eq!(args.len(), 2);
            } else {
                panic!("Expected MethodCall (set)");
            }
        }

        // Stmt 2: f.set("path", p)
        if let crate::parser::ast::StmtKind::ExprStmt(expr) = &program.stmts[1].kind {
            if let crate::parser::ast::ExprKind::MethodCall { method, args, .. } = &expr.kind {
                assert_eq!(*method, interner.intern("set"));
                assert_eq!(args.len(), 2);
            } else {
                panic!("Expected MethodCall (set)");
            }
        }
    }
}
