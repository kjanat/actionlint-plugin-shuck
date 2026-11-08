use crate::options::Options;
use shuck_linter::AnalysisRequest;
use shuck_linter::Diagnostic;
use shuck_linter::ShellDialect;
use shuck_parser::parser::ParseDiagnostic;
use shuck_parser::parser::Parser;

pub struct Analysis {
    pub diagnostics: Vec<Diagnostic>,
    pub parse_diagnostics: Vec<ParseDiagnostic>,
}

pub fn analyze(
    source: &str,
    shell: ShellDialect,
    options: &Options,
    environment_known: bool,
) -> Result<Analysis, String> {
    let settings = options.settings(shell, environment_known)?;
    let parsed = Parser::with_profile(source, shell.shell_profile()).parse();
    Ok(Analysis {
        diagnostics: AnalysisRequest::from_parse_result(&parsed, source, &settings).lint(),
        parse_diagnostics: parsed.diagnostics,
    })
}
