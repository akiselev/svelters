const h = globalThis.__svelters_host;
const t = h.create('thing');
const check = (c, m) => {
  if (!c) throw new Error(m);
};
h.setProp(t, 'geometry', { x: 1, y: 2, width: 30, height: 40, extra: 'ignored' });
// A Proxy (what a Svelte $state object is) reads like a plain object, once per field.
let reads = 0;
const proxied = new Proxy(
  { x: 1, y: 2, width: 30, height: 40 },
  {
    get(o, k) {
      reads++;
      return o[k];
    },
  },
);
h.setProp(t, 'geometry', proxied);
check(reads <= 8, `fields read at most twice each, got ${reads} reads`);
const g = h.getProp(t, 'geometry');
check(g.x === 1 && g.width === 30 && Object.keys(g).join() === 'x,y,width,height', JSON.stringify(g));
h.setProp(t, 'data', new Uint8Array([1, 2, 3, 4]));
const d = h.getProp(t, 'data');
check(d instanceof Uint8Array && d.length === 4 && d[3] === 4, 'bytes');
const sub = new Uint8Array([9, 9, 1, 2, 3, 4, 9]).subarray(2, 6);
h.setProp(t, 'data', sub);
check(h.getProp(t, 'data').join() === '1,2,3,4', 'typed array view honours its offset');
h.setProp(t, 'tags', ['a', 'bé']);
check(h.getProp(t, 'tags').join() === 'a,bé', 'tags');
h.setProp(t, 'when', new Date(1700000000000));
const w = h.getProp(t, 'when');
check(w instanceof Date && w.getTime() === 1700000000000, 'date');
h.setProp(t, 'when', 1700000000000);
h.setProp(t, 'color', { red: 1, green: 0, blue: 0, alpha: 1 });
h.setProp(t, 'color', '#ff8800');
h.setProp(t, 'loose', [1, 'x']);
h.setProp(t, 'orientation', 'vertical');
check(h.getProp(t, 'orientation') === 'vertical', 'enum comes back as its name');
h.setProp(t, 'align', ['left', 'top']);
check(h.getProp(t, 'align').join() === 'left,top', 'flags come back as names');
h.setProp(t, 'align', []);
check(h.getProp(t, 'align').length === 0, 'empty flags');
check(h.call(t, 'setBoth', [3, 'x']) === true, 'method result');
check(h.call(t, 'setBoth', [3]) === true, 'optional trailing argument');
check(h.call(t, 'bounds', []).height === 4, 'record result');
check(h.call(t, 'raw', [1, 'two', [3], null]).length === 4, 'untyped method');
let bad = 0;
try {
  h.setProp(t, 'geometry', {
    x: 1,
    y: 2,
    width: 3,
    get height() {
      throw new RangeError('getter ran');
    },
  });
} catch (e) {
  bad = e.message === 'getter ran' ? 1 : 0;
}
check(bad === 1, 'exceptions from getters propagate');
console.log('ok');
