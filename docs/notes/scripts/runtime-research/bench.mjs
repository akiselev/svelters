// Probe (runtime research): signal workloads shaped like svelters apps, run against an adapter
// module (svelte-signals.mjs = Svelte 5.57.2's own signals, mini-signals.min.mjs = the minimal core).
//   qjs bench.mjs <adapter.mjs>      (QuickJS-NG 0.16.2, the version rquickjs 0.14 embeds)
//   node bench.mjs <adapter.mjs>
// Prints one line per workload: best-of-5 milliseconds.
const arg = typeof scriptArgs !== 'undefined' ? scriptArgs[1] : process.argv[2];
const L = await import(arg.startsWith('/') || arg.startsWith('.') ? arg : './' + arg);
const now = typeof performance !== 'undefined' ? () => performance.now() : () => Date.now();
const out = typeof print !== 'undefined' ? print : console.log;

function time(name, setup, rounds = 5) {
  let best = Infinity, check;
  for (let r = 0; r < rounds; r++) {
    const fn = setup();
    const t0 = now();
    check = fn();
    best = Math.min(best, now() - t0);
  }
  out(`${L.name}\t${name}\t${best.toFixed(2)} ms\t(check ${check})`);
}

// A: build 5k (signal -> derived -> effect) triples, as a list of rows mounting
time('create 5k sig+derived+effect', () => () => {
  let n = 0;
  L.root(() => {
    for (let i = 0; i < 5000; i++) {
      const s = L.signal(i);
      const d = L.computed(() => L.read(s) * 2);
      L.effect(() => { n += L.read(d); });
    }
  });
  return n;
});

// B: one source, 1000 effects; 200 write+flush rounds (a theme flag read by every widget)
time('fanout 1x1000, 200 updates', () => {
  const s = L.signal(0);
  let n = 0;
  L.root(() => { for (let i = 0; i < 1000; i++) L.effect(() => { n += L.read(s); }); });
  return () => { for (let r = 1; r <= 200; r++) { L.write(s, r); L.flush(); } return n; };
});

// C: chain of 100 deriveds (500 overflows qjs's default stack in both cores), one effect; 500 rounds
time('chain 100, 500 updates', () => {
  const s = L.signal(0);
  let d = s, n = 0;
  L.root(() => {
    for (let i = 0; i < 100; i++) { const p = d; d = L.computed(() => L.read(p) + 1); }
    const last = d;
    L.effect(() => { n = L.read(last); });
  });
  return () => { for (let r = 1; r <= 500; r++) { L.write(s, r); L.flush(); } return n; };
});

// D: 1000 independent counters with one template effect each; 2000 rounds of 5 writes + flush
// (the common UI case: small updates, many idle effects)
time('1000 counters, 2000x5 writes', () => {
  const ss = [];
  let n = 0;
  L.root(() => {
    for (let i = 0; i < 1000; i++) { const s = L.signal(0); ss.push(s); L.effect(() => { n += L.read(s); }); }
  });
  return () => {
    let seed = 1;
    for (let r = 1; r <= 2000; r++) {
      for (let k = 0; k < 5; k++) { seed = (seed * 1103515245 + 12345) & 0x7fffffff; const s = ss[seed % 1000]; L.write(s, L.read(s) + 1); }
      L.flush();
    }
    return n;
  };
});

// E: source -> 200 deriveds (parity, mostly unchanged) -> 200 effects; 500 rounds (MAYBE_DIRTY cut-off)
time('200 equal-cutoff deriveds, 500 updates', () => {
  const s = L.signal(0);
  let n = 0;
  L.root(() => {
    for (let i = 0; i < 200; i++) {
      const d = L.computed(() => (L.read(s) >> 4) + i);
      L.effect(() => { n += L.read(d); });
    }
  });
  return () => { for (let r = 1; r <= 500; r++) { L.write(s, r); L.flush(); } return n; };
});

// F: 500 effects with dynamic deps (read a or b by a toggle); 500 rounds toggling and writing
time('dynamic deps 500, 500 updates', () => {
  const t = L.signal(true), a = L.signal(0), b = L.signal(0);
  let n = 0;
  L.root(() => { for (let i = 0; i < 500; i++) L.effect(() => { n += L.read(t) ? L.read(a) : L.read(b); }); });
  return () => {
    for (let r = 1; r <= 500; r++) { L.write(t, r % 2 === 0); L.write(r % 2 ? b : a, r); L.flush(); }
    return n;
  };
});

// G: per-flush floor: one source, one effect, 20000 write+flush rounds (one click -> one label)
time('1 counter, 20000 updates', () => {
  const s = L.signal(0);
  let n = 0;
  L.root(() => { L.effect(() => { n = L.read(s); }); });
  return () => { for (let r = 1; r <= 20000; r++) { L.write(s, r); L.flush(); } return n; };
});
