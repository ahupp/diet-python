use crate::{ruff_ast_to_string, transformer::Transformer};
use ruff_python_ast::{ModModule, Stmt};
use ruff_source_file::LineIndex;
use ruff_text_size::{Ranged, TextRange};

#[derive(Debug, Clone)]
pub struct LineEntry {
    pub line: Option<usize>,
    pub text: String,
}

pub fn source_lines(source: &str) -> Vec<String> {
    source
        .split('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line).to_string())
        .collect()
}

pub fn stmt_lines_with_locations(
    module: &mut ModModule,
    line_index: &LineIndex,
    source: &str,
) -> Vec<LineEntry> {
    let mut collector = StmtLineCollector {
        line_index,
        source,
        entries: Vec::new(),
    };
    collector.visit_body(&mut module.body);
    collector.entries
}

struct StmtLineCollector<'a> {
    line_index: &'a LineIndex,
    source: &'a str,
    entries: Vec<LineEntry>,
}

impl Transformer for StmtLineCollector<'_> {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        let line = line_number(stmt.range(), self.line_index, self.source);
        let rendered = ruff_ast_to_string(&*stmt);
        for line_text in split_lines(&rendered) {
            self.entries.push(LineEntry {
                line,
                text: line_text,
            });
        }
    }
}

fn line_number(range: TextRange, line_index: &LineIndex, source: &str) -> Option<usize> {
    if range.is_empty() {
        return None;
    }
    Some(line_index.line_column(range.start(), source).line.get())
}

fn split_lines(text: &str) -> Vec<String> {
    let trimmed = text.strip_suffix('\n').unwrap_or(text);
    trimmed
        .split('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line).to_string())
        .collect()
}
