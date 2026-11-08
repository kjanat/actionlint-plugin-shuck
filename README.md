# actionlint-plugin-shuck

[Shuck](https://github.com/ewhauser/shuck) checks shell scripts in GitHub Actions
workflows and composite actions. This plugin embeds Shuck's Rust library in a
WebAssembly component. It needs no native Shuck executable, subprocess, network
access or filesystem access.

It reports Shuck's codes, messages and severities, with locations and supported
fixes mapped to the original YAML. Called composite actions are checked too.

## Use

```sh
mise exec -- mise run package
actionlint check --experimental --plugin ./artifacts/plugin.wasm .github
```

Use an actionlint v2 build that supplies
`kjanat:actionlint-script/shell-script-inputs@0.3.0` to standalone analyzers.
The component implements `kjanat:actionlint/analyzer@0.2.0`. It adds its own
`shuck` rule, so Shuck and ShellCheck can run together. Add `--no-tool=shellcheck`
to run Shuck without ShellCheck.

`--experimental` enables this nursery rule. An explicit rule setting, as shown
below, also enables it.

This repository tests the current v2 development host. The SDK is included in
[`vendor/actionlint-extension-api`](vendor/actionlint-extension-api).
[`vendor/sdk-snapshot.json`](vendor/sdk-snapshot.json) records its source commit,
local changes and file hashes.

## Configure

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

`select` accepts Shuck codes, prefixes, categories and named groups. If omitted,
Shuck's defaults apply. `ignore` removes matching rules. Unknown options and
selectors are errors.

The workflow interpreter selects Bash, POSIX shell, dash, mksh or Zsh. Custom
commands are recognized by executable name, including paths and quoted names.
Unknown interpreters and non-shell languages are skipped. Generic `ksh` is
skipped because its name does not identify a dialect. The optional `dialect`
setting accepts `bash`, `posix`, `mksh` or `zsh`; it changes the dialect of
recognized shell scripts. It does not turn Python or PowerShell steps into shell
scripts. A shebang in `run:` does not override the workflow's interpreter.

Shuck directives work inside scripts and in attached YAML comments:

```yaml
- name: Unquoted argument required by this command
  shell: bash
  # shuck: disable=S001
  run: echo $HOME
```

Use `# actionlint-ignore-next-line shuck` before a `run:` declaration to suppress
the plugin through actionlint.

## Fixes and limits

Each fix carries Shuck's safe or unsafe classification. The plugin offers
single-line edits within YAML block scalars. It omits fixes for inline scalars,
encoded characters, multiline replacements and lines containing `${{ }}`. These
cases need YAML-aware rewriting before edits can be offered safely. Diagnostics
still point to the source.

The host masks expressions and supplies interpreter startup commands and declared
environment names without values or secrets. When unresolved names or spelling
prevent the host from determining those declarations, the plugin disables
undefined-variable checking (`C006`) for that script. Static declarations cannot
describe every variable an earlier command or a self-hosted runner might export.

The plugin does not follow `source` commands into other files. Interpreter options
are analyzed only when the host supplies their startup commands. Recognizing
`zsh -f {0}` does not establish that every Zsh option has been modeled. The rule
is marked `nursery` while this integration gets wider testing.

## Develop and test

Install Rust through rustup and the tools declared in `mise.toml`.
`rust-toolchain.toml` includes the `wasm32-wasip2` target.

```sh
mise exec -- bun install --frozen-lockfile
mise exec -- mise run test
mise exec -- mise run lint
mise exec -- mise run typecheck
mise exec -- mise run package
ACTIONLINT=/path/to/actionlint mise exec -- mise run test:host
```

In PowerShell, set the executable with
`$env:ACTIONLINT = 'C:\path\to\actionlint.exe'`, then run
`mise exec -- mise run test:host`.

The integration tests load the real component through actionlint. They check
dialects, workflows, composite actions, configuration, suppressions, Unicode
positions and applying fixes followed by another analysis. Rust tests also check
the mapping of native Shuck diagnostics and edits.
