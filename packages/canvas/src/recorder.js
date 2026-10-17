// Canvas2D-like recorder. Every call appends `[name, ...args]` to a display list; `toJSON()` is the
// string the native canvas takes as its `ops` property. No state is read back: the recorder is write-only,
// so a list can be built anywhere (an effect, a worker-like function) and replayed natively.

const STATE_PROPS = ['fillStyle', 'strokeStyle', 'lineWidth', 'lineCap', 'lineJoin', 'globalAlpha', 'font', 'textAlign', 'textBaseline'];

export class Canvas2D {
  /** @type {Array<Array<string|number|boolean|number[]>>} */
  ops = [];
  #state = {};

  constructor(width = 0, height = 0) {
    this.width = width;
    this.height = height;
  }

  #op(...a) {
    this.ops.push(a);
  }

  // state
  save() { this.#op('save'); }
  restore() { this.#op('restore'); }
  translate(x, y) { this.#op('translate', x, y); }
  rotate(angle) { this.#op('rotate', angle); }
  scale(x, y) { this.#op('scale', x, y); }
  transform(a, b, c, d, e, f) { this.#op('transform', a, b, c, d, e, f); }
  setTransform(a, b, c, d, e, f) { this.#op('setTransform', a, b, c, d, e, f); }
  setLineDash(segments) { this.#op('setLineDash', Array.from(segments)); }

  // paths
  beginPath() { this.#op('beginPath'); }
  closePath() { this.#op('closePath'); }
  moveTo(x, y) { this.#op('moveTo', x, y); }
  lineTo(x, y) { this.#op('lineTo', x, y); }
  bezierCurveTo(c1x, c1y, c2x, c2y, x, y) { this.#op('bezierCurveTo', c1x, c1y, c2x, c2y, x, y); }
  quadraticCurveTo(cx, cy, x, y) { this.#op('quadraticCurveTo', cx, cy, x, y); }
  arc(x, y, r, start, end, counterclockwise = false) { this.#op('arc', x, y, r, start, end, !!counterclockwise); }
  rect(x, y, w, h) { this.#op('rect', x, y, w, h); }

  // painting
  fill() { this.#op('fill'); }
  stroke() { this.#op('stroke'); }
  clip() { this.#op('clip'); }
  fillRect(x, y, w, h) { this.#op('fillRect', x, y, w, h); }
  strokeRect(x, y, w, h) { this.#op('strokeRect', x, y, w, h); }
  clearRect(x, y, w, h) { this.#op('clearRect', x, y, w, h); }
  fillText(text, x, y) { this.#op('fillText', String(text), x, y); }
  strokeText(text, x, y) { this.#op('strokeText', String(text), x, y); }

  /** Forgets everything recorded so far. */
  clear() {
    this.ops.length = 0;
  }

  /** The display list as the native `ops` value. */
  toJSON() {
    return JSON.stringify(this.ops);
  }
}

// Style properties are plain accessors that record an op when set (as on CanvasRenderingContext2D).
for (const name of STATE_PROPS) {
  Object.defineProperty(Canvas2D.prototype, name, {
    get() {
      return this._get(name);
    },
    set(v) {
      this._set(name, v);
    },
  });
}

Canvas2D.prototype._get = function (name) {
  return this._values?.[name];
};
Canvas2D.prototype._set = function (name, v) {
  (this._values ??= {})[name] = v;
  this.ops.push([name, v]);
};
