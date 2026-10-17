// `pnpm test` entry: compiles and runs the cases when a `svelters-compile` binary exists (the
// cargo nextest gate builds it), else skips with a message. `pnpm run test:compiler` builds the
// compiler through tools/dev/cargo-slot first and never skips.
import { execFileSync } from 'node:child_process';
import { compilerPath } from '@svelters/native-runtime/vite';

try {
  compilerPath();
} catch (e) {
  console.log(`SKIP compiled experiment: ${e.message}; \`pnpm run test:compiler\` builds it through tools/dev/cargo-slot`);
  process.exit(0);
}
const run = (args) => execFileSync(process.execPath, args, { cwd: import.meta.dirname, stdio: 'inherit' });
try {
  run(['build.mjs']);
  run(['--experimental-vm-modules', '--no-warnings', 'run.mjs']);
} catch (e) {
  process.exit(e.status ?? 1);
}
