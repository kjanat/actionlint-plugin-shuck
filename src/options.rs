use schemars::JsonSchema;
use serde::Deserialize;
use shuck_linter::LinterSettings;
use shuck_linter::ShellDialect;
use shuck_linter::parse_rule_selectors;

#[derive(Clone, Copy, Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Dialect {
    Bash,
    Posix,
    Mksh,
    Zsh,
}
impl Dialect {
    pub fn shell(self) -> ShellDialect {
        match self {
            Self::Bash => ShellDialect::Bash,
            Self::Posix => ShellDialect::Sh,
            Self::Mksh => ShellDialect::Mksh,
            Self::Zsh => ShellDialect::Zsh,
        }
    }
}

#[derive(Default, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Options {
    /// Override the workflow interpreter's dialect.
    pub dialect: Option<Dialect>,
    /// Shuck rule codes, prefixes, categories or named groups to enable. Omission uses Shuck's defaults.
    pub select: Option<Vec<String>>,
    /// Shuck rule selectors to disable.
    pub ignore: Vec<String>,
}
impl Options {
    pub fn parse(text: &str) -> Result<Self, String> {
        let options: Self = serde_json::from_str(if text.trim().is_empty() { "{}" } else { text })
            .map_err(|error| error.to_string())?;
        options.settings(ShellDialect::Bash, true)?;
        Ok(options)
    }
    pub fn settings(
        &self,
        shell: ShellDialect,
        environment_known: bool,
    ) -> Result<LinterSettings, String> {
        let ignore =
            parse_rule_selectors(&self.ignore, "ignore").map_err(|error| error.to_string())?;
        let mut settings = if let Some(select) = &self.select {
            let select =
                parse_rule_selectors(select, "select").map_err(|error| error.to_string())?;
            LinterSettings::from_selectors(&select, &ignore)
        } else {
            let mut settings = LinterSettings::default();
            for selector in ignore {
                for rule in selector.into_rule_set().iter() {
                    settings.rules.remove(rule);
                }
            }
            settings
        };
        settings.shell = shell;
        settings.resolve_source_closure = false;
        if !environment_known {
            settings.rules.remove(shuck_linter::Rule::UndefinedVariable);
        }
        Ok(settings)
    }
}
pub fn schema() -> String {
    serde_json::to_string(&schemars::schema_for!(Options)).expect("options schema is JSON")
}
