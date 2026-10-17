#!/usr/bin/env node
// Prints the TestHost op log of a gallery fixture opening every page (what check-pages.mjs runs),
// for writing `style-setters.expected`. Usage: node fixtures/gallery-common/dump-log.mjs <fixtureDir>
import path from 'node:path';
import { runBundle } from '../../packages/testhost/src/index.js';
import { pages, SIDEBAR_FIRST_HANDLE } from './pages.mjs';

const dir = path.resolve(process.argv[2] ?? '.');
const tk = path.basename(dir).replace(/^gallery-/, '');
const script = [{ checkpoint: true }];
pages(dir).forEach((_, i) => script.push({ event: [SIDEBAR_FIRST_HANDLE + i, 'clicked', []] }, { checkpoint: true }));
process.stdout.write(await runBundle({ bundlePath: path.join(dir, 'dist/app.js'), script, styleDialect: tk === 'qt' ? 'qss' : 'css' }));
