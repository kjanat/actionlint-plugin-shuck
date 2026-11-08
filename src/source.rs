use actionlint_extension_api::Position;
use actionlint_extension_api::Script;
use actionlint_extension_api::kjanat::actionlint_script::shell_script_inputs::Input;
use actionlint_extension_api::kjanat::actionlint_script::shell_script_inputs::Line;

pub struct SourceMap<'a> {
    pub script: &'a Script,
    pub prepared: &'a Input,
}
impl SourceMap<'_> {
    pub fn position(&self, offset: usize, end: bool) -> Option<Position> {
        if offset == self.prepared.text.len() && self.prepared.text.ends_with('\n') {
            return self.position(offset - 1, true);
        }
        let prefix = self.prepared.text.get(..offset)?;
        let row = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
        let column = prefix.rsplit('\n').next()?.chars().count() + 1;
        let line = self.prepared.lines.get(row - 1).or_else(|| {
            if end && column == 1 {
                self.prepared.lines.last()
            } else {
                None
            }
        })?;
        match line {
            Line::Generated => None,
            Line::Comment(start) => Some(Position {
                line: start.line,
                column: start.column.checked_add(u32::try_from(column - 1).ok()?)?,
            }),
            Line::Source(original) => {
                let source_row = if row > self.prepared.lines.len() {
                    original.checked_add(1)?
                } else {
                    *original
                };
                self.script_position(source_row, u32::try_from(column).ok()?, end)
            }
        }
    }
    fn script_position(&self, row: u32, column: u32, end: bool) -> Option<Position> {
        for span in &self.script.spans {
            if span.line == row
                && column >= span.start
                && (column < span.end || end && column == span.end)
            {
                if let Some(position) = span.document_end {
                    return Some(if end && column == span.end {
                        position
                    } else {
                        span.document
                    });
                }
                return Some(Position {
                    line: span.document.line,
                    column: span.document.column.checked_add(column - span.start)?,
                });
            }
        }
        None
    }
    pub fn fix_allowed(&self, start: usize, end: usize) -> bool {
        let Some(prefix) = self.prepared.text.get(..start) else {
            return false;
        };
        let Some(range) = self.prepared.text.get(start..end) else {
            return false;
        };
        if range.contains('\n') {
            return false;
        }
        let row = prefix.bytes().filter(|byte| *byte == b'\n').count();
        match self.prepared.lines.get(row) {
            Some(Line::Source(original)) => {
                let start_column = prefix
                    .rsplit('\n')
                    .next()
                    .map_or(0, |line| line.chars().count())
                    + 1;
                let end_column = start_column + range.chars().count();
                self.script.spans.iter().any(|span| {
                    span.line == *original
                        && span.document.line > self.script.start.line
                        && span.document_end.is_none()
                        && start_column >= span.start as usize
                        && end_column <= span.end as usize
                }) && self
                    .script
                    .text
                    .split('\n')
                    .nth((*original as usize).saturating_sub(1))
                    .is_some_and(|line| !line.contains("${{"))
            }
            _ => false,
        }
    }
}
