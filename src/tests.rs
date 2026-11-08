use super::*;
use actionlint_extension_api::Position;
use actionlint_extension_api::SourceReference;
use actionlint_extension_api::Span;
use shell_script_inputs::Input as Prepared;
use shell_script_inputs::Line;

fn options() -> Options {
    Options::parse(r#"{"select":["S001"]}"#).expect("valid options")
}
fn script(text: &str) -> Script {
    Script {
        reference: SourceReference {
            document: "workflow.yml".into(),
            revision: "captured-revision".into(),
            node: "run".into(),
            address: "jobs.test.steps[0].run".into(),
        },
        job_id: "test".into(),
        step_id: String::new(),
        text: text.into(),
        shell: "bash".into(),
        start: Position {
            line: 6,
            column: 14,
        },
        path: "workflow.yml".into(),
        spans: vec![Span {
            line: 1,
            start: 1,
            end: u32::try_from(text.trim_end_matches('\n').chars().count() + 1)
                .expect("fixture width"),
            document: Position { line: 7, column: 9 },
            document_end: None,
        }],
        occurrence: Some("test-run".into()),
        runtime: None,
    }
}
#[test]
fn diagnostics_and_fixes_use_original_unicode_positions() {
    let script = script("echo 😀; echo $HOME\n");
    let prepared = Prepared {
        text: format!("# generated\nset -e\n{}", script.text),
        masked_text: script.text.clone(),
        dialect: Some("bash".into()),
        shell: "bash".into(),
        shell_setup: "set -e".into(),
        lines: vec![Line::Generated, Line::Generated, Line::Source(1)],
        excluded: Vec::new(),
    };
    let findings = engine::analyze(&prepared.text, ShellDialect::Bash, &options(), true)
        .expect("Shuck analysis");
    let mapped = map_diagnostics(&script, &prepared, findings).expect("source mapping");
    assert_eq!(mapped.len(), 1, "{mapped:?}");
    assert_eq!(mapped[0].code, "S001");
    assert_eq!((mapped[0].start.line, mapped[0].start.column), (7, 22));
    assert_eq!(mapped[0].fixes.len(), 1);
    let edits = &mapped[0].fixes[0].edits;
    assert_eq!(edits.len(), 1);
    assert_eq!((edits[0].start.line, edits[0].start.column), (7, 22));
    assert_eq!((edits[0].end.line, edits[0].end.column), (7, 27));
    assert!(
        edits
            .iter()
            .all(|edit| edit.path == "workflow.yml" && edit.replacement == "\"$HOME\"")
    );
    assert_eq!(mapped[0].fixes[0].revisions[0].digest, "captured-revision");
}
#[test]
fn shuck_dialect_changes_unquoted_expansion_analysis() {
    let bash = engine::analyze("echo $HOME\n", ShellDialect::Bash, &options(), true).expect("Bash");
    let zsh = engine::analyze("echo $HOME\n", ShellDialect::Zsh, &options(), true).expect("Zsh");
    assert_eq!(bash.diagnostics.len(), 1);
    assert!(zsh.diagnostics.is_empty(), "{:?}", zsh.diagnostics);
}
#[test]
fn own_suppressions_and_rule_selection_reach_shuck() {
    let disabled = Options::parse(r#"{"select":["S001"],"ignore":["S001"]}"#).expect("options");
    assert!(
        engine::analyze("echo $HOME\n", ShellDialect::Bash, &disabled, true)
            .expect("ignored rule")
            .diagnostics
            .is_empty()
    );
    let diagnostics = engine::analyze(
        "# shuck: disable=S001\necho $HOME\n",
        ShellDialect::Bash,
        &options(),
        true,
    )
    .expect("directive");
    assert!(
        diagnostics.diagnostics.is_empty(),
        "{:?}",
        diagnostics.diagnostics
    );
}
#[test]
fn invalid_options_are_rejected_by_the_engine() {
    for config in [
        r#"{"unknown":true}"#,
        r#"{"dialect":"python"}"#,
        r#"{"select":["NO-SUCH-RULE"]}"#,
        r#"{"ignore":[""]}"#,
    ] {
        assert!(Options::parse(config).is_err(), "{config}");
    }
}
#[test]
fn applying_shuck_edits_removes_the_reported_problem() {
    let source = "echo $HOME\n";
    let findings = engine::analyze(source, ShellDialect::Bash, &options(), true).expect("analysis");
    let fixed = shuck_linter::apply_fixes(source, &findings.diagnostics, Applicability::Unsafe);
    assert_eq!(fixed.fixes_applied, 1);
    assert_eq!(fixed.code, "echo \"$HOME\"\n");
    assert!(
        engine::analyze(&fixed.code, ShellDialect::Bash, &options(), true)
            .expect("fixed source")
            .diagnostics
            .is_empty()
    );
}
#[test]
fn fixes_do_not_edit_workflow_expression_lines() {
    let script = script("echo $HOME ${{ inputs.name }}\n");
    let prepared = Prepared {
        text: "echo $HOME masked-template\n".into(),
        masked_text: "echo $HOME masked-template\n".into(),
        dialect: Some("bash".into()),
        shell: "bash".into(),
        shell_setup: String::new(),
        lines: vec![Line::Source(1)],
        excluded: Vec::new(),
    };
    let findings =
        engine::analyze(&prepared.text, ShellDialect::Bash, &options(), true).expect("analysis");
    let mapped = map_diagnostics(&script, &prepared, findings).expect("mapping");
    assert_eq!(mapped.len(), 1);
    assert!(mapped[0].fixes.is_empty());
}

#[test]
fn workflow_shell_selects_the_dialect_instead_of_script_comments() {
    let prepared = Prepared {
        text: "#!/bin/zsh\necho $HOME\n".into(),
        masked_text: String::new(),
        dialect: Some("bash".into()),
        shell: "bash".into(),
        shell_setup: String::new(),
        lines: Vec::new(),
        excluded: Vec::new(),
    };
    assert_eq!(dialect(&prepared, &options()), ShellDialect::Bash);
    let python = Prepared {
        dialect: None,
        shell: "python".into(),
        ..prepared
    };
    let override_bash = Options::parse(r#"{"dialect":"bash"}"#).expect("options");
    assert_eq!(dialect(&python, &override_bash), ShellDialect::Unknown);
}

#[test]
fn encoded_scalars_have_diagnostics_without_raw_yaml_edits() {
    let mut script = script("echo $HOME\n");
    script.start.line = 7;
    let prepared = Prepared {
        text: script.text.clone(),
        masked_text: script.text.clone(),
        dialect: Some("bash".into()),
        shell: "bash".into(),
        shell_setup: String::new(),
        lines: vec![Line::Source(1)],
        excluded: Vec::new(),
    };
    let findings =
        engine::analyze(&prepared.text, ShellDialect::Bash, &options(), true).expect("analysis");
    let mapped = map_diagnostics(&script, &prepared, findings).expect("mapping");
    assert_eq!(mapped.len(), 1);
    assert!(mapped[0].fixes.is_empty());
}

#[test]
fn host_environment_declarations_are_visible_to_shuck() {
    let options = Options::parse(r#"{"select":["C006"]}"#).expect("options");
    assert!(
        engine::analyze(
            "export declared_name\necho \"$declared_name\"\n",
            ShellDialect::Bash,
            &options,
            true
        )
        .expect("declared variable")
        .diagnostics
        .is_empty()
    );
    assert!(
        engine::analyze("echo \"$missing\"\n", ShellDialect::Bash, &options, false)
            .expect("unknown environment")
            .diagnostics
            .is_empty()
    );
}

#[test]
fn unterminated_strings_report_parse_errors_in_yaml() {
    let script = script("echo \"unterminated\n");
    let prepared = Prepared {
        text: script.text.clone(),
        masked_text: script.text.clone(),
        dialect: Some("bash".into()),
        shell: "bash".into(),
        shell_setup: String::new(),
        lines: vec![Line::Source(1)],
        excluded: Vec::new(),
    };
    let findings = engine::analyze(
        &prepared.text,
        ShellDialect::Bash,
        &Options::default(),
        true,
    )
    .expect("analysis");
    assert!(
        !findings.parse_diagnostics.is_empty(),
        "Shuck must report the unterminated string"
    );
    let expected = findings.diagnostics.len() + findings.parse_diagnostics.len();
    let mapped = map_diagnostics(&script, &prepared, findings).expect("mapping");
    assert_eq!(
        mapped.len(),
        expected,
        "a parser diagnostic was dropped: {mapped:?}"
    );
}

#[test]
fn custom_commands_select_the_executable_dialect() {
    for (command, expected) in [
        ("zsh -f {0}", ShellDialect::Zsh),
        ("mksh {0}", ShellDialect::Mksh),
        ("'/usr/local/bin/zsh' -f {0}", ShellDialect::Zsh),
        (r#""C:\Program Files\zsh.exe" -f {0}"#, ShellDialect::Zsh),
        ("python {0}", ShellDialect::Unknown),
        ("unknown-wrapper {0}", ShellDialect::Unknown),
        ("ksh {0}", ShellDialect::Unknown),
    ] {
        let prepared = Prepared {
            text: String::new(),
            masked_text: String::new(),
            dialect: None,
            shell: command.into(),
            shell_setup: String::new(),
            lines: Vec::new(),
            excluded: Vec::new(),
        };
        assert_eq!(dialect(&prepared, &options()), expected, "{command}");
    }
}
