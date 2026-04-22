use std::io::{self, Write};
use crate::lexer::scanner::Scanner;
use crate::parser::pratt::Parser;
use crate::sema::checker::Checker;
use crate::sema::symbol_table::SymbolTable;
use crate::backend::Compiler;
use crate::backend::vm::VM;

pub fn run_repl() {
    println!("XCX Interactive Mode (REPL)");
    println!("Type '!help' for assistance or '.terminal !exit;' to quit.");

    let vm = std::sync::Arc::new(VM::new());
    let mut symbols = SymbolTable::new();
    let mut compiler = Compiler::new();
    let mut interner = crate::sema::interner::Interner::new();

    loop {
        print!("xcx> ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        let bytes_read = match io::stdin().read_line(&mut input) {
            Ok(n) => n,
            Err(_) => {
                break;
            }
        };

        if bytes_read == 0 {
            break;
        }

        let input = input.trim();
        if input.is_empty() {
            continue;
        }

        // Intercept REPL-specific commands
        if input.starts_with('!') {
            match input {
                "!help" => {
                    print_help();
                    continue;
                }
                "!exit" => {
                    println!("Goodbye!");
                    break;
                }
                "!clear" => {
                    // ANSI escape sequence for clearing screen
                    print!("\x1B[2J\x1B[1;1H");
                    io::stdout().flush().unwrap();
                    continue;
                }
                _ => {
                    println!("Unknown REPL command: {}. Type '!help' for available commands.", input);
                    continue;
                }
            }
        }

        let scanner = Scanner::new(input);
        let mut parser = Parser::new_with_interner(input, scanner, interner);
        let mut program = parser.parse_program();
        interner = parser.into_interner();

        let mut checker = Checker::new(&interner);
        let errors = checker.check(&mut program, &mut symbols);

        if !errors.is_empty() {
            for err in errors {
                println!("Error: {}", err.kind.to_diagnostic_message());
            }
            continue;
        }

        let (main_chunk, constants, functions) = compiler.compile(&program, &mut interner);
        
        let ctx = crate::backend::vm::SharedContext {
            constants,
            functions,
        };
        vm.clone().run(main_chunk, ctx);
    }
}

fn print_help() {
    print!("{}", r#"
================================================================================
                               XCX HELP SYSTEM
================================================================================

REPL COMMANDS:
  !help          Show this help message
  !clear         Clear the terminal screen
  !exit          Exit the interactive mode

BASIC SYNTAX:
  type: name = value;       Declare a variable (e.g., i: age = 25;)
  const type: NAME = value; Declare a constant
  >! expression;            Print result to terminal (e.g., >! 2 + 2;)
  >? variable;              Wait for user input

DATA TYPES:
  i: Integer (48-bit)       f: Float (64-bit)
  s: String (UTF-8)         b: Boolean (true/false)
  date: Date (YYYY-MM-DD)   json: JSON Object
  array:T { ... }           set:D  { ... }
  map:K<->V { ... }         table: { columns=[...] rows=[...] }

BUILT-IN SERVICES:
  json.parse(s)             Parse string to JSON
  date.now()                Get current date
  date("2024-01-01")        Create date literal
  store.read(path)          Read file content
  net.get(url)              Perform HTTP GET request

ARITHMETIC & LOGIC:
  +, -, *, /, %, ^, ++      Operators
  ==, !=, >, <, >=, <=      Comparisons
  AND, OR, NOT, HAS         Logical operators

CONTROL FLOW:
  if (cond) then; ... end;
  while (cond) do; ... end;
  for i in start to end do; ... end;

HALT SYSTEM:
  halt.alert >! msg;        Warning (non-fatal)
  halt.error >! msg;        Logic error (stops frame)
  halt.fatal >! msg;        Critical error (terminates process)

Type any valid XCX statement followed by a semicolon to execute it.
================================================================================
"#);
}
