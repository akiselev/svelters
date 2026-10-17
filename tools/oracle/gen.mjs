#!/usr/bin/env node
// Oracle corpus generator (X1, agent F). Runs the REAL acorn 8.19.0 and svelte/compiler 5.57.2
// over a fixed set of inputs and writes their output as JSONL under target/oracle/:
//
//   acorn.jsonl   { id, code, options, ast, comments } | { id, code, options, error: { message, pos, raisedAt } }
//   svelte.jsonl  { id, source, options, ast }         | { id, source, options, error: { code, message, start, end } }
//
// Any case may also carry `skip: "<reason>"` (input the Rust side cannot represent, e.g. a
// source with lone surrogates). Positions are UTF-16 offsets, exactly as upstream reports them.
//
// Usage: node tools/oracle/gen.mjs [acorn|svelte|all]   (env UPSTREAM=/path, default /tmp/claude-0/up)
// See tools/oracle/README.md.

import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import { createRequire } from 'node:module';
import { fileURLToPath, pathToFileURL } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, '../..');
const upstream = process.env.UPSTREAM || '/tmp/claude-0/up';
const outDir = path.join(root, 'target/oracle');

const ACORN_VERSION = '8.19.0';
const SVELTE_VERSION = '5.57.2';

// ---------------------------------------------------------------------------------------------
// locate the upstream packages in node_modules/.pnpm (neither is a direct root dependency)

function pnpmPackage(prefix, name) {
	const dir = path.join(root, 'node_modules/.pnpm');
	if (!fs.existsSync(dir)) throw new Error(`${dir} missing: run \`pnpm install --frozen-lockfile\` at the repo root`);
	const hit = fs.readdirSync(dir).filter((d) => d.startsWith(prefix)).sort();
	if (!hit.length) throw new Error(`no ${prefix}* in ${dir}`);
	return path.join(dir, hit[0], 'node_modules', name);
}

const require = createRequire(import.meta.url);
const acornDir = pnpmPackage(`acorn@${ACORN_VERSION}`, 'acorn');
const acorn = require(path.join(acornDir, 'dist/acorn.js'));
if (acorn.version !== ACORN_VERSION) throw new Error(`acorn ${acorn.version}, expected ${ACORN_VERSION}`);

async function loadSvelte() {
	const dir = pnpmPackage(`svelte@file+vendor+svelte+svelte-${SVELTE_VERSION}`, 'svelte');
	const pkg = JSON.parse(fs.readFileSync(path.join(dir, 'package.json'), 'utf8'));
	if (pkg.version !== SVELTE_VERSION) throw new Error(`svelte ${pkg.version}, expected ${SVELTE_VERSION}`);
	return import(pathToFileURL(path.join(dir, 'src/compiler/index.js')).href);
}

// ---------------------------------------------------------------------------------------------
// JSON helpers

/** Is `s` free of lone surrogates (representable as a Rust String)? */
const wellFormed = (s) => s.isWellFormed();

/** JSON.stringify replacer: BigInt -> null (acorn's `value` for BigInt literals; Rust serializes
 *  `LiteralValue::BigInt` as null), lone surrogates -> U+FFFD (Rust strings cannot hold them),
 *  `loc`/`range` dropped (the comparator ignores them anyway; keeps the corpus small). */
function replacer(key, value) {
	if (key === 'loc' || key === 'range') return undefined;
	if (typeof value === 'bigint') return null;
	if (typeof value === 'string' && !value.isWellFormed()) return value.toWellFormed();
	return value;
}
const line = (obj) => JSON.stringify(obj, replacer) + '\n';

function walk(dir, pred, out = []) {
	if (!fs.existsSync(dir)) return out;
	for (const ent of fs.readdirSync(dir, { withFileTypes: true }).sort((a, b) => (a.name < b.name ? -1 : a.name > b.name ? 1 : 0))) {
		const p = path.join(dir, ent.name);
		if (ent.isDirectory()) {
			if (ent.name !== 'node_modules' && ent.name !== '_output') walk(p, pred, out);
		} else if (pred(p)) out.push(p);
	}
	return out;
}

function writeJsonl(name, cases) {
	fs.mkdirSync(outDir, { recursive: true });
	const file = path.join(outDir, name);
	const tmp = file + '.tmp';
	const fd = fs.openSync(tmp, 'w');
	let bytes = 0;
	for (const c of cases) {
		const s = line(c);
		bytes += Buffer.byteLength(s);
		fs.writeSync(fd, s);
	}
	fs.closeSync(fd);
	fs.renameSync(tmp, file);
	return { file, bytes };
}

// ---------------------------------------------------------------------------------------------
// acorn corpus

/** Options as recorded: JSON-safe copy (callbacks and token arrays replaced by markers). */
function recordOptions(opts) {
	const out = {};
	for (const [k, v] of Object.entries(opts)) {
		if (typeof v === 'function') out[k] = '[function]';
		else if (k === 'onComment' || k === 'onToken') out[k] = Array.isArray(v) ? '[array]' : String(v);
		else if (k === 'program') out[k] = '[node]';
		else out[k] = v;
	}
	return out;
}

/** Runs real acorn. `onComment`/`onToken` are replaced by our own collector (they never change
 *  the AST); `locations`/`ranges` are turned off (they only add `loc`/`range`, which are ignored). */
function runAcorn(code, opts) {
	const run = { ...opts, locations: false, ranges: false };
	delete run.onToken;
	delete run.onInsertedSemicolon;
	delete run.onTrailingComma;
	delete run.sourceFile;
	delete run.directSourceFile;
	const comments = [];
	run.onComment = (block, value, start, end) => comments.push({ type: block ? 'Block' : 'Line', value, start, end });
	try {
		const ast = acorn.parse(code, run);
		return { ast, comments };
	} catch (e) {
		if (!(e instanceof SyntaxError) || typeof e.pos !== 'number') {
			return { error: { message: String(e && e.message), pos: null, raisedAt: null, crash: true } };
		}
		return { error: { message: e.message, pos: e.pos, raisedAt: e.raisedAt } };
	}
}

function acornTestCases() {
	const testDir = path.join(upstream, 'acorn/test');
	// Same file order as acorn's test/run.js.
	const runJs = fs.readFileSync(path.join(testDir, 'run.js'), 'utf8');
	const files = [...runJs.matchAll(/require\("\.\/(tests[^"]*\.js)"\)/g)].map((m) => m[1]);
	if (!files.length) throw new Error('no tests*.js listed in acorn/test/run.js');
	const extra = fs.readdirSync(testDir).filter((f) => /^tests.*\.js$/.test(f) && !files.includes(f)).sort();
	const cases = [];
	for (const file of [...files, ...extra]) {
		const collected = [];
		const driver = {
			test: (code, ast, options) => collected.push({ kind: 'test', code, options }),
			testFail: (code, message, options) => collected.push({ kind: 'testFail', code, options }),
			testAssert: (code, assert, options) => collected.push({ kind: 'testAssert', code, options }),
		};
		const fakeRequire = (p) => {
			if (p === './driver.js') return driver;
			if (p === '../acorn' || p === '../acorn/dist/acorn.js') return acorn;
			if (p === '../acorn-loose') return {};
			throw new Error(`${file}: unexpected require(${p})`);
		};
		const ctx = vm.createContext({ exports: {}, require: fakeRequire, ...driver, acorn, console });
		vm.runInContext(fs.readFileSync(path.join(testDir, file), 'utf8'), ctx, { filename: file });
		collected.forEach((t, i) => {
			// Defaults of acorn's test/driver.js runTests.
			const opts = { ...(t.options || { locations: true }) };
			if (!opts.ecmaVersion) opts.ecmaVersion = 5;
			const id = `acorn-test/${file}#${i}`;
			const c = { id, code: t.code, options: recordOptions(opts) };
			if (typeof t.code !== 'string') return;
			if (!wellFormed(t.code)) c.skip = 'source contains lone surrogates';
			Object.assign(c, runAcorn(t.code, opts));
			cases.push(c);
		});
	}
	return cases;
}

function acornFileCases() {
	const sources = [
		['svelte', path.join(upstream, 'svelte'), 'packages/svelte/src'],
		['acorn', path.join(upstream, 'acorn'), 'acorn/src'],
	];
	const cases = [];
	for (const [tag, base, sub] of sources) {
		for (const f of walk(path.join(base, sub), (p) => p.endsWith('.js'))) {
			const code = fs.readFileSync(f, 'utf8');
			const opts = { ecmaVersion: 16, sourceType: 'module' };
			cases.push({ id: `${tag}:${path.relative(base, f)}`, code, options: opts, ...runAcorn(code, opts) });
		}
	}
	return cases;
}

// ---------------------------------------------------------------------------------------------
// svelte corpus

async function svelteCases() {
	const { parse } = await loadSvelte();
	const testsDir = path.join(upstream, 'svelte/packages/svelte/tests');
	const suites = ['parser-modern', 'parser-legacy', 'compiler-errors', 'runtime-runes', 'runtime-legacy', 'validator'];
	const inputs = [];
	for (const s of suites) {
		for (const f of walk(path.join(testsDir, s, 'samples'), (p) => p.endsWith('.svelte'))) {
			const rel = path.relative(path.join(testsDir, s, 'samples'), f);
			const loose = s === 'parser-modern' && rel.split(path.sep)[0].startsWith('loose-');
			inputs.push({ id: `svelte-tests/${s}/${rel}`, file: f, loose });
		}
	}
	for (const f of walk(path.join(root, 'fixtures'), (p) => p.endsWith('.svelte') && path.basename(path.dirname(p)) === 'src' && path.dirname(path.dirname(path.dirname(p))) === path.join(root, 'fixtures'))) {
		inputs.push({ id: `svelters/${path.relative(root, f)}`, file: f, loose: false });
	}
	for (const f of walk(path.join(root, 'tests/renderer-conformance/apps'), (p) => p.endsWith('.svelte') && path.dirname(p) === path.join(root, 'tests/renderer-conformance/apps'))) {
		inputs.push({ id: `svelters/${path.relative(root, f)}`, file: f, loose: false });
	}
	const cases = [];
	for (const { id, file, loose } of inputs) {
		const source = fs.readFileSync(file, 'utf8');
		const options = { modern: true, loose };
		const c = { id, source, options };
		if (!wellFormed(source)) c.skip = 'source contains lone surrogates';
		try {
			c.ast = parse(source, { modern: true, loose });
		} catch (e) {
			if (e && e.name === 'CompileError' && Array.isArray(e.position)) {
				// Svelte appends the docs URL to the message; the Rust CompileError does not carry it.
				const message = e.message.replace(new RegExp(`\\nhttps://svelte\\.dev/e/${e.code}$`), '');
				c.error = { code: e.code, message, start: e.position[0], end: e.position[1] };
			} else {
				c.error = { code: null, message: String(e && e.message), start: null, end: null, crash: true };
			}
		}
		cases.push(c);
	}
	return cases;
}

// ---------------------------------------------------------------------------------------------

function summary(name, cases, { bytes, file }) {
	const err = cases.filter((c) => c.error).length;
	const skip = cases.filter((c) => c.skip).length;
	console.log(`${name}: ${cases.length} cases (${cases.length - err} ok, ${err} errors, ${skip} skip), ${(bytes / 1048576).toFixed(1)} MB -> ${path.relative(root, file)}`);
}

const which = process.argv[2] || 'all';
if (which === 'acorn' || which === 'all') {
	const cases = [...acornTestCases(), ...acornFileCases()];
	summary('acorn', cases, writeJsonl('acorn.jsonl', cases));
}
if (which === 'svelte' || which === 'all') {
	const cases = await svelteCases();
	summary('svelte', cases, writeJsonl('svelte.jsonl', cases));
}
