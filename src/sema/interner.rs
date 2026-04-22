
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StringId(pub u32);

#[derive(Debug, Clone)]
/// Global string interner for the XCX compiler.
/// Transforms strings into unique `StringId` values to optimize comparison
/// and reduce memory usage across the AST and bytecode.
pub struct Interner {
    map: HashMap<String, StringId>,
    strings: Vec<String>,
}

impl Interner {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
            strings: Vec::new(),
        }
    }

    pub fn intern(&mut self, s: &str) -> StringId {
        if let Some(&id) = self.map.get(s) {
            return id;
        }

        let id = StringId(self.strings.len() as u32);
        self.strings.push(s.to_string());
        self.map.insert(s.to_string(), id);
        id
    }

    pub fn lookup(&self, id: StringId) -> &str {
        &self.strings[id.0 as usize]
    }
}
