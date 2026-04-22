
#[cfg(test)]
mod tests {
    use crate::parser::pratt::Parser;
    use crate::sema::checker::Checker;
    use crate::sema::symbol_table::SymbolTable;
    use crate::backend::Compiler as XCXCompiler;
    use crate::backend::vm::{VM, Value, SharedContext};
    use std::sync::Arc;

    fn run(source: &str) -> Arc<VM> {
        let mut parser = Parser::new(source);
        let mut program = parser.parse_program();
        let mut interner = parser.into_interner();

        let mut checker = Checker::new(&interner);
        let mut symbols = SymbolTable::new();
        let errors = checker.check(&mut program, &mut symbols);
        assert!(
            errors.is_empty(),
            "Type-check errors in test source:\n{:?}",
            errors
        );

        let mut compiler = XCXCompiler::new();
        let (main_chunk, constants, functions) = compiler.compile(&program, &mut interner);

        let vm = Arc::new(VM::new());
        let ctx = SharedContext { constants, functions };
        vm.clone().run(main_chunk, ctx);
        vm
    }

    // Helper: assert a global is a specific integer
    fn assert_int(vm: &Arc<VM>, idx: usize, expected: i64, msg: &str) {
        match vm.get_global(idx) {
            Some(v) if v.is_int() => assert_eq!(v.as_i64(), expected, "{}", msg),
            other => panic!("{}: expected Int({}), got {:?}", msg, expected, other),
        }
    }

    // Helper: assert a global is a specific bool
    fn assert_bool(vm: &Arc<VM>, idx: usize, expected: bool, msg: &str) {
        match vm.get_global(idx) {
            Some(v) if v.is_bool() => assert_eq!(v.as_bool(), expected, "{}", msg),
            // false is also the default NaN-box value for uninitialised slots
            None if !expected => {}
            other => panic!("{}: expected Bool({}), got {:?}", msg, expected, other),
        }
    }

    // Helper: assert a global is a specific string
    fn assert_str(vm: &Arc<VM>, idx: usize, expected: &str, msg: &str) {
        match vm.get_global(idx) {
            Some(v) if v.is_ptr() => {
                let s_bytes = v.as_string();
                let s_str = String::from_utf8_lossy(&s_bytes);
                assert_eq!(s_str, expected, "{}", msg);
            }
            other => panic!("{}: expected Str({:?}), got {:?}", msg, expected, other),
        }
    }

    // Helper: assert a global is a float approximately equal to expected
    fn assert_float(vm: &Arc<VM>, idx: usize, expected: f64, msg: &str) {
        match vm.get_global(idx) {
            Some(v) if v.is_float() => assert!((v.as_f64() - expected).abs() < 1e-9, "{}: expected {}, got {}", msg, expected, v.as_f64()),
            other => panic!("{}: expected Float({}), got {:?}", msg, expected, other),
        }
    }

    // -------------------------------------------------------------------------
    // Sanity test — basic arithmetic (original smoke test, kept for reference).
    // -------------------------------------------------------------------------
    #[test]
    fn test_basic_arithmetic() {
        run("i: x = 10; i: y = 20; >! x + y;");
    }

    // -------------------------------------------------------------------------
    // REPL — Parser::new_with_interner was called without `source`.
    // -------------------------------------------------------------------------
    #[test]
    fn test_repl_parser_new_accepts_source() {
        let source = "i: a = 42;";
        let mut parser = Parser::new(source);
        let mut program = parser.parse_program();
        let interner = parser.into_interner();

        let mut checker = Checker::new(&interner);
        let mut symbols = SymbolTable::new();
        let errors = checker.check(&mut program, &mut symbols);
        assert!(errors.is_empty(), "Unexpected type errors: {:?}", errors);
    }

    // -------------------------------------------------------------------------
    // Regression: Ensure no debug prints in production.
    // -------------------------------------------------------------------------
    #[test]
    fn test_no_debug_print_on_method_call() {
        let source = r#"
            table: t = table {
                columns: [id :: i @auto, name :: s]
                rows: [("Alice"), ("Bob")]
            };
            i: n = t.count();
            >! n;
        "#;
        run(source); // must not panic
    }

    // -------------------------------------------------------------------------
    // Regression: Unary negation of a float literal crashed the VM.
    // -------------------------------------------------------------------------
    #[test]
    fn test_unary_negation_float_does_not_crash() {
        let source = "f: x = -3.14;";
        run(source);
    }

    #[test]
    fn test_unary_negation_int_still_works() {
        let source = "i: x = -7;";
        run(source);
    }

    #[test]
    fn test_unary_negation_float_value_is_correct() {
        let source = "f: result = -2.5;";

        let mut parser = Parser::new(source);
        let mut program = parser.parse_program();
        let mut interner = parser.into_interner();

        let mut checker = Checker::new(&interner);
        let mut symbols = SymbolTable::new();
        let errors = checker.check(&mut program, &mut symbols);
        assert!(errors.is_empty(), "{:?}", errors);

        let name_id = interner.intern("result");
        let mut compiler = XCXCompiler::new();
        let (main_chunk, constants, functions) = compiler.compile(&program, &mut interner);
        let global_idx = compiler.get_global_idx(name_id);

        let vm = Arc::new(VM::new());
        let ctx = SharedContext { constants, functions };
        vm.clone().run(main_chunk, ctx);

        assert_float(&vm, global_idx, -2.5, "Expected -2.5");
    }

    // -------------------------------------------------------------------------
    // Regression: halt.error did not stop the current frame.
    // -------------------------------------------------------------------------
    #[test]
    fn test_halt_error_stops_current_frame() {
        let source = "i: sentinel = 0;\nhalt.error >! \"stopping here\";\ni: sentinel = 99;";

        let mut parser = Parser::new(source);
        let mut program = parser.parse_program();
        let mut interner = parser.into_interner();

        let mut checker = Checker::new(&interner);
        let mut symbols = SymbolTable::new();
        let _ = checker.check(&mut program, &mut symbols);

        let name_id = interner.intern("sentinel");
        let mut compiler = XCXCompiler::new();
        let (main_chunk, constants, functions) = compiler.compile(&program, &mut interner);
        let global_idx = compiler.get_global_idx(name_id);

        let vm = Arc::new(VM::new());
        let ctx = SharedContext { constants, functions };
        vm.clone().run(main_chunk, ctx);

        match vm.get_global(global_idx) {
            Some(v) if v.is_int() => assert_eq!(v.as_i64(), 0, "halt.error failed to stop frame — sentinel was mutated to {}", v.as_i64()),
            Some(v) if v.is_bool() && !v.as_bool() => {} // uninitialised slot — halt stopped execution
            None => {} // halt stopped before global was written
            other => panic!("Unexpected value for sentinel: {:?}", other),
        }
    }

    // -------------------------------------------------------------------------
    // Regression: Globals Vec was fixed at 1024 entries.
    // -------------------------------------------------------------------------
    #[test]
    fn test_globals_exceed_1024() {
        let mut source = String::new();
        for i in 0..1030 {
            source.push_str(&format!("i: var{i} = {i};\n"));
        }
        source.push_str(">! var1029;");
        run(&source);
    }

    // -------------------------------------------------------------------------
    // Professional HTTP Tests: Client and SSRF
    // -------------------------------------------------------------------------

    #[test]
    fn test_http_client_local_server() {
        use std::thread;

        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let addr_str = server.server_addr().to_string();
        let port = addr_str.split(':').last().unwrap().parse::<u16>().unwrap();

        thread::spawn(move || {
            if let Ok(Some(request)) = server.recv_timeout(std::time::Duration::from_secs(5)) {
                let response = tiny_http::Response::from_string("{\"hello\":\"world\"}")
                    .with_status_code(200);
                let _ = request.respond(response);
            }
        });

        let source = format!(r#"
            i: success = 0;
            json: res = net.get("http://127.0.0.1:{}");
            if (res.ok) then;
                if (res.body.hello == "world") then;
                    success = 42;
                end;
            end;
        "#, port);

        let mut parser = Parser::new(&source);
        let mut program = parser.parse_program();
        let mut interner = parser.into_interner();
        let mut checker = Checker::new(&interner);
        let mut symbols = SymbolTable::new();
        let _ = checker.check(&mut program, &mut symbols);

        let success_id = interner.intern("success");
        let mut compiler = XCXCompiler::new();
        let (main_chunk, constants, functions) = compiler.compile(&program, &mut interner);
        let success_idx = compiler.get_global_idx(success_id);

        let vm = Arc::new(VM::new());
        let ctx = SharedContext { constants, functions };
        vm.clone().run(main_chunk, ctx);

        assert_int(&vm, success_idx, 42, "HTTP integration test failed");
    }

    #[test]
    fn test_ssrf_protection_link_local() {
        let source = r#"
            json: res = net.get("http://169.254.169.254/latest/meta-data/");
            s: err = res.error;
        "#;
        let mut parser = Parser::new(source);
        let mut program = parser.parse_program();
        let mut interner = parser.into_interner();
        let mut checker = Checker::new(&interner);
        let mut symbols = SymbolTable::new();
        let _ = checker.check(&mut program, &mut symbols);

        let err_id = interner.intern("err");
        let mut compiler = XCXCompiler::new();
        let (main_chunk, constants, functions) = compiler.compile(&program, &mut interner);
        let err_idx = compiler.get_global_idx(err_id);

        let vm = Arc::new(VM::new());
        let ctx = SharedContext { constants, functions };
        vm.clone().run(main_chunk, ctx);

        match vm.get_global(err_idx) {
            Some(v) if v.is_ptr() => {
                let s_bytes = v.as_string();
                let s_str = String::from_utf8_lossy(&s_bytes);
                assert!(s_str.contains("SSRF"), "Expected SSRF error string, got: {}", s_str);
            }
            other => panic!("SSRF protection test failed! Expected error string, got {:?}", other),
        }
    }

    // -------------------------------------------------------------------------
    // FAZA 1 Tests — String Methods
    // -------------------------------------------------------------------------

    #[test]
    fn test_string_starts_with_true() {
        let source = r#"b: result = "admin@xcx.pl".startsWith("admin");"#;
        let mut parser = Parser::new(source);
        let mut program = parser.parse_program();
        let mut interner = parser.into_interner();
        let mut checker = Checker::new(&interner);
        let mut symbols = SymbolTable::new();
        let _ = checker.check(&mut program, &mut symbols);
        let id = interner.intern("result");
        let mut compiler = XCXCompiler::new();
        let (main_chunk, constants, functions) = compiler.compile(&program, &mut interner);
        let idx = compiler.get_global_idx(id);
        let vm = Arc::new(VM::new());
        let ctx = SharedContext { constants, functions };
        vm.clone().run(main_chunk, ctx);
        assert_bool(&vm, idx, true, "startsWith(\"admin\") should be true");
    }

    #[test]
    fn test_string_starts_with_false() {
        let source = r#"b: result = "xcx@xcx.pl".startsWith("admin");"#;
        let mut parser = Parser::new(source);
        let mut program = parser.parse_program();
        let mut interner = parser.into_interner();
        let mut checker = Checker::new(&interner);
        let mut symbols = SymbolTable::new();
        let _ = checker.check(&mut program, &mut symbols);
        let id = interner.intern("result");
        let mut compiler = XCXCompiler::new();
        let (main_chunk, constants, functions) = compiler.compile(&program, &mut interner);
        let idx = compiler.get_global_idx(id);
        let vm = Arc::new(VM::new());
        let ctx = SharedContext { constants, functions };
        vm.clone().run(main_chunk, ctx);
        assert_bool(&vm, idx, false, "startsWith(\"admin\") should be false");
    }

    #[test]
    fn test_string_ends_with_true() {
        let source = r#"b: result = "main.xcx".endsWith(".xcx");"#;
        let mut parser = Parser::new(source);
        let mut program = parser.parse_program();
        let mut interner = parser.into_interner();
        let mut checker = Checker::new(&interner);
        let mut symbols = SymbolTable::new();
        let _ = checker.check(&mut program, &mut symbols);
        let id = interner.intern("result");
        let mut compiler = XCXCompiler::new();
        let (main_chunk, constants, functions) = compiler.compile(&program, &mut interner);
        let idx = compiler.get_global_idx(id);
        let vm = Arc::new(VM::new());
        let ctx = SharedContext { constants, functions };
        vm.clone().run(main_chunk, ctx);
        assert_bool(&vm, idx, true, "endsWith(\".xcx\") should be true");
    }

    #[test]
    fn test_string_ends_with_false() {
        let source = r#"b: result = "main.xcx".endsWith(".rs");"#;
        let mut parser = Parser::new(source);
        let mut program = parser.parse_program();
        let mut interner = parser.into_interner();
        let mut checker = Checker::new(&interner);
        let mut symbols = SymbolTable::new();
        let _ = checker.check(&mut program, &mut symbols);
        let id = interner.intern("result");
        let mut compiler = XCXCompiler::new();
        let (main_chunk, constants, functions) = compiler.compile(&program, &mut interner);
        let idx = compiler.get_global_idx(id);
        let vm = Arc::new(VM::new());
        let ctx = SharedContext { constants, functions };
        vm.clone().run(main_chunk, ctx);
        assert_bool(&vm, idx, false, "endsWith(\".rs\") should be false");
    }

    #[test]
    fn test_string_to_int_valid() {
        let source = r#"i: result = "42".toInt();"#;
        let mut parser = Parser::new(source);
        let mut program = parser.parse_program();
        let mut interner = parser.into_interner();
        let mut checker = Checker::new(&interner);
        let mut symbols = SymbolTable::new();
        let _ = checker.check(&mut program, &mut symbols);
        let id = interner.intern("result");
        let mut compiler = XCXCompiler::new();
        let (main_chunk, constants, functions) = compiler.compile(&program, &mut interner);
        let idx = compiler.get_global_idx(id);
        let vm = Arc::new(VM::new());
        let ctx = SharedContext { constants, functions };
        vm.clone().run(main_chunk, ctx);
        assert_int(&vm, idx, 42, ".toInt() should return 42");
    }

    #[test]
    fn test_string_to_float_valid() {
        let source = r#"f: result = "3.14".toFloat();"#;
        let mut parser = Parser::new(source);
        let mut program = parser.parse_program();
        let mut interner = parser.into_interner();
        let mut checker = Checker::new(&interner);
        let mut symbols = SymbolTable::new();
        let _ = checker.check(&mut program, &mut symbols);
        let id = interner.intern("result");
        let mut compiler = XCXCompiler::new();
        let (main_chunk, constants, functions) = compiler.compile(&program, &mut interner);
        let idx = compiler.get_global_idx(id);
        let vm = Arc::new(VM::new());
        let ctx = SharedContext { constants, functions };
        vm.clone().run(main_chunk, ctx);
        assert_float(&vm, idx, 3.14, ".toFloat() expected 3.14");
    }

    #[test]
    fn test_string_to_int_with_whitespace() {
        let source = r#"i: result = "  99  ".toInt();"#;
        let mut parser = Parser::new(source);
        let mut program = parser.parse_program();
        let mut interner = parser.into_interner();
        let mut checker = Checker::new(&interner);
        let mut symbols = SymbolTable::new();
        let _ = checker.check(&mut program, &mut symbols);
        let id = interner.intern("result");
        let mut compiler = XCXCompiler::new();
        let (main_chunk, constants, functions) = compiler.compile(&program, &mut interner);
        let idx = compiler.get_global_idx(id);
        let vm = Arc::new(VM::new());
        let ctx = SharedContext { constants, functions };
        vm.clone().run(main_chunk, ctx);
        assert_int(&vm, idx, 99, ".toInt() should handle whitespace");
    }

    // -------------------------------------------------------------------------
    // FAZA 2 Tests — Array Methods: sort() and reverse()
    // -------------------------------------------------------------------------

    #[test]
    fn test_array_sort_integers() {
        let source = r#"
            array:i: nums {5, 2, 8, 1, 9};
            nums.sort();
            i: first = nums.get(0);
            i: last  = nums.get(4);
        "#;
        let mut parser = Parser::new(source);
        let mut program = parser.parse_program();
        let mut interner = parser.into_interner();
        let mut checker = Checker::new(&interner);
        let mut symbols = SymbolTable::new();
        let _ = checker.check(&mut program, &mut symbols);
        let first_id = interner.intern("first");
        let last_id  = interner.intern("last");
        let mut compiler = XCXCompiler::new();
        let (main_chunk, constants, functions) = compiler.compile(&program, &mut interner);
        let first_idx = compiler.get_global_idx(first_id);
        let last_idx  = compiler.get_global_idx(last_id);
        let vm = Arc::new(VM::new());
        let ctx = SharedContext { constants, functions };
        vm.clone().run(main_chunk, ctx);
        assert_int(&vm, first_idx, 1, "After sort first element should be 1");
        assert_int(&vm, last_idx,  9, "After sort last element should be 9");
    }

    #[test]
    fn test_array_sort_strings() {
        let source = r#"
            array:s: words {"banana", "apple", "cherry"};
            words.sort();
            s: first = words.get(0);
        "#;
        let mut parser = Parser::new(source);
        let mut program = parser.parse_program();
        let mut interner = parser.into_interner();
        let mut checker = Checker::new(&interner);
        let mut symbols = SymbolTable::new();
        let _ = checker.check(&mut program, &mut symbols);
        let first_id = interner.intern("first");
        let mut compiler = XCXCompiler::new();
        let (main_chunk, constants, functions) = compiler.compile(&program, &mut interner);
        let first_idx = compiler.get_global_idx(first_id);
        let vm = Arc::new(VM::new());
        let ctx = SharedContext { constants, functions };
        vm.clone().run(main_chunk, ctx);
        assert_str(&vm, first_idx, "apple", "After sort first string should be 'apple'");
    }

    #[test]
    fn test_array_reverse_integers() {
        let source = r#"
            array:i: nums {1, 2, 3, 4, 5};
            nums.reverse();
            i: first = nums.get(0);
            i: last  = nums.get(4);
        "#;
        let mut parser = Parser::new(source);
        let mut program = parser.parse_program();
        let mut interner = parser.into_interner();
        let mut checker = Checker::new(&interner);
        let mut symbols = SymbolTable::new();
        let _ = checker.check(&mut program, &mut symbols);
        let first_id = interner.intern("first");
        let last_id  = interner.intern("last");
        let mut compiler = XCXCompiler::new();
        let (main_chunk, constants, functions) = compiler.compile(&program, &mut interner);
        let first_idx = compiler.get_global_idx(first_id);
        let last_idx  = compiler.get_global_idx(last_id);
        let vm = Arc::new(VM::new());
        let ctx = SharedContext { constants, functions };
        vm.clone().run(main_chunk, ctx);
        assert_int(&vm, first_idx, 5, "After reverse first should be 5");
        assert_int(&vm, last_idx,  1, "After reverse last should be 1");
    }

    #[test]
    fn test_array_sort_then_reverse() {
        let source = r#"
            array:i: nums {3, 1, 4, 1, 5, 9, 2, 6};
            nums.sort();
            nums.reverse();
            i: first = nums.get(0);
        "#;
        let mut parser = Parser::new(source);
        let mut program = parser.parse_program();
        let mut interner = parser.into_interner();
        let mut checker = Checker::new(&interner);
        let mut symbols = SymbolTable::new();
        let _ = checker.check(&mut program, &mut symbols);
        let first_id = interner.intern("first");
        let mut compiler = XCXCompiler::new();
        let (main_chunk, constants, functions) = compiler.compile(&program, &mut interner);
        let first_idx = compiler.get_global_idx(first_id);
        let vm = Arc::new(VM::new());
        let ctx = SharedContext { constants, functions };
        vm.clone().run(main_chunk, ctx);
        assert_int(&vm, first_idx, 9, "After sort+reverse first should be 9");
    }

    #[test]
    fn test_wait_ms() {
        let source = r#"
            @wait(10);
            b: result = true;
        "#;
        let mut parser = Parser::new(source);
        let mut program = parser.parse_program();
        let mut interner = parser.into_interner();
        let mut checker = Checker::new(&interner);
        let mut symbols = SymbolTable::new();
        let _ = checker.check(&mut program, &mut symbols);
        let id = interner.intern("result");
        let mut compiler = XCXCompiler::new();
        let (main_chunk, constants, functions) = compiler.compile(&program, &mut interner);
        let idx = compiler.get_global_idx(id);
        let vm = Arc::new(VM::new());
        let ctx = SharedContext { constants, functions };
        vm.clone().run(main_chunk, ctx);
        assert_bool(&vm, idx, true, "@wait(10) should execute and allow next stmt");
    }

    #[test]
    fn test_env_get() {
        unsafe { std::env::set_var("XCX_TEST_VAR", "hello_xcx"); }

        let source = r#"s: val = env.get("XCX_TEST_VAR");"#;
        let mut parser = Parser::new(source);
        let mut program = parser.parse_program();
        let mut interner = parser.into_interner();
        let mut checker = Checker::new(&interner);
        let mut symbols = SymbolTable::new();
        let _ = checker.check(&mut program, &mut symbols);
        let id = interner.intern("val");
        let mut compiler = XCXCompiler::new();
        let (main_chunk, constants, functions) = compiler.compile(&program, &mut interner);
        let idx = compiler.get_global_idx(id);
        let vm = Arc::new(VM::new());
        let ctx = SharedContext { constants, functions };
        vm.clone().run(main_chunk, ctx);
        assert_str(&vm, idx, "hello_xcx", "env.get should retrieve XCX_TEST_VAR");
    }

    #[test]
    fn test_crypto_bcrypt() {
        let source = r#"
            s: pass = "super-secret";
            s: hashed = crypto.hash(pass, "bcrypt");
            b: ok = crypto.verify(pass, hashed, "bcrypt");
            b: fail = crypto.verify("wrong", hashed, "bcrypt");
        "#;
        let mut parser = Parser::new(source);
        let mut program = parser.parse_program();
        let mut interner = parser.into_interner();
        let mut checker = Checker::new(&interner);
        let mut symbols = SymbolTable::new();
        let _ = checker.check(&mut program, &mut symbols);
        let ok_id = interner.intern("ok");
        let fail_id = interner.intern("fail");
        let mut compiler = XCXCompiler::new();
        let (main_chunk, constants, functions) = compiler.compile(&program, &mut interner);
        let ok_idx = compiler.get_global_idx(ok_id);
        let fail_idx = compiler.get_global_idx(fail_id);
        let vm = Arc::new(VM::new());
        let ctx = SharedContext { constants, functions };
        vm.clone().run(main_chunk, ctx);
        assert_bool(&vm, ok_idx,   true,  "bcrypt verify should be true for correct password");
        assert_bool(&vm, fail_idx, false, "bcrypt verify should be false for wrong password");
    }

    #[test]
    fn test_crypto_argon2() {
        let source = r#"
            s: pass = "argon-secret";
            s: hashed = crypto.hash(pass, "argon2");
            b: ok = crypto.verify(pass, hashed, "argon2");
        "#;
        let mut parser = Parser::new(source);
        let mut program = parser.parse_program();
        let mut interner = parser.into_interner();
        let mut checker = Checker::new(&interner);
        let mut symbols = SymbolTable::new();
        let _ = checker.check(&mut program, &mut symbols);
        let ok_id = interner.intern("ok");
        let mut compiler = XCXCompiler::new();
        let (main_chunk, constants, functions) = compiler.compile(&program, &mut interner);
        let ok_idx = compiler.get_global_idx(ok_id);
        let vm = Arc::new(VM::new());
        let ctx = SharedContext { constants, functions };
        vm.clone().run(main_chunk, ctx);
        assert_bool(&vm, ok_idx, true, "argon2 verify should be true for correct password");
    }

    #[test]
    fn test_crypto_token() {
        let source = r#"
            s: t1 = crypto.token(16);
            s: t2 = crypto.token(32);
            i: len1 = t1.length;
            i: len2 = t2.length;
        "#;
        let mut parser = Parser::new(source);
        let mut program = parser.parse_program();
        let mut interner = parser.into_interner();
        let mut checker = Checker::new(&interner);
        let mut symbols = SymbolTable::new();
        let _ = checker.check(&mut program, &mut symbols);
        let l1_id = interner.intern("len1");
        let l2_id = interner.intern("len2");
        let mut compiler = XCXCompiler::new();
        let (main_chunk, constants, functions) = compiler.compile(&program, &mut interner);
        let l1_idx = compiler.get_global_idx(l1_id);
        let l2_idx = compiler.get_global_idx(l2_id);
        let vm = Arc::new(VM::new());
        let ctx = SharedContext { constants, functions };
        vm.clone().run(main_chunk, ctx);
        assert_int(&vm, l1_idx, 16, "crypto.token(16) hex length should be 16");
        assert_int(&vm, l2_idx, 32, "crypto.token(32) hex length should be 32");
    }

    // -------------------------------------------------------------------------
    // Value size guarantee — NaN-boxing must fit in 8 bytes
    // -------------------------------------------------------------------------
    #[test]
    fn test_value_size_is_8_bytes() {
        assert_eq!(std::mem::size_of::<Value>(), 8, "Value must be exactly 8 bytes (NaN-boxed)");
    }

    #[test]
    fn test_jit_fibonacci() {
        let source = r#"
            func fib(i: n -> i) {
                if (n < 2) then; return n; end;
                return fib(n - 1) + fib(n - 2);
            };
            i: result = fib(10);
        "#;
        let mut parser = Parser::new(source);
        let mut program = parser.parse_program();
        let mut interner = parser.into_interner();
        let mut checker = Checker::new(&interner);
        let mut symbols = SymbolTable::new();
        let _ = checker.check(&mut program, &mut symbols);
        
        let id_name = interner.intern("result");
        let mut compiler = XCXCompiler::new();
        let (main_chunk, constants, functions) = compiler.compile(&program, &mut interner);
        let idx = compiler.get_global_idx(id_name);
        
        let vm = Arc::new(VM::new());
        let ctx = SharedContext { constants, functions };
        vm.clone().run(main_chunk, ctx);
        assert_int(&vm, idx, 55, "fib(10) should be 55");
    }

    #[test]
    fn test_jit_sieve() {
        let source = r#"
            set:N: primes {2,,100};
            for p in 2 to 10 do;
                if (primes.contains(p)) then;
                    for mult in (p * p) to 100 @step p do;
                        primes.remove(mult);
                    end;
                end;
            end;
            i: count = primes.size();
        "#;
        let mut parser = Parser::new(source);
        let mut program = parser.parse_program();
        let mut interner = parser.into_interner();
        let mut checker = Checker::new(&interner);
        let mut symbols = SymbolTable::new();
        let _ = checker.check(&mut program, &mut symbols);
        
        let id_count = interner.intern("count");
        let mut compiler = XCXCompiler::new();
        let (main_chunk, constants, functions) = compiler.compile(&program, &mut interner);
        let idx = compiler.get_global_idx(id_count);
        
        let vm = Arc::new(VM::new());
        let ctx = SharedContext { constants, functions };
        vm.clone().run(main_chunk, ctx);
        assert_int(&vm, idx, 25, "Primes up to 100 should be 25");
    }
}
