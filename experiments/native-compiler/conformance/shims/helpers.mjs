// The parts of Svelte's tests/helpers.js that `_config.js` files import (Svelte 5.57.2 @ f3de3f2,
// MIT, copied verbatim apart from types).

export const async_mode = process.env.SVELTE_NO_ASYNC !== 'true';

export function normalise_inspect_logs(logs) {
  const normalised = [];
  for (const log of logs) {
    if (log === 'stack trace') continue;
    if (log instanceof Error) {
      const last_line = log.stack
        ?.trim()
        .split('\n')
        .filter((line) => !line.includes('at Module.get_stack'))[1];
      const match = last_line && /(at .+) /.exec(last_line);
      if (match) normalised.push(match[1]);
    } else {
      normalised.push(log);
    }
  }
  return normalised;
}

export function normalise_trace_logs(logs) {
  let normalised = [];
  logs = logs.slice();
  while (logs.length > 0) {
    const log = logs.shift();
    if (log instanceof Error) continue;
    if (typeof log === 'string' && log.includes('%c')) {
      const split = log.split('%c');
      const first = split.shift().trim();
      if (first) normalised.push({ log: first });
      while (split.length > 0) {
        const log = split.shift().trim();
        const highlighted = logs.shift() === 'color: CornflowerBlue; font-weight: bold';
        if (/\(.+ms\)/.test(log)) continue;
        normalised.push({ log, highlighted });
      }
    } else {
      normalised.push({ log });
    }
  }
  return normalised;
}
