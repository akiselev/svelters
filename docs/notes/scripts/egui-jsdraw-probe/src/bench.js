// Workloads for the egui-jsdraw probe. Every frame_* function emits `n` 8x8 rects on a 100-wide grid,
// the same scene the Rust baselines produce. Host functions installed by main.rs:
//   rect(x, y, w, h, rgba)        one host call per draw op
//   painter.rect(...)             same, as a method on a host object
//   flush(arrayBuffer, words)     decode a command buffer (u32 opcode words + f32 args)
//   flush_json(string)            X8 status quo: JSON display list
//   flush_array(numbers[])        plain JS array converted to Vec<f64>

const COLOR = 0xff8040ff >>> 0;

function frame_call(n) {
  for (let i = 0; i < n; i++) {
    rect((i % 100) * 10, ((i / 100) | 0) * 10, 8, 8, COLOR);
  }
}

function frame_method(n) {
  const p = painter;
  for (let i = 0; i < n; i++) {
    p.rect((i % 100) * 10, ((i / 100) | 0) * 10, 8, 8, COLOR);
  }
}

// The loop and arithmetic alone, no draw call: the floor for any JS-side approach.
function frame_nocall(n) {
  let s = 0;
  for (let i = 0; i < n; i++) {
    s += (i % 100) * 10 + ((i / 100) | 0) * 10 + 8 + 8 + COLOR;
  }
  return s;
}

// An empty counted loop: the interpreter's per-iteration floor.
function frame_empty(n) {
  let s = 0;
  for (let i = 0; i < n; i++) s += i;
  return s;
}

// Command buffer. Layout per op: [opcode u32][args f32 | u32]. One ArrayBuffer, two views.
class Painter {
  constructor(words) {
    this.ab = new ArrayBuffer(words * 4);
    this.f = new Float32Array(this.ab);
    this.u = new Uint32Array(this.ab);
    this.k = 0;
  }
  grow() {
    const ab = new ArrayBuffer(this.ab.byteLength * 2);
    new Uint32Array(ab).set(this.u);
    this.ab = ab;
    this.f = new Float32Array(ab);
    this.u = new Uint32Array(ab);
  }
  rect(x, y, w, h, rgba) {
    let k = this.k;
    if (k + 6 > this.u.length) this.grow();
    const f = this.f;
    this.u[k] = 1;
    f[k + 1] = x;
    f[k + 2] = y;
    f[k + 3] = w;
    f[k + 4] = h;
    this.u[k + 5] = rgba;
    this.k = k + 6;
  }
  // polyline: [2][count][width f32][rgba] then count (x, y) f32 pairs
  beginLine(count, width, rgba) {
    let k = this.k;
    while (k + 4 + count * 2 > this.u.length) this.grow();
    this.u[k] = 2;
    this.u[k + 1] = count;
    this.f[k + 2] = width;
    this.u[k + 3] = rgba;
    this.k = k + 4;
  }
  pt(x, y) {
    const k = this.k;
    this.f[k] = x;
    this.f[k + 1] = y;
    this.k = k + 2;
  }
  // circle: [3][cx][cy][r][fill rgba]
  circle(cx, cy, r, rgba) {
    let k = this.k;
    if (k + 5 > this.u.length) this.grow();
    this.u[k] = 3;
    this.f[k + 1] = cx;
    this.f[k + 2] = cy;
    this.f[k + 3] = r;
    this.u[k + 4] = rgba;
    this.k = k + 5;
  }
  flush() {
    flush(this.ab, this.k);
    this.k = 0;
  }
}

const P = new Painter(6 * 1024);

// Inline buffer writes (what a compiler could emit), no method call per op.
const RAW = new Painter(6 * 100000);
function frame_buf_inline(n) {
  const u = RAW.u, f = RAW.f;
  let k = 0;
  for (let i = 0; i < n; i++) {
    u[k] = 1;
    f[k + 1] = (i % 100) * 10;
    f[k + 2] = ((i / 100) | 0) * 10;
    f[k + 3] = 8;
    f[k + 4] = 8;
    u[k + 5] = COLOR;
    k += 6;
  }
  flush(RAW.ab, k);
}

// The ergonomic API: painter.rect() is a JS method that encodes into the buffer.
function frame_buf_api(n) {
  const p = P;
  for (let i = 0; i < n; i++) {
    p.rect((i % 100) * 10, ((i / 100) | 0) * 10, 8, 8, COLOR);
  }
  p.flush();
}

// X8 today: [name, ...args] arrays, JSON.stringify, parsed natively.
function frame_json(n) {
  const ops = [];
  for (let i = 0; i < n; i++) {
    ops.push(['fillRect', (i % 100) * 10, ((i / 100) | 0) * 10, 8, 8, '#ff8040ff']);
  }
  flush_json(JSON.stringify(ops));
}

// A flat plain array of numbers converted by rquickjs (Vec<f64>).
function frame_array(n) {
  const a = [];
  for (let i = 0; i < n; i++) {
    a.push(1, (i % 100) * 10, ((i / 100) | 0) * 10, 8, 8, COLOR);
  }
  flush_array(a);
}

// ---- Realistic widgets -----------------------------------------------------------------------

const SPARK = new Float64Array(100000);
for (let i = 0; i < SPARK.length; i++) SPARK[i] = Math.sin(i * 0.05) * 10 + Math.sin(i * 0.31) * 3;

// Sparkline over the first n samples into a w x h box: min/max scan, then a polyline.
function frame_spark(n, x0, y0, w, h) {
  const d = SPARK;
  let mn = Infinity, mx = -Infinity;
  for (let i = 0; i < n; i++) {
    const v = d[i];
    if (v < mn) mn = v;
    if (v > mx) mx = v;
  }
  const sx = w / (n - 1), sy = h / (mx - mn || 1);
  const p = P;
  p.beginLine(n, 1.5, 0x4080ffff);
  for (let i = 0; i < n; i++) p.pt(x0 + i * sx, y0 + h - (d[i] - mn) * sy);
  p.flush();
}

// A knob: track arc (48 segments), value arc, thumb dot. Called once per knob per frame.
function knob(cx, cy, r, value) {
  const p = P;
  const a0 = Math.PI * 0.75, sweep = Math.PI * 1.5;
  const segs = 48;
  p.beginLine(segs + 1, 3, 0x404040ff);
  for (let i = 0; i <= segs; i++) {
    const a = a0 + (sweep * i) / segs;
    p.pt(cx + Math.cos(a) * r, cy + Math.sin(a) * r);
  }
  const vs = Math.max(1, Math.round(segs * value));
  p.beginLine(vs + 1, 3, 0x40a0ffff);
  for (let i = 0; i <= vs; i++) {
    const a = a0 + (sweep * value * i) / vs;
    p.pt(cx + Math.cos(a) * r, cy + Math.sin(a) * r);
  }
  const a = a0 + sweep * value;
  p.circle(cx + Math.cos(a) * r * 0.7, cy + Math.sin(a) * r * 0.7, r * 0.12, 0xffffffff);
}
function knob_flush() {
  P.flush();
}

// Per-widget invocation cost: Rust calls this once per widget per frame (3 rects each).
function paint_small(x, y, w, h, t) {
  const p = P;
  p.rect(x, y, w, h, 0x202020ff);
  p.rect(x + 1, y + 1, (w - 2) * t, h - 2, 0x40a0ffff);
  p.rect(x + w - 4, y, 4, h, 0xffffffff);
}
function paint_small_obj(r) {
  const p = P;
  p.rect(r.x, r.y, r.w, r.h, 0x202020ff);
  p.rect(r.x + 1, r.y + 1, (r.w - 2) * r.t, r.h - 2, 0x40a0ffff);
  p.rect(r.x + r.w - 4, r.y, 4, r.h, 0xffffffff);
}
function noop() {}
