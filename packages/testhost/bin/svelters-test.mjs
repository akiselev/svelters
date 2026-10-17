#!/usr/bin/env node
// Usage: node --experimental-vm-modules svelters-test.mjs <fixtureDir> [--update]
// Runs <fixtureDir>/dist/app.js against <fixtureDir>/script.json and compares the op log with
// <fixtureDir>/expected.oplog (byte for byte). --update (or UPDATE=1) rewrites expected.oplog.
// A fixture written in the Qt vocabulary says so in its package.json: `"svelters": { "toolkit": "qt" }`
// (Qt prop schema and QSS style dialect); the default is GTK.
import fs from 'node:fs';
import path from 'node:path';
import { runBundle } from '../src/index.js';

const args = process.argv.slice(2);
const update = args.includes('--update') || process.env.UPDATE === '1';
const dir = path.resolve(args.find((a) => !a.startsWith('--')) ?? '.');
const name = path.basename(dir);
const bundlePath = path.join(dir, 'dist', 'app.js');
const expectedPath = path.join(dir, 'expected.oplog');

const code = fs.readFileSync(bundlePath, 'utf8');
const forbidden = [
  [/['"]node:[a-z]/, 'node: specifier'],
  [/\brequire\s*\(/, 'require()'],
  [/\bprocess\s*\./, 'process.*'],
  [/\bBuffer\b/, 'Buffer'],
  [/^\s*(import|export)\s/m, 'top-level import/export statement'],
  [/\bimport\s*\(/, 'dynamic import()'],
];
for (const [re, what] of forbidden) {
  if (re.test(code)) {
    console.error(`FAIL ${name}: bundle contains ${what}`);
    process.exit(1);
  }
}

const script = JSON.parse(fs.readFileSync(path.join(dir, 'script.json'), 'utf8'));
const pkgPath = path.join(dir, 'package.json');
const toolkit = (fs.existsSync(pkgPath) && JSON.parse(fs.readFileSync(pkgPath, 'utf8')).svelters?.toolkit) || 'gtk';
const actual = await runBundle({ bundlePath, script, toolkit });

if (update) {
  fs.writeFileSync(expectedPath, actual);
  console.log(`UPDATED ${name} (${actual.split('\n').length - 1} ops)`);
} else {
  const expected = fs.readFileSync(expectedPath, 'utf8');
  if (expected !== actual) {
    console.error(`FAIL ${name}: op log differs from expected.oplog\n--- expected\n${expected}--- actual\n${actual}`);
    process.exit(1);
  }
  console.log(`PASS ${name} (${actual.split('\n').length - 1} ops)`);
}
