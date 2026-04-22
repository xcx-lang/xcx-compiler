mod lexer;
mod parser;
mod sema;
mod diagnostic;
mod backend;

use std::fs;
use std::env;
use std::sync::Arc;

use crate::parser::pratt::Parser;
use crate::sema::checker::Checker;
use crate::sema::symbol_table::SymbolTable;
use crate::backend::Compiler;
use crate::backend::vm::VM;
use crate::diagnostic::Reporter;

fn main() {
    crate::backend::vm::preserve_jit_helpers_dummy();
    ctrlc::set_handler(move || {
        crate::backend::vm::SHUTDOWN.store(true, std::sync::atomic::Ordering::SeqCst);
        println!("\n[XCX] Shutdown signal received. Cleaning up...");
    }).expect("Error setting Ctrl-C handler");

    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        crate::backend::repl::run_repl();
        return;
    }

    let first_arg = &args[1];
    if first_arg == "--help" || first_arg == "-h" || first_arg == "help" {
        println!("Usage:");
        println!("  xcx                Start REPL");
        println!("  xcx <file.xcx>     Run file");
        println!("  xcx --version      Show version");
        println!("  xcx --help         Show help");
        println!("\nInside REPL:");
        println!("  !help              Show REPL commands");
        return;
    }

    if first_arg == "--version" || first_arg == "version" {
        println!("xcx 3.0 ({}/{})", std::env::consts::OS, std::env::consts::ARCH);
        return;
    }

    if first_arg == "pax" {
        let mut pax_path = "lib/pax.xcx".to_string();
        
        if !std::path::Path::new(&pax_path).exists() {
             if let Ok(exe_path) = env::current_exe() {
                 let mut current = exe_path.parent();
                 while let Some(dir) = current {
                     let alt_path = dir.join("lib/pax.xcx");
                     if alt_path.exists() {
                         pax_path = alt_path.to_string_lossy().to_string();
                         break;
                     }
                     current = dir.parent();
                 }
             }
        }

        if !std::path::Path::new(&pax_path).exists() {
            eprintln!("PAX manager not found at {}. Please ensure it is installed in the lib directory.", pax_path);
            return;
        }
        run_file(&pax_path);
    } else {
        run_file(first_arg);
    }
}

fn run_file(filename: &str) {
    let source = match fs::read_to_string(filename) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Could not read file {}: {}", filename, e);
            return;
        }
    };

    let current_dir = std::path::Path::new(filename)
        .parent()
        .unwrap_or(std::path::Path::new("."));

    let start_time = std::time::Instant::now();
    let mut parser = Parser::new(&source);
    let program_raw = parser.parse_program();
    if parser.has_error {
        return;
    }
    let mut interner = parser.into_interner();

    let mut expander = crate::parser::expander::Expander::new(&mut interner);

    if let Ok(cwd) = std::env::current_dir() {
        let lib_path = cwd.join("lib");
        if lib_path.exists() {
            expander.add_include_path(lib_path);
        }
    }

    let mut program = match expander.expand(program_raw, current_dir) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Expansion error: {}", e);
            return;
        }
    };

    let mut checker = Checker::new(&interner);
    let mut symbols = SymbolTable::new();
    let errors = checker.check(&mut program, &mut symbols);

    if !errors.is_empty() {
        let reporter = Reporter::new(&source);
        for err in &errors {
            reporter.error(err.span.line, err.span.col, err.span.len, &err.kind.to_diagnostic_message());
        }
        let duration = start_time.elapsed();
        println!("\n[XCX] Semantic analysis failed in {:?}. Found {} error(s).", duration, errors.len());
        return;
    }

    let mut compiler = Compiler::new();
    let (main_chunk, constants, functions) = compiler.compile(&program, &mut interner);
    let duration = start_time.elapsed();
    println!("[XCX] Compiled successfully in {:?}.", duration);

    let ctx = crate::backend::vm::SharedContext {
        constants,
        functions,
    };

    let vm = Arc::new(VM::new());
    let vm2 = vm.clone();
    vm.run(main_chunk, ctx);
    if vm2.error_count.load(std::sync::atomic::Ordering::SeqCst) > 0 {
        std::process::exit(1);
    }
}