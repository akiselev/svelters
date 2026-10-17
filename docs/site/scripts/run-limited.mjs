// Run a command limited to at most 4 CPU cores (taskset where available) and 4 worker threads.
import { spawnSync } from 'node:child_process';
import os from 'node:os';

const [cmd, ...args] = process.argv.slice(2);
const env = { ...process.env, UV_THREADPOOL_SIZE: '4', RAYON_NUM_THREADS: '4' };
const cpus = os.availableParallelism();
let r;
if (process.platform === 'linux' && cpus > 4) {
  r = spawnSync('taskset', ['-c', '0-3', cmd, ...args], { stdio: 'inherit', env });
  if (r.error && r.error.code === 'ENOENT') r = spawnSync(cmd, args, { stdio: 'inherit', env });
} else {
  r = spawnSync(cmd, args, { stdio: 'inherit', env });
}
process.exit(r.status ?? 1);
