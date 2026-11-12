# actionlint-plugin-shuck

This actionlint plugin runs [Shuck](https://github.com/ewhauser/shuck) on shell
scripts in GitHub Actions workflows and composite actions. It packages the Shuck
Rust library as a WebAssembly component. The plugin does not need a Shuck binary,
subprocess, network connection or filesystem access.

The plugin reports Shuck diagnostics at their original YAML locations. It also
maps supported fixes back to the YAML and checks composite actions called by a
workflow.

## Install and run

```sh
actionlint plugin install shuck
actionlint check --experimental .github
```

Use an actionlint v2 build that provides
`kjanat:actionlint-script/shell-script-inputs@0.3.0` to standalone analyzers. The
plugin implements `kjanat:actionlint/analyzer@0.2.0`. It registers the `shuck`
rule independently of ShellCheck. To run Shuck without ShellCheck, pass
`--no-tool=shellcheck`.

The `shuck` rule is a nursery rule. Enable it with `--experimental` or an explicit
rule setting.

This repository tests the v2 development host. Cargo fetches the SDK from the
actionlint Git revision pinned in `Cargo.toml` and `Cargo.lock`.

## Configure the rule

```yaml
lint:
  rules:
    correctness:
      shuck:
        level: warn
        options:
          select: [S001, C006]
          ignore: []
```

Use `select` to choose Shuck codes, prefixes, categories or named groups. If you
omit it, Shuck uses its defaults. Use `ignore` to remove matching rules. Unknown
options or selectors cause an error.

The plugin chooses Bash, POSIX shell, dash, mksh or Zsh from the workflow
interpreter. It recognizes custom commands by executable name, including quoted
names and paths. It skips unknown interpreters, non-shell languages and generic
`ksh`, whose name does not identify a dialect.

Set `dialect` to `bash`, `posix`, `mksh` or `zsh` to override the dialect of a
recognized shell script. This setting does not turn Python or PowerShell steps
into shell scripts. A shebang in `run:` does not override the workflow interpreter.

Place Shuck directives inside scripts or in attached YAML comments:

```yaml
- name: Pass the argument without quotes
  shell: bash
  # shuck: disable=S001
  run: echo $HOME
```

To suppress the plugin through actionlint, put
`# actionlint-ignore-next-line shuck` before a `run:` declaration.

## Fixes and limits

Fixes retain Shuck's safe or unsafe classification. The plugin offers edits on a
single line of a YAML block scalar. It does not offer fixes for inline scalars,
encoded characters, multiline replacements or lines containing `${{ }}`.
Those edits need YAML-aware rewriting. Diagnostics still point to the source.

The host masks expressions and supplies interpreter startup commands and declared
environment names without values or secrets. If unresolved names or spelling
prevent the host from determining the declarations, the plugin disables the
undefined-variable check (`C006`) for that script. Static declarations cannot
account for every variable exported by an earlier command or self-hosted runner.

The plugin does not follow `source` commands into other files. It analyzes
interpreter options only when the host supplies their startup commands. Support
for `zsh -f {0}` does not establish whether every Zsh option is modeled. The rule
remains in the nursery while the integration receives wider testing.

## Develop and test

Install Rust with rustup and install the tools in `mise.toml`. The Rust toolchain
configuration includes the `wasm32-wasip2` target.

```sh
mise exec -- bun install --frozen-lockfile
mise exec -- mise run test
mise exec -- mise run lint
mise exec -- mise run typecheck
mise exec -- mise run package
ACTIONLINT=/path/to/actionlint mise exec -- mise run test:host
```

In PowerShell, set `$env:ACTIONLINT = 'C:\path\to\actionlint.exe'` before running
`mise exec -- mise run test:host`.

The host tests load the component through actionlint. They cover dialects,
workflows, composite actions, configuration, suppressions and Unicode positions.
They also apply a fix and analyze the result again. Rust tests cover the mapping
of Shuck diagnostics and edits.
