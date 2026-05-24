pub struct Reporter<'a> {
    lines: Vec<&'a str>,
    filename: Option<String>,
}

const ANSI_RED: &str = "\x1b[31;1m";
const ANSI_YELLOW: &str = "\x1b[33;1m";
const ANSI_CYAN: &str = "\x1b[36m";
const ANSI_BOLD: &str = "\x1b[1m";
const ANSI_RESET: &str = "\x1b[0m";

impl<'a> Reporter<'a> {
    pub fn new(source: &'a str) -> Self {
        Self {
            lines: source.lines().collect(),
            filename: None,
        }
    }

    pub fn with_filename(mut self, filename: &str) -> Self {
        self.filename = Some(filename.to_string());
        self
    }

    pub fn report(&self, line: usize, col: usize, len: usize, message: &str, level: &str) {
        let level_color = if level == "ERROR" { ANSI_RED } else { ANSI_YELLOW };
        
        if let Some(ref file) = self.filename {
            println!("{}{}[{}:{}:{}] {}{}{}: {}{}{}", ANSI_CYAN, ANSI_BOLD, file, line, col, level_color, ANSI_BOLD, level, ANSI_RESET, ANSI_BOLD, message);
        } else {
            println!("{}{}{}: {}{}{}", level_color, ANSI_BOLD, level, ANSI_RESET, ANSI_BOLD, message);
        }
        
        if line > 0 && line <= self.lines.len() {
            let line_content = self.lines[line - 1];
            println!("{} {:>3} |{} {}", ANSI_CYAN, line, ANSI_RESET, line_content);
            
            let padding = " ".repeat(col + 6);
            let highlight = if len > 0 { "~".repeat(len) } else { "^".to_string() };
            println!("{}{}{}{}", padding, ANSI_YELLOW, highlight, ANSI_RESET);
        }
        println!();
    }

    pub fn error(&self, line: usize, col: usize, len: usize, message: &str) {
        self.report(line, col, len, message, "ERROR");
    }

    #[allow(dead_code)]
    pub fn warn(&self, line: usize, col: usize, len: usize, message: &str) {
        self.report(line, col, len, message, "WARN");
    }
}
