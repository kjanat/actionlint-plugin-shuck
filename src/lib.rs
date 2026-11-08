mod engine;
mod options;
mod source;

use actionlint_extension_api::Descriptor;
use actionlint_extension_api::Diagnostic;
use actionlint_extension_api::Edit;
use actionlint_extension_api::Fix;
use actionlint_extension_api::Guest;
use actionlint_extension_api::Input;
use actionlint_extension_api::InputKind;
use actionlint_extension_api::Outcome;
use actionlint_extension_api::Revision;
use actionlint_extension_api::Script;
use actionlint_extension_api::kjanat::actionlint_script::environments;
use actionlint_extension_api::kjanat::actionlint_script::shell_script_inputs;
use options::Options;
use shuck_linter::Applicability;
use shuck_linter::ShellDialect;
use source::SourceMap;

pub struct Plugin;
impl Guest for Plugin {
    fn describe() -> Descriptor {
        descriptor()
    }
    fn analyze(input: Input) -> Outcome {
        let options = match Options::parse(&input.options) {
            Ok(options) => options,
            Err(error) => {
                return Outcome {
                    diagnostics: Vec::new(),
                    error: Some(error),
                };
            }
        };
        let mut diagnostics = Vec::new();
        for mut script in input.scripts.into_iter().chain(input.invoked) {
            if script.path.is_empty() {
                script.path.clone_from(&input.path);
            }
            match analyze_script(&script, &options) {
                Ok(findings) => diagnostics.extend(findings),
                Err(error) => {
                    return Outcome {
                        diagnostics,
                        error: Some(error),
                    };
                }
            }
        }
        Outcome {
            diagnostics,
            error: None,
        }
    }
}

pub fn descriptor() -> Descriptor {
    Descriptor {
        name: "shuck".into(),
        description: "Check shell scripts with Shuck".into(),
        category: "correctness".into(),
        subcategory: "shell".into(),
        maturity: "nursery".into(),
        provider: "kjanat/actionlint-plugin-shuck".into(),
        recommended: true,
        explanation: "Checks Bash, POSIX shell, mksh and Zsh scripts with Shuck.".into(),
        help: "Use select, ignore and dialect under lint.rules.correctness.shuck.options.".into(),
        documentation_url: env!("CARGO_PKG_REPOSITORY").into(),
        fix_availability: String::new(),
        options_schema: options::schema(),
        examples: Vec::new(),
        required_inputs: vec![
            InputKind::Scripts,
            InputKind::Invocations,
            InputKind::Execution,
        ],
    }
}

fn dialect(prepared: &shell_script_inputs::Input, options: &Options) -> ShellDialect {
    let inferred = if let Some(name) = &prepared.dialect {
        ShellDialect::from_name(name)
    } else {
        let Ok(words) = shell_words::split(&prepared.shell) else {
            return ShellDialect::Unknown;
        };
        let Some(executable) = words.first() else {
            return ShellDialect::Unknown;
        };
        let name = executable.rsplit(['/', '\\']).next().unwrap_or(executable);
        ShellDialect::from_name(name.strip_suffix(".exe").unwrap_or(name))
    };
    match inferred {
        ShellDialect::Unknown | ShellDialect::Ksh => ShellDialect::Unknown,
        _ => options.dialect.map_or(inferred, |dialect| dialect.shell()),
    }
}

fn analyze_script(script: &Script, options: &Options) -> Result<Vec<Diagnostic>, String> {
    let occurrence = script
        .occurrence
        .as_deref()
        .ok_or("script occurrence missing")?;
    let Some(mut prepared) = shell_script_inputs::for_script(occurrence, None)? else {
        return Ok(Vec::new());
    };
    let shell = dialect(&prepared, options);
    if shell == ShellDialect::Unknown {
        return Ok(Vec::new());
    }
    if prepared.dialect.as_deref() != Some(shell.as_str()) {
        let Some(rebuilt) = shell_script_inputs::for_script(occurrence, Some(shell.as_str()))?
        else {
            return Ok(Vec::new());
        };
        prepared = rebuilt;
    }
    let facts = environments::for_script(occurrence)?;
    let environment_known = !facts.is_empty()
        && facts.iter().all(|fact| {
            !fact.unknown_names
                && fact.lookup_comparison != environments::NameComparison::Unknown
                && fact.names.iter().all(|name| name.case_known)
        });
    map_diagnostics(
        script,
        &prepared,
        engine::analyze(&prepared.text, shell, options, environment_known)?,
    )
}

fn map_diagnostics(
    script: &Script,
    prepared: &shell_script_inputs::Input,
    findings: engine::Analysis,
) -> Result<Vec<Diagnostic>, String> {
    let map = SourceMap { script, prepared };
    let mut diagnostics = Vec::new();
    for finding in findings.diagnostics {
        let empty = finding.span.start.offset() == finding.span.end.offset();
        let Some(start) = map.position(finding.span.start.offset(), empty) else {
            continue;
        };
        let Some(end) = map.position(finding.span.end.offset(), true) else {
            continue;
        };
        let mut fixes = Vec::new();
        if let Some(fix) = finding.fix {
            let edits = fix
                .edits()
                .iter()
                .map(|edit| {
                    let start_offset = usize::from(edit.range().start());
                    let end_offset = usize::from(edit.range().end());
                    if edit.content().contains(['\0', '\r', '\n'])
                        || !map.fix_allowed(start_offset, end_offset)
                    {
                        return None;
                    }
                    Some(Edit {
                        path: script.path.clone(),
                        start: map.position(start_offset, true)?,
                        end: map.position(end_offset, true)?,
                        replacement: edit.content().into(),
                    })
                })
                .collect::<Option<Vec<_>>>();
            if let Some(edits) = edits.filter(|edits| !edits.is_empty()) {
                fixes.push(Fix {
                    applicability: match fix.applicability() {
                        Applicability::Safe => "safe",
                        Applicability::Unsafe => "unsafe",
                    }
                    .into(),
                    revisions: vec![Revision {
                        path: script.path.clone(),
                        digest: script.reference.revision.clone(),
                    }],
                    description: finding
                        .fix_title
                        .clone()
                        .unwrap_or_else(|| finding.message.clone()),
                    edits,
                });
            }
        }
        diagnostics.push(Diagnostic {
            reference: Some(script.reference.clone()),
            rule: String::new(),
            code: finding.rule.code().into(),
            severity: match finding.severity {
                shuck_linter::Severity::Hint => "info",
                shuck_linter::Severity::Warning => "warning",
                shuck_linter::Severity::Error => "error",
            }
            .into(),
            message: finding.message,
            path: script.path.clone(),
            start,
            end,
            fixes,
            explanation: String::new(),
            help: String::new(),
            documentation_url: "https://github.com/ewhauser/shuck".into(),
            related: Vec::new(),
        });
    }
    for finding in findings.parse_diagnostics {
        let Some(start) = map.position(finding.span.start.offset(), true) else {
            continue;
        };
        let Some(end) = map.position(finding.span.end.offset(), true) else {
            continue;
        };
        diagnostics.push(Diagnostic {
            reference: Some(script.reference.clone()),
            rule: String::new(),
            code: "parse".into(),
            severity: "error".into(),
            message: finding.message,
            path: script.path.clone(),
            start,
            end,
            fixes: Vec::new(),
            explanation: String::new(),
            help: String::new(),
            documentation_url: "https://github.com/ewhauser/shuck".into(),
            related: Vec::new(),
        });
    }
    Ok(diagnostics)
}

#[cfg(target_arch = "wasm32")]
actionlint_extension_api::export_plugin!(Plugin);

#[cfg(test)]
mod tests;
