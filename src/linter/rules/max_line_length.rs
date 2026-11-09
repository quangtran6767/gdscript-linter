use crate::linter::LinterConfig;
use crate::linter::diagnostic::{Diagnostic, Severity, Span};
use crate::linter::rules::Rule;
use crate::node_kind::GDScriptNodeKind;
use crate::shared_utils::measure_line_length_until_line_return;

pub struct MaxLineLengthRule {
    config: LinterConfig,
}

impl MaxLineLengthRule {
    pub fn new(config: &LinterConfig) -> Self {
        Self {
            config: config.clone(),
        }
    }
}

impl Rule for MaxLineLengthRule {
    fn check_source(&mut self, source_code: &str, diagnostics: &mut Vec<Diagnostic>) {
        let max_line_length = self.config.max_line_length;
        let mut line_start_byte = 0;
        for line in source_code.split_inclusive('\n') {
            let current_line_start = line_start_byte;
            line_start_byte += line.len();

            let line_without_return = line.trim_end_matches(['\n', '\r']);
            let display_width =
                measure_line_length_until_line_return(line_without_return, self.config.indent_size);
            if display_width <= max_line_length {
                continue;
            }

            // The span covers the part of the line past the limit. Tabs count
            // as one indentation level, like in the formatter.
            let mut overflow_start = line_without_return.len();
            let mut width = 0;
            for (byte_index, character) in line_without_return.char_indices() {
                if character == '\t' {
                    width += self.config.indent_size;
                } else {
                    width += 1;
                }
                if width > max_line_length {
                    overflow_start = byte_index;
                    break;
                }
            }

            diagnostics.push(Diagnostic {
                check: None,
                rule: "max-line-length".to_string(),
                severity: Severity::Warning,
                message: format!(
                    "Line is too long. Found {} characters, maximum allowed is {}",
                    display_width, max_line_length
                ),
                help: Some(
                    "Split the line, for example by wrapping arguments in parentheses or extracting a variable."
                        .to_string(),
                ),
                span: Span::from_byte_range(
                    source_code,
                    current_line_start + overflow_start,
                    current_line_start + line_without_return.len(),
                ),
                fixes: Vec::new(),
            });
        }
    }

    fn get_target_ast_nodes(&self) -> &[GDScriptNodeKind] {
        &[]
    }
}
