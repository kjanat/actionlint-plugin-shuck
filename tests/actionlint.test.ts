import { afterAll, beforeAll, expect, test } from 'bun:test';
import { copyFile, mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { z } from 'zod';

const position = z.object({ line: z.number().int().positive(), column: z.number().int().positive() });
const resultSchema = z.object({
	diagnostics: z.array(z.object({
		rule: z.string(),
		code: z.string().optional(),
		severity: z.string(),
		message: z.string(),
		path: z.string(),
		start: position,
		end: position,
		fixes: z.array(z.object({
			applicability: z.string(),
			edits: z.array(z.object({ path: z.string(), start: position, end: position, replacement: z.string() })),
		})).optional(),
	})),
});

const executable = process.env.ACTIONLINT;
if (!executable) throw new Error('Set ACTIONLINT to the actionlint v2 executable to test.');
const actionlint = resolve(executable);
const component = resolve('artifacts/plugin.wasm');
const plugin = process.env.ACTIONLINT_PLUGIN;
let directory: string;

beforeAll(async () => {
	directory = await mkdtemp(join(tmpdir(), 'actionlint-shuck-'));
	const git = Bun.spawnSync(['git', 'init', '--quiet', '--initial-branch=master', directory]);
	expect(git.exitCode, git.stderr.toString()).toBe(0);
	await mkdir(join(directory, '.github/actions/example'), { recursive: true });
	await copyFile(new URL('fixtures/workflow.yml', import.meta.url), join(directory, 'workflow.yml'));
	await copyFile(
		new URL('fixtures/action.yml', import.meta.url),
		join(directory, '.github/actions/example/action.yml'),
	);
	await copyFile(new URL('fixtures/actionlint.yaml', import.meta.url), join(directory, 'actionlint.yaml'));
});
afterAll(async () => {
	if (directory) await rm(directory, { recursive: true, force: true });
});

async function check(path = 'workflow.yml', config = 'actionlint.yaml') {
	const env = { ...process.env, XDG_CACHE_HOME: join(directory, 'cache'), LOCALAPPDATA: join(directory, 'cache') };
	if (plugin) {
		const installation = Bun.spawnSync([actionlint, 'plugin', 'install', plugin, '--config', config], {
			cwd: directory,
			env,
		});
		expect(installation.exitCode, installation.stderr.toString()).toBe(0);
		expect(await readFile(join(directory, config), 'utf8')).toContain('github.com/kjanat/actionlint-plugin-shuck@');
	}
	const child = Bun.spawn([
		actionlint,
		'check',
		...(plugin ? [] : ['--plugin', component]),
		'--config',
		config,
		'--no-tool=shellcheck',
		'--no-tool=ruff',
		'--json',
		path,
	], { cwd: directory, env, stdout: 'pipe', stderr: 'pipe' });
	const [stdout, stderr, status] = await Promise.all([
		new Response(child.stdout).text(),
		new Response(child.stderr).text(),
		child.exited,
	]);
	expect(status, stderr || stdout).toBeLessThan(2);
	return resultSchema.parse(JSON.parse(stdout));
}

test('real Wasm analysis maps Bash, POSIX, mksh and invoked composite findings', async () => {
	const result = await check();
	const findings = result.diagnostics.filter(diagnostic => diagnostic.rule === 'shuck');
	expect(
		findings.map(diagnostic => [diagnostic.code, diagnostic.start.line, diagnostic.start.column, diagnostic.severity]),
	)
		.toEqual([
			['S001', 8, 17, 'warning'],
			['S001', 11, 24, 'warning'],
			['S001', 17, 19, 'warning'],
			['S001', 23, 19, 'warning'],
		]);
	expect(findings[1]?.fixes?.[0]?.edits[0]?.replacement).toBe('"$HOME"');
	expect(findings[0]?.path.replaceAll('\\', '/')).toEndWith('.github/actions/example/action.yml');
});

test('standalone composite actions use the same plugin', async () => {
	const result = await check('.github/actions/example/action.yml');
	expect(result.diagnostics.filter(diagnostic => diagnostic.rule === 'shuck').map(diagnostic => diagnostic.code))
		.toEqual(['S001']);
});

test('configuration can disable Shuck and change its level', async () => {
	await writeFile(join(directory, 'off.yaml'), 'lint:\n  rules:\n    correctness:\n      shuck: off\n');
	expect((await check('workflow.yml', 'off.yaml')).diagnostics.filter(diagnostic => diagnostic.rule === 'shuck'))
		.toEqual([]);
	await writeFile(
		join(directory, 'info.yaml'),
		'lint:\n  rules:\n    correctness:\n      shuck:\n        level: info\n        options:\n          select: [S001]\n',
	);
	const findings = (await check('workflow.yml', 'info.yaml')).diagnostics.filter(diagnostic =>
		diagnostic.rule === 'shuck'
	);
	expect(findings.length).toBeGreaterThan(0);
	expect(findings.every(diagnostic => diagnostic.severity === 'info')).toBe(true);
});

test('applying component edits removes the finding and produces valid YAML', async () => {
	const result = await check();
	const finding = result.diagnostics.find(diagnostic => diagnostic.rule === 'shuck' && diagnostic.start.line === 11);
	expect(finding?.fixes).toHaveLength(1);
	const fix = finding?.fixes?.[0];
	if (!fix) throw new Error('Expected a Shuck fix for the block scalar');
	const lines = (await readFile(join(directory, 'workflow.yml'), 'utf8')).split('\n');
	for (const edit of [...fix.edits].sort((a, b) => b.start.line - a.start.line || b.start.column - a.start.column)) {
		expect(edit.start.line).toBe(edit.end.line);
		const line = lines[edit.start.line - 1];
		if (line === undefined) throw new Error('Edit points outside the workflow');
		const points = Array.from(line);
		lines[edit.start.line - 1] = [
			...points.slice(0, edit.start.column - 1),
			edit.replacement,
			...points.slice(edit.end.column - 1),
		].join('');
	}
	await writeFile(join(directory, 'fixed.yml'), lines.join('\n'));
	const fixed = await check('fixed.yml');
	expect(fixed.diagnostics.some(diagnostic => diagnostic.rule === 'syntax-check')).toBe(false);
	expect(fixed.diagnostics.some(diagnostic => diagnostic.rule === 'shuck' && diagnostic.start.line === 11)).toBe(false);
});

test('quoted YAML maps escaped characters without offering an unsafe YAML rewrite', async () => {
	const line = '        run: "echo \\U0001F600; echo $HOME"';
	await writeFile(
		join(directory, 'quoted.yml'),
		`on: push\njobs:\n  check:\n    runs-on: ubuntu-latest\n    steps:\n      - shell: bash\n${line}\n`,
	);
	const result = await check('quoted.yml');
	const findings = result.diagnostics.filter(diagnostic => diagnostic.rule === 'shuck');
	expect(findings).toHaveLength(1);
	expect(findings[0]?.code).toBe('S001');
	expect(findings[0]?.start).toEqual({ line: 7, column: line.indexOf('$HOME') + 1 });
	expect(findings[0]?.fixes ?? []).toEqual([]);
});

test('unterminated shell text produces a parser diagnostic', async () => {
	await writeFile(
		join(directory, 'broken.yml'),
		'on: push\njobs:\n  check:\n    runs-on: ubuntu-latest\n    steps:\n      - shell: bash\n        run: |\n          echo "unterminated\n',
	);
	const result = await check('broken.yml');
	const findings = result.diagnostics.filter(diagnostic => diagnostic.rule === 'shuck');
	expect(findings.some(diagnostic => diagnostic.code === 'parse' && diagnostic.start.line === 8)).toBe(true);
});
