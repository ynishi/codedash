//! Parser registry — resolves language name or file extension to a parser.

use crate::port::parser::LanguageParser;

pub struct ParserRegistry {
    parsers: Vec<Box<dyn LanguageParser>>,
}

impl ParserRegistry {
    pub fn new() -> Self {
        let mut reg = Self {
            parsers: Vec::new(),
        };
        reg.register(Box::new(super::rust::RustParser::new()));
        reg
    }

    pub fn register(&mut self, parser: Box<dyn LanguageParser>) {
        self.parsers.push(parser);
    }

    /// Look up a parser by language name.
    pub fn for_name(&self, name: &str) -> Option<&dyn LanguageParser> {
        self.parsers
            .iter()
            .find(|p| p.name() == name)
            .map(|p| p.as_ref())
    }

    /// Look up a parser by file extension.
    pub fn for_extension(&self, ext: &str) -> Option<&dyn LanguageParser> {
        self.parsers
            .iter()
            .find(|p| p.extensions().contains(&ext))
            .map(|p| p.as_ref())
    }
}

impl Default for ParserRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn for_name_returns_rust_parser() {
        let reg = ParserRegistry::new();
        let parser = reg.for_name("rust");
        assert!(parser.is_some());
        assert_eq!(parser.unwrap().name(), "rust");
    }

    #[test]
    fn for_name_returns_none_for_unknown() {
        let reg = ParserRegistry::new();
        assert!(reg.for_name("brainfuck").is_none());
    }

    #[test]
    fn for_extension_returns_rust_parser() {
        let reg = ParserRegistry::new();
        let parser = reg.for_extension("rs");
        assert!(parser.is_some());
        assert_eq!(parser.unwrap().name(), "rust");
    }

    #[test]
    fn for_extension_returns_none_for_unknown() {
        let reg = ParserRegistry::new();
        assert!(reg.for_extension("py").is_none());
    }
}
