// VIS1: a minimal Chrome DevTools Protocol client for the VS Code workbench window, plus a small CLI
// for poking at it while writing capture.js. Runs on Windows inside Code.exe as Node
// (ELECTRON_RUN_AS_NODE=1, Node 24: global fetch and WebSocket), next to a VS Code launched with
// --remote-debugging-port=9229 (setup.sh --launch).
//
//   node cdp.js js '<expression>'          prints the JSON value
//   node cdp.js jsfile <file.js>          the same, the expression read from a file
//   node cdp.js shot <out.png> [x y w h]   a screenshot (CSS px, at DPR 1)
//   node cdp.js key <key> [mods]           mods: ctrl,shift,alt
//   node cdp.js type <text>
//   node cdp.js click <x> <y> [right]
//   node cdp.js move <x> <y>
'use strict';
const fs = require('node:fs');

const PORT = Number(process.env.CDP_PORT || 9229);

class Cdp {
  static async connect() {
    const list = await (await fetch(`http://127.0.0.1:${PORT}/json/list`)).json();
    const page = list.find((t) => t.type === 'page' && /workbench/.test(t.url));
    if (!page) throw new Error('no workbench page: ' + JSON.stringify(list.map((t) => t.url)));
    const ws = new WebSocket(page.webSocketDebuggerUrl);
    await new Promise((ok, err) => {
      ws.onopen = ok;
      ws.onerror = err;
    });
    const c = new Cdp(ws);
    await c.send('Page.enable');
    await c.send('DOM.enable');
    await c.send('CSS.enable');
    return c;
  }
  constructor(ws) {
    this.ws = ws;
    this.id = 0;
    this.pending = new Map();
    ws.onmessage = (m) => {
      const msg = JSON.parse(m.data);
      if (msg.id && this.pending.has(msg.id)) {
        const { ok, err } = this.pending.get(msg.id);
        this.pending.delete(msg.id);
        if (msg.error) err(new Error(JSON.stringify(msg.error)));
        else ok(msg.result);
      }
    };
  }
  send(method, params = {}) {
    const id = ++this.id;
    this.ws.send(JSON.stringify({ id, method, params }));
    return new Promise((ok, err) => this.pending.set(id, { ok, err }));
  }
  async eval(expression) {
    const r = await this.send('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true });
    if (r.exceptionDetails) throw new Error(JSON.stringify(r.exceptionDetails));
    return r.result.value;
  }
  /** DPR 1 at the window's CSS size, so crops are 1x whatever the Windows display scale is. */
  async dpr(scale = 1) {
    const { w, h } = await this.eval('({w: innerWidth, h: innerHeight})');
    await this.send('Emulation.setDeviceMetricsOverride', { width: w, height: h, deviceScaleFactor: scale, mobile: false });
  }
  /** `clip` is in the page's CSS px (getBoundingClientRect). CDP's clip is in DIPs at zoom 1, so
   * under window.zoomLevel (setup: zoom 1/display-scale, so devicePixelRatio is 1) it is scaled by
   * the zoom; the display scale brings the crop back to one image pixel per CSS px. */
  async shot(file, clip) {
    const p = { format: 'png', captureBeyondViewport: false };
    if (clip) {
      if (this.zoom === undefined) {
        // measured once: a 100 px clip at scale 1 comes back (display scale) px wide
        const r = await this.send('Page.captureScreenshot', { format: 'png', clip: { x: 0, y: 0, width: 100, height: 10, scale: 1 } });
        this.zoom = Buffer.from(r.data, 'base64').readUInt32BE(16) / 100;
      }
      const f = this.zoom;
      p.clip = { x: clip.x / f, y: clip.y / f, width: clip.width / f, height: clip.height / f, scale: 1 };
    }
    const r = await this.send('Page.captureScreenshot', p);
    fs.writeFileSync(file, Buffer.from(r.data, 'base64'));
  }
  async key(key, mods = []) {
    const m = (mods.includes('alt') ? 1 : 0) | (mods.includes('ctrl') ? 2 : 0) | (mods.includes('shift') ? 8 : 0);
    const named = {
      Enter: [13, 'Enter', '\r'], Escape: [27, 'Escape'], Tab: [9, 'Tab'], ArrowDown: [40, 'ArrowDown'],
      ArrowUp: [38, 'ArrowUp'], ArrowRight: [39, 'ArrowRight'], ArrowLeft: [37, 'ArrowLeft'], F1: [112, 'F1'],
      Backspace: [8, 'Backspace'], Space: [32, 'Space', ' '],
    };
    let code, k, text;
    if (named[key]) [code, k, text] = named[key];
    else {
      code = key.toUpperCase().charCodeAt(0);
      k = key;
      text = m & 2 ? undefined : key;
    }
    const base = { modifiers: m, windowsVirtualKeyCode: code, key: k, code: k.length === 1 ? 'Key' + k.toUpperCase() : k };
    await this.send('Input.dispatchKeyEvent', { type: text ? 'keyDown' : 'rawKeyDown', text, ...base });
    await this.send('Input.dispatchKeyEvent', { type: 'keyUp', ...base });
  }
  async type(text) {
    await this.send('Input.insertText', { text });
  }
  async mouse(type, x, y, button = 'none', clickCount = 0) {
    await this.send('Input.dispatchMouseEvent', { type, x, y, button, clickCount });
  }
  async click(x, y, button = 'left') {
    await this.mouse('mouseMoved', x, y);
    await this.mouse('mousePressed', x, y, button, 1);
    await this.mouse('mouseReleased', x, y, button, 1);
  }
  async waitFor(expr, ms = 3000) {
    const end = Date.now() + ms;
    while (Date.now() < end) {
      if (await this.eval(expr)) return true;
      await new Promise((r) => setTimeout(r, 100));
    }
    return false;
  }
  /** Opens the quick input with `key` (F1: commands; ctrl+p: files), types `text`, waits for a row
   * whose label starts with `text`'s first word and accepts it. Retries the open, which a busy
   * workbench sometimes drops. */
  async quick(text, file = false) {
    const key = 'F1', mods = [];
    const open = `(() => { const w = document.querySelector('.quick-input-widget'); return !!w && w.style.display !== 'none' && w.offsetHeight > 0; })()`;
    for (let i = 0; i < 3; i++) {
      if (i > 0) await this.eval(`document.activeElement?.blur(); document.querySelector('.monaco-workbench')?.focus()`);
      await this.key(key, mods);
      if (await this.waitFor(open, 1500)) break;
    }
    if (!(await this.eval(open))) throw new Error('quick input did not open for ' + text);
    if (file) {
      // F1 opens the command mode (">"); removing the prefix switches to file search
      await this.key('Backspace');
      await new Promise((r) => setTimeout(r, 200));
    }
    await this.type(text);
    const want = text.replace(/^>\s*/, '').toLowerCase();
    const ok = await this.waitFor(
      `(() => { const r = document.querySelector('.quick-input-widget .monaco-list-row'); return !!r && r.textContent.toLowerCase().includes(${JSON.stringify(want.split(/[ :]/)[0])}); })()`,
      3000,
    );
    if (!ok) throw new Error('no quick input row for ' + text);
    await new Promise((r) => setTimeout(r, 200));
    await this.key('Enter');
    await this.waitFor(`!(${open})`, 2000);
    await new Promise((r) => setTimeout(r, 500));
  }
  command(name) {
    return this.quick(name);
  }
  openFile(name) {
    return this.quick(name, true);
  }
  /** The DOM node id of the first element matching `selector` (pierces shadow roots). */
  async node(selector) {
    const objectId = (
      await this.send('Runtime.evaluate', { expression: `__vis.q(${JSON.stringify(selector)})` })
    ).result.objectId;
    if (!objectId) return null;
    const { nodeId } = await this.send('DOM.requestNode', { objectId });
    return nodeId;
  }
  async force(selector, states) {
    await this.send('DOM.getDocument', { depth: -1, pierce: true });
    const id = await this.node(selector);
    if (!id) throw new Error('no node ' + selector);
    await this.send('CSS.forcePseudoState', { nodeId: id, forcedPseudoClasses: states });
    return id;
  }
  close() {
    this.ws.close();
  }
}

/** Page-side helpers, installed once (`__vis`): querying through shadow roots and reading metrics. */
const PAGE_HELPERS = `
window.__vis = window.__vis || {
  all(sel, root = document) {
    const out = [...root.querySelectorAll(sel)];
    for (const el of root.querySelectorAll('*')) if (el.shadowRoot) out.push(...__vis.all(sel, el.shadowRoot));
    return out;
  },
  q(sel) {
    return __vis.all(sel).find((e) => { const r = e.getBoundingClientRect(); return r.width > 0 && r.height > 0; }) || null;
  },
  metrics(el, textEl) {
    const r = el.getBoundingClientRect();
    const cs = getComputedStyle(el);
    const t = textEl ? getComputedStyle(textEl) : cs;
    const px = (v) => Math.round(parseFloat(v) * 100) / 100;
    return {
      rect: { x: px(r.x), y: px(r.y), w: px(r.width), h: px(r.height) },
      padding: [px(cs.paddingTop), px(cs.paddingRight), px(cs.paddingBottom), px(cs.paddingLeft)],
      border_width: px(cs.borderTopWidth),
      border_color: cs.borderTopWidth !== '0px' && cs.borderTopStyle !== 'none' ? cs.borderTopColor : null,
      radius: px(cs.borderTopLeftRadius),
      bg: cs.backgroundColor,
      fg: t.color,
      font_size: px(t.fontSize),
      font_weight: Number(t.fontWeight),
      font_family: t.fontFamily,
      line_height: t.lineHeight,
      outline: cs.outlineStyle !== 'none' && cs.outlineWidth !== '0px' ? cs.outlineWidth + ' ' + cs.outlineColor + ' offset ' + cs.outlineOffset : null,
      box_shadow: cs.boxShadow === 'none' ? null : cs.boxShadow,
      opacity: Number(cs.opacity),
      cursor: cs.cursor,
    };
  },
  vars() {
    const cs = getComputedStyle(document.querySelector('.monaco-workbench'));
    const out = {};
    for (let i = 0; i < cs.length; i++) if (cs[i].startsWith('--vscode-')) out[cs[i]] = cs.getPropertyValue(cs[i]).trim();
    return out;
  },
};
true`;

module.exports = { Cdp, PAGE_HELPERS };

if (require.main === module) {
  (async () => {
    const [cmd, ...a] = process.argv.slice(2);
    const c = await Cdp.connect();
    await c.eval(PAGE_HELPERS);
    if (cmd === 'js') console.log(JSON.stringify(await c.eval(a[0]), null, 1));
    else if (cmd === 'jsfile') console.log(JSON.stringify(await c.eval(fs.readFileSync(a[0], 'utf8')), null, 1));
    else if (cmd === 'shot') await c.shot(a[0], a.length > 1 ? { x: +a[1], y: +a[2], width: +a[3], height: +a[4] } : undefined);
    else if (cmd === 'key') await c.key(a[0], (a[1] || '').split(','));
    else if (cmd === 'type') await c.type(a[0]);
    else if (cmd === 'click') await c.click(+a[0], +a[1], a[2] || 'left');
    else if (cmd === 'move') await c.mouse('mouseMoved', +a[0], +a[1]);
    else if (cmd === 'dpr') await c.dpr(+(a[0] || 1));
    else if (cmd === 'cmd') await c.command(a[0]);
    else throw new Error('unknown command ' + cmd);
    c.close();
  })().catch((e) => {
    console.error(e);
    process.exit(1);
  });
}
