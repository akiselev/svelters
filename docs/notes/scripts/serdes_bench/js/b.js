// Part B: prototype bindings (ids pre-resolved, typed record conversion, batching).
const B = globalThis.B;
const N = 10000;
function bench(name, fn) {
  let best = 1e9;
  for (let r = 0; r < 9; r++) { const t0 = B.now(); fn(); const t = B.now() - t0; if (t < best) best = t; }
  B.log(`B\t${name}\t${(best * 1e6 / N).toFixed(0)}`);
}
const h = 1, P_LABEL = 1, P_SPACING = 2, P_GEOM = 3, P_ORIENT = 4;
bench('js loop only', () => { let s = 0; for (let i = 0; i < N; i++) s += i; });
bench('js: object literal {x,y,width,height}', () => { let o; for (let i = 0; i < N; i++) o = { x: i, y: 2, width: 300, height: 400 }; });
const tw = new Float64Array(8);
bench('js: 4 Float64Array stores', () => { for (let i = 0; i < N; i++) { tw[0] = i; tw[1] = 2; tw[2] = 300; tw[3] = 400; } });
bench('noop native call (h,id,v)', () => { for (let i = 0; i < N; i++) B.noop(h, P_SPACING, i); });
bench('setPropId int (generic conv)', () => { for (let i = 0; i < N; i++) B.setPropId(h, P_SPACING, i); });
bench('setPropId str 11B (generic conv)', () => { for (let i = 0; i < N; i++) B.setPropId(h, P_LABEL, 'hello world'); });
bench('setPropId enum as number', () => { for (let i = 0; i < N; i++) B.setPropId(h, P_ORIENT, 1); });
bench('setPropId rect LIST [x,y,w,h]', () => { for (let i = 0; i < N; i++) B.setPropId(h, P_GEOM, [i, 2, 300, 400]); });
bench('setPropId rect OBJECT {x,y,width,height} (typed record)', () => { for (let i = 0; i < N; i++) B.setPropId(h, P_GEOM, { x: i, y: 2, width: 300, height: 400 }); });
const pre = { x: 1, y: 2, width: 300, height: 400 };
bench('setPropId rect OBJECT preallocated', () => { for (let i = 0; i < N; i++) B.setPropId(h, P_GEOM, pre); });
const prx = new Proxy({ x: 1, y: 2, width: 300, height: 400 }, { get(t, k, r) { return Reflect.get(t, k, r); } });
bench('setPropId rect OBJECT behind a Proxy (like Svelte $state)', () => { for (let i = 0; i < N; i++) B.setPropId(h, P_GEOM, prx); });
bench('setRect4 flat args (h,id,x,y,w,h)', () => { for (let i = 0; i < N; i++) B.setRect4(h, P_GEOM, i, 2, 300, 400); });

// Batched: ops encoded into a Float64Array; strings in a side array; one native call per flush.
const CAP = 1 << 16;
const buf = new Float64Array(CAP);
let n = 0; let strs = [];
const OP_INT = 1, OP_STR = 2, OP_RECT = 3, OP_F64 = 4;
function flush() { if (n) { B.flush(buf.buffer, n, strs); n = 0; strs = []; } }
function setInt(h, id, v) { if (n + 4 > CAP) flush(); buf[n++] = OP_INT; buf[n++] = h; buf[n++] = id; buf[n++] = v; }
function setStr(h, id, s) { if (n + 4 > CAP) flush(); buf[n++] = OP_STR; buf[n++] = h; buf[n++] = id; buf[n++] = strs.push(s) - 1; }
function setRect(h, id, r) { if (n + 7 > CAP) flush(); buf[n++] = OP_RECT; buf[n++] = h; buf[n++] = id; buf[n++] = r.x; buf[n++] = r.y; buf[n++] = r.width; buf[n++] = r.height; }
bench('batched int (encode+flush+decode+dispatch)', () => { for (let i = 0; i < N; i++) setInt(h, P_SPACING, i); flush(); });
bench('batched str 11B', () => { for (let i = 0; i < N; i++) setStr(h, P_LABEL, 'hello world'); flush(); });
bench('batched rect object', () => { for (let i = 0; i < N; i++) setRect(h, P_GEOM, { x: i, y: 2, width: 300, height: 400 }); flush(); });
bench('batched mixed (int,str,rect)/3', () => { for (let i = 0; i < N; i += 3) { setInt(h, P_SPACING, i); setStr(h, P_LABEL, 'hello world'); setRect(h, P_GEOM, pre); } flush(); });
// Binary string encoding inside the batch (UTF-16 code units copied into the Float64 stream is silly; use a Uint16Array side buffer)
const sbuf = new Uint16Array(1 << 20); let sn = 0;
function setStr16(h, id, s) { if (n + 5 > CAP || sn + s.length > sbuf.length) { flush16(); } buf[n++] = 5; buf[n++] = h; buf[n++] = id; buf[n++] = sn; buf[n++] = s.length; for (let k = 0; k < s.length; k++) sbuf[sn++] = s.charCodeAt(k); }
function flush16() { if (n) { B.flush16(buf.buffer, n, sbuf.buffer, sn); n = 0; sn = 0; } }
bench('batched str 11B as UTF-16 code units in a Uint16Array', () => { for (let i = 0; i < N; i++) setStr16(h, P_LABEL, 'hello world'); flush16(); });
// Callback registration cost: JS function -> persistent native root
bench('register+unregister closure (Persistent)', () => { for (let i = 0; i < N; i++) { const id = B.regFn(() => i); B.unregFn(id); } });
const S11 = 'hello world', KB = 'x'.repeat(1024), KBL = 'é'.repeat(1024), KBU = 'é'.repeat(512) + '✓'.repeat(512);
for (const [nm, s] of [['11B ascii', S11], ['1Ki ascii', KB], ['1Ki latin1', KBL], ['1Ki non-latin1 (wide)', KBU]]) {
  bench(`str ${nm}: String (to_string, current)`, () => { for (let i = 0; i < N; i++) B.strRust(s); });
  bench(`str ${nm}: borrowed UTF-8 (JS_ToCStringLen2)`, () => { for (let i = 0; i < N; i++) B.strUtf8(s); });
  bench(`str ${nm}: borrowed UTF-16 (JS_ToCStringLenUTF16)`, () => { for (let i = 0; i < N; i++) B.strUtf16(s); });
}
