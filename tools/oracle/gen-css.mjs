#!/usr/bin/env node
// Q3 corpus generator: runs the REAL svelte/compiler 5.57.2 over every `.svelte` with a `<style>`
// in Svelte's own test samples (css, runtime-*, snapshot, validator, compiler-errors, ...) and in
// this repo's fixtures, and writes what its CSS pipeline (phases/2-analyze/css + 3-transform/css)
// produced to crates/svelters-compiler/tests/css_scope/corpus.json, which is checked in: the Rust
// comparison test (tests/css_scope.rs) needs no Node.
//
// Per case: the source, the options, and for `css: 'external'` the scoped stylesheet
// (`css.code`), `css.hasGlobal` and the `css_*` warnings; for `css: 'injected'` (not dev) the
// minified stylesheet (the `$$css.code` string of the client output); the arguments the
// `cssHash` option received, Svelte's default hash of the CSS and of the filename. A case whose
// compile throws a `css_*` (or unexpected_eof inside `<style>`) error records it; any other
// failure drops the case.
//
// Positions are UTF-16 offsets (upstream's).
//
// Usage: node tools/oracle/gen-css.mjs
//   env UPSTREAM=/path (default /tmp/claude-0/up): a checkout of sveltejs/svelte @ tag svelte@5.57.2 in $UPSTREAM/svelte
//   env SVELTE_PKG=/path/to/node_modules/svelte (default: node_modules/.pnpm/svelte@5.57.2 of this repo)

import fs from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';
import { fileURLToPath, pathToFileURL } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, '../..');
const upstream = process.env.UPSTREAM || '/tmp/claude-0/up';
const out = path.join(root, 'crates/svelters-compiler/tests/css_scope/corpus.json');
const SVELTE_VERSION = '5.57.2';

function sveltePackage() {
	if (process.env.SVELTE_PKG) return process.env.SVELTE_PKG;
	const dir = path.join(root, 'node_modules/.pnpm');
	const hit = fs.readdirSync(dir).filter((d) => d.startsWith(`svelte@${SVELTE_VERSION}`)).sort();
	if (!hit.length) throw new Error(`no svelte@${SVELTE_VERSION} in ${dir}`);
	return path.join(dir, hit[0], 'node_modules/svelte');
}

const pkgDir = sveltePackage();
const pkg = JSON.parse(fs.readFileSync(path.join(pkgDir, 'package.json'), 'utf8'));
if (pkg.version !== SVELTE_VERSION) throw new Error(`svelte ${pkg.version}, expected ${SVELTE_VERSION}`);
const { compile, parse } = await import(pathToFileURL(path.join(pkgDir, 'src/compiler/index.js')).href);
// the acorn svelte itself resolves (a dependency of the package)
const acorn = createRequire(path.join(pkgDir, 'package.json'))('acorn');

function walk(dir, pred, acc = []) {
	if (!fs.existsSync(dir) || !fs.statSync(dir).isDirectory()) return acc;
	for (const ent of fs.readdirSync(dir, { withFileTypes: true }).sort((a, b) => (a.name < b.name ? -1 : 1))) {
		const p = path.join(dir, ent.name);
		if (ent.isDirectory()) {
			if (ent.name !== 'node_modules' && ent.name !== '_output') walk(p, pred, acc);
		} else if (pred(p)) acc.push(p);
	}
	return acc;
}

/** `_config.js` of a css sample: only `compileOptions` matters here. */
async function loadConfig(dir) {
	const f = path.join(dir, '_config.js');
	if (!fs.existsSync(f)) return {};
	const code = fs
		.readFileSync(f, 'utf8')
		.replace(/^import .*$/gm, '')
		.replace(/export default test\(/, 'export default (');
	const mod = await import('data:text/javascript,' + encodeURIComponent(code));
	return mod.default ?? {};
}

function stripLink(message) {
	const lines = message.split('\n');
	if (lines.at(-1)?.startsWith('https://svelte.dev/e/')) lines.pop();
	return lines.join('\n');
}

function isCssError(e) {
	return typeof e?.code === 'string' && (e.code.startsWith('css_') || e.code === 'unexpected_eof');
}

/** The `$$css.code` string of a client compile with `css: 'injected'`. */
function injectedCode(js) {
	const ast = acorn.parse(js, { ecmaVersion: 'latest', sourceType: 'module' });
	for (const node of ast.body) {
		if (node.type !== 'VariableDeclaration') continue;
		const d = node.declarations[0];
		if (d.id.name !== '$$css') continue;
		const p = d.init.properties.find((p) => p.key.name === 'code');
		if (p.value.type === 'Literal') return p.value.value;
		if (p.value.type === 'TemplateLiteral' && p.value.expressions.length === 0) return p.value.quasis[0].value.cooked;
		throw new Error(`$$css.code is a ${p.value.type}`);
	}
	return null;
}

function run(source, options) {
	const seen = {};
	const cssHash = options.cssHash ?? (() => 'svelte-xyz');
	const opts = {
		...options,
		generate: 'client',
		cssHash: (args) => {
			seen.args = { css: args.css, filename: args.filename ?? null, name: args.name };
			seen.default_css = `svelte-${args.hash(args.css)}`;
			// validate-options.js's default cssHash
			seen.default_filename = `svelte-${args.hash(args.filename === '(unknown)' ? args.css : (args.filename ?? args.css))}`;
			return (seen.hash = cssHash(args));
		}
	};
	const rec = {};
	try {
		const r = compile(source, { ...opts, css: 'external' });
		rec.external = {
			code: r.css?.code ?? null,
			hasGlobal: r.css?.hasGlobal ?? null,
			warnings: r.warnings
				.filter((w) => w.code.startsWith('css_'))
				.map((w) => ({ code: w.code, message: stripLink(w.message), start: w.position?.[0] ?? null, end: w.position?.[1] ?? null }))
		};
	} catch (e) {
		if (!isCssError(e)) return { skip: `${e.code ?? 'crash'}: ${String(e.message).split('\n')[0]}` };
		rec.error = { code: e.code, message: stripLink(e.message), start: e.position?.[0] ?? null, end: e.position?.[1] ?? null };
		return rec;
	}
	if (!options.dev) {
		try {
			const r = compile(source, { ...opts, css: 'injected' });
			rec.injected = injectedCode(r.js.code);
		} catch (e) {
			rec.injected_error = e.code ?? `crash: ${String(e.message).split('\n')[0]}`;
		}
	}
	rec.hash = seen.hash ?? null;
	rec.hash_args = seen.args ?? null;
	rec.default_hash_css = seen.default_css ?? null;
	rec.default_hash_filename = seen.default_filename ?? null;
	return rec;
}

const cases = [];
const skipped = [];

function add(id, source, options) {
	if (!/<style[\s>]/.test(source)) return;
	const rec = run(source, options);
	if (rec.skip) {
		skipped.push(`${id}: ${rec.skip}`);
		return;
	}
	const recorded = {};
	for (const [k, v] of Object.entries(options)) recorded[k] = typeof v === 'function' ? '[function]' : v;
	cases.push({ id, source, options: recorded, ...rec });
}

const tests = path.join(upstream, 'svelte/packages/svelte/tests');
if (!fs.existsSync(tests)) throw new Error(`${tests} missing: clone sveltejs/svelte @ svelte@${SVELTE_VERSION} into $UPSTREAM/svelte`);

// 1. the css samples, with their compileOptions
const cssSamples = path.join(tests, 'css/samples');
for (const name of fs.readdirSync(cssSamples).sort()) {
	const dir = path.join(cssSamples, name);
	const config = await loadConfig(dir);
	const options = { ...(config.compileOptions ?? {}) };
	for (const f of walk(dir, (p) => p.endsWith('.svelte'))) {
		add(`css/${name}/${path.relative(dir, f)}`, fs.readFileSync(f, 'utf8'), options);
	}
}

// 2. every other upstream sample with a <style>, default options, plus the same with a filename
for (const suite of fs.readdirSync(tests).sort()) {
	if (suite === 'css') continue;
	for (const f of walk(path.join(tests, suite), (p) => p.endsWith('.svelte'))) {
		add(`${suite}/${path.relative(path.join(tests, suite), f)}`, fs.readFileSync(f, 'utf8'), {});
	}
}

/** Native `bind:` directives blanked with spaces, as the Vite plugin's `withoutBindings` does. */
function withoutBindings(src) {
	const ranges = [];
	const visit = (n) => {
		if (Array.isArray(n)) for (const x of n) visit(x);
		else if (n && typeof n === 'object') {
			if (n.type === 'BindDirective' && typeof n.start === 'number') ranges.push([n.start, n.end]);
			for (const [k, v] of Object.entries(n)) if (k !== 'instance' && k !== 'module' && k !== 'css') visit(v);
		}
	};
	visit(parse(src, { modern: true }).fragment);
	let out = src;
	for (const [a, b] of ranges.sort((x, y) => y[0] - x[0])) out = out.slice(0, a) + ' '.repeat(b - a) + out.slice(b);
	return out;
}

// 3. this repo's components (as written, or with their native bindings blanked when Svelte
// rejects them; the Vite plugin also lifts `@native` blocks first)
for (const sub of ['fixtures', 'examples']) {
	for (const f of walk(path.join(root, sub), (p) => p.endsWith('.svelte'))) {
		const rel = path.relative(root, f);
		const src = fs.readFileSync(f, 'utf8');
		if (!/<style[\s>]/.test(src)) continue;
		const probe = run(src, { filename: rel });
		add(`repo/${rel}`, /^bind_/.test(probe.skip ?? '') ? withoutBindings(src) : src, { filename: rel });
	}
}

fs.mkdirSync(path.dirname(out), { recursive: true });
fs.writeFileSync(out, JSON.stringify({ svelte: SVELTE_VERSION, cases }, null, '\t') + '\n');
console.log(`${cases.length} cases (${cases.filter((c) => c.error).length} errors), ${skipped.length} skipped -> ${path.relative(root, out)}`);
for (const s of skipped) console.log(`  skipped ${s}`);
