// VIS1: captures VS Code reference crops and computed metrics, one theme per run, from a VS Code
// started by `setup.sh --launch` (clean profile, workspace C:\vis1\ws). Runs on Windows inside
// Code.exe as Node: `VIS_THEME=dark-modern winnode C:\vis1\capture.js`.
//
// Every element is a real workbench widget (the explorer tree, editor tabs, the find widget, the
// settings editor, a custom dialog, a notification toast, a context menu, a hover), never a DOM copy:
// VS Code paints several of them from JS (button hover backgrounds, list selection rules per list id).
// States are reached the way a user reaches them (mouse moves, clicks, keyboard focus) and, where a
// state is a CSS pseudo-class, also forced with CSS.forcePseudoState so the crop is deterministic.
// Disabled states use VS Code's own `disabled` class on the real widget.
//
// Output: C:\vis1\out\<theme>\<element>-<state>.png (1x crops, element rect + 4px) and metrics.json
// ({ meta, items: [{ element, state, how, metrics, png }], vars }).
'use strict';
const fs = require('node:fs');
const path = require('node:path');
const { Cdp, PAGE_HELPERS } = require('./cdp.js');

const THEMES = {
  'dark-modern': 'Default Dark Modern',
  'light-modern': 'Default Light Modern',
};
const theme = process.env.VIS_THEME || 'dark-modern';
const OUT = path.join('C:\\vis1\\out', theme);
const SETTINGS = 'C:\\vis1\\ud\\User\\settings.json';
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

let c;
const items = [];
const log = (...a) => console.log(...a);

async function js(expr) {
  return c.eval(expr);
}

/** The element's metrics (`textSel`: the descendant whose text style counts, as a selector relative
 * to the element) and a crop of it. */
async function capture(element, state, sel, { text, how = 'rest', pad = 4, clipSel } = {}) {
  await sleep(120);
  const m = await js(`(() => {
    const el = __vis.q(${JSON.stringify(sel)});
    if (!el) return null;
    const t = ${text ? `el.querySelector(${JSON.stringify(text)})` : 'null'};
    const m = __vis.metrics(el, t);
    if (t) { const r = t.getBoundingClientRect(); m.text_rect = { x: r.x, y: r.y, w: r.width, h: r.height }; }
    m.text = (t || el).textContent.trim().slice(0, 60);
    const clip = ${clipSel ? `__vis.q(${JSON.stringify(clipSel)})` : 'el'}.getBoundingClientRect();
    m.clip = { x: clip.x, y: clip.y, w: clip.width, h: clip.height };
    return m;
  })()`);
  if (!m) {
    log(`  MISSING ${element}-${state} (${sel})`);
    items.push({ element, state, how, selector: sel, metrics: null });
    return null;
  }
  const png = `${element}-${state}.png`;
  const r = m.clip;
  const x = Math.max(0, Math.floor(r.x) - pad), y = Math.max(0, Math.floor(r.y) - pad);
  await c.shot(path.join(OUT, png), { x, y, width: Math.ceil(r.w) + 2 * pad, height: Math.ceil(r.h) + 2 * pad });
  delete m.clip;
  items.push({ element, state, how, selector: sel, metrics: m, png });
  log(`  ${element}-${state}: ${m.rect.w}x${m.rect.h} bg ${m.bg} fg ${m.fg}`);
  return m;
}

async function force(sel, states) {
  try {
    await c.force(sel, states);
  } catch (e) {
    log('  force failed', sel, e.message);
  }
}

async function center(sel) {
  return js(`(() => { const r = __vis.q(${JSON.stringify(sel)})?.getBoundingClientRect(); return r ? { x: r.x + r.width / 2, y: r.y + r.height / 2 } : null; })()`);
}

async function hoverMouse(sel) {
  const p = await center(sel);
  if (p) {
    await c.mouse("mouseMoved", p.x - 2, p.y);
    await sleep(60);
    await c.mouse("mouseMoved", p.x, p.y);
  }
  await sleep(450);
}

async function away() {
  await c.mouse('mouseMoved', 700, 500);
  await sleep(150);
}

/** Captures a widget in rest, hover (real mouse + forced), active (forced), focus (focus() + forced),
 * then disabled through the widget's `disabled` class. */
async function states(element, sel, opts = {}, which = ['rest', 'hover', 'active', 'focus', 'disabled']) {
  if (which.includes('rest')) {
    await away();
    await capture(element, 'rest', sel, opts);
  }
  if (which.includes('hover')) {
    await hoverMouse(sel);
    await force(sel, ['hover']);
    await capture(element, 'hover', sel, { ...opts, how: 'mouse over + forced :hover' });
    await force(sel, []);
    await away();
  }
  if (which.includes('active')) {
    await force(sel, ['hover', 'active']);
    await capture(element, 'active', sel, { ...opts, how: 'forced :hover:active' });
    await force(sel, []);
  }
  if (which.includes('focus')) {
    await js(`__vis.q(${JSON.stringify(sel)}).focus()`);
    await force(sel, ['focus', 'focus-visible']);
    await capture(element, 'focus', sel, { ...opts, how: 'focus() + forced :focus-visible' });
    await force(sel, []);
    await js(`__vis.q(${JSON.stringify(sel)}).blur()`);
  }
  if (which.includes('disabled')) {
    await js(`__vis.q(${JSON.stringify(sel)}).classList.add('disabled')`);
    await capture(element, 'disabled', sel, { ...opts, how: 'VS Code `disabled` class' });
    await js(`__vis.q(${JSON.stringify(sel)}).classList.remove('disabled')`);
  }
}

const command = (name) => c.command(name);
const openFile = (name) => c.openFile(name);

/** Reverts and closes every editor (a previous run's edits, hot exit), closes dialogs and widgets. */
async function clean() {
  for (let i = 0; i < 3; i++) await c.key('Escape');
  for (let i = 0; i < 8; i++) {
    const dirty = await js(`__vis.all('.tabs-container .tab.dirty').length`);
    if (!dirty) break;
    await js(`__vis.q('.tabs-container .tab.dirty').dispatchEvent(new MouseEvent('mousedown', {bubbles: true, button: 0}))`);
    await sleep(300);
    await command('View: Revert and Close Editor');
  }
  await command('View: Close All Editors');
}

async function main() {
  fs.mkdirSync(OUT, { recursive: true });
  for (const f of fs.readdirSync(OUT)) fs.unlinkSync(path.join(OUT, f));
  // the theme, through the watched settings file
  const s = JSON.parse(fs.readFileSync(SETTINGS, 'utf8'));
  s['workbench.colorTheme'] = THEMES[theme];
  fs.writeFileSync(SETTINGS, JSON.stringify(s, null, 2));
  c = await Cdp.connect();
  await sleep(2000);
  // a reload shows the "extensions are disabled" toast again (the notification toast reference)
  // (DPR 1 first: the emulation outlives the reload, so the workbench lays out at 1x from the start)
  if (Math.abs((await c.eval("devicePixelRatio")) - 1) > 0.01) await c.dpr(1);
  // (CDP's Page.reload breaks the workbench; VS Code's own reload goes through its main process)
  await c.eval(PAGE_HELPERS);
  for (let i = 0; i < 3; i++) await c.key('Escape');
  await c.command('Developer: Reload Window');
  await sleep(3000);
  try {
    if (!(await c.waitFor(`!!document.querySelector('.monaco-workbench .explorer-folders-view')`, 30000))) throw new Error('not loaded');
  } catch (e) {
    log('reconnecting after the reload:', e.message);
    c.close();
    await sleep(3000);
    c = await Cdp.connect();
    if (Math.abs((await c.eval("devicePixelRatio")) - 1) > 0.01) await c.dpr(1);
    await c.waitFor(`!!document.querySelector('.monaco-workbench .explorer-folders-view')`, 30000);
  }
  await sleep(3000);
  await c.eval(PAGE_HELPERS);
  await sleep(300);
  const meta = await js(`({ theme: ${JSON.stringify(THEMES[theme])}, ua: navigator.userAgent, dpr: devicePixelRatio, size: [innerWidth, innerHeight],
    themeClass: document.querySelector('.monaco-workbench').className })`);
  log('theme', meta.theme, 'dpr', meta.dpr);
  await js(`document.activeElement?.blur()`);

  // ---- the notification toast and its primary button (the reload shows it; Escape hides it) ----
  const toast = '.notifications-toasts .notification-toast';
  if (await c.waitFor(`!!__vis.q(${JSON.stringify(toast)})`, 5000)) {
    await capture('toast', 'rest', toast, { text: '.notification-list-item-message' });
    await states('button-primary', toast + ' .monaco-text-button', {}, ['rest', 'hover', 'active', 'focus', 'disabled']);
  } else log('  no toast visible');
  await clean();

  // ---- the explorer: tree rows, twistie, pane header, sash ----
  await command('View: Show Explorer');
  await js(`(() => { for (const r of __vis.all('.explorer-folders-view .monaco-list-row')) if (/^(src|docs)$/.test(r.textContent.trim()) && r.getAttribute('aria-expanded') === 'false') r.click(); })()`);
  await sleep(500);
  const row = (name) => `.explorer-folders-view .monaco-list-row[aria-label^="${name}"]`;
  await away();
  await js('document.activeElement?.blur()');
  await capture('list-row', 'rest', row('main.ts'), { text: '.label-name', how: 'the tree is not focused' });
  await hoverMouse(row('main.ts'));
  await capture('list-row', 'hover', row('main.ts'), { text: '.label-name', how: 'mouse over' });
  await js(`__vis.q(${JSON.stringify(row('util.ts'))}).dispatchEvent(new MouseEvent('mousedown', {bubbles: true}))`);
  const p = await center(row('util.ts'));
  await c.click(p.x, p.y);
  await sleep(300);
  await away();
  await capture('list-row', 'selected-focused', row('util.ts'), { text: '.label-name', how: 'clicked; the tree has focus' });
  await c.key('ArrowDown');
  await sleep(200);
  await capture('list-row', 'focused', '.explorer-folders-view .monaco-list-row.focused', { text: '.label-name', how: 'ArrowDown: the keyboard-focused row (focus outline)' });
  await c.key('ArrowUp');
  await sleep(200);
  await js('document.activeElement?.blur()');
  await sleep(200);
  await capture('list-row', 'selected-inactive', row('util.ts'), { text: '.label-name', how: 'selected, the tree lost focus' });
  await c.key('ArrowUp');
  await capture('tree-twistie', 'expanded', row('src') + ' .monaco-tl-twistie', { how: 'expanded folder twistie' });
  await capture('pane-header', 'rest', '.pane-header', { text: '.title' });
  await capture('sidebar-title', 'rest', '.sidebar .composite.title', { text: 'h2' });
  await away();
  await capture('sash', 'rest', '.monaco-sash.vertical:not(.disabled)', { pad: 3 });
  await hoverMouse('.monaco-sash.vertical:not(.disabled)');
  await sleep(600);
  await capture('sash', 'hover', '.monaco-sash.vertical:not(.disabled)', { pad: 3, how: 'mouse over, 600ms (hover delay)' });
  await away();

  // ---- the editor: tabs (inactive, active, hover, dirty), scrollbar ----
  await openFile('main.ts');
  await openFile('util.ts');
  await openFile('long.txt');
  const tab = (name) => `.tabs-container .tab[aria-label^="${name}"]`;
  await away();
  await capture('tab', 'active', tab('long.txt'), { text: '.label-name' });
  await capture('tab', 'inactive', tab('main.ts'), { text: '.label-name' });
  await force(tab('main.ts'), ['hover']);
  await hoverMouse(tab('main.ts'));
  await capture('tab', 'hover', tab('main.ts'), { text: '.label-name', how: 'mouse over + forced :hover' });
  await force(tab('main.ts'), []);
  await away();
  await capture('editor-group-header', 'rest', '.editor-group-container .title', {});
  // scrollbar: shown on mouse over the editor
  await hoverMouse('.editor-instance .monaco-editor .view-lines');
  await sleep(300);
  await capture('scrollbar', 'rest', '.editor-instance .monaco-editor .scrollbar.vertical .slider', { pad: 2 });
  await hoverMouse('.editor-instance .monaco-editor .scrollbar.vertical .slider');
  await capture('scrollbar', 'hover', '.editor-instance .monaco-editor .scrollbar.vertical .slider', { pad: 2, how: 'mouse over the slider' });
  await js(`__vis.q('.editor-instance .monaco-editor .scrollbar.vertical .slider').classList.add('active')`);
  await capture('scrollbar', 'active', '.editor-instance .monaco-editor .scrollbar.vertical .slider', { pad: 2, how: 'VS Code `active` class (dragging)' });
  await js(`__vis.q('.editor-instance .monaco-editor .scrollbar.vertical .slider').classList.remove('active')`);
  await capture('scrollbar-track', 'rest', '.editor-instance .monaco-editor .scrollbar.vertical', { pad: 0 });
  await away();

  // ---- the find widget: input box, input-option toggles ----
  await js(`__vis.q('.editor-instance .monaco-editor textarea')?.focus()`);
  await c.key('f', ['ctrl']);
  await sleep(500);
  const findInput = '.find-widget .monaco-findInput .monaco-inputbox';
  await capture('input', 'focus', findInput, { text: 'textarea, input', how: 'the find input, focused, empty (placeholder)' });
  await c.type('line 1');
  await sleep(300);
  await capture('input', 'filled-focus', findInput, { text: 'textarea, input', how: 'typed text' });
  await js(`__vis.q('.editor-instance .monaco-editor textarea')?.focus()`);
  await sleep(200);
  await away();
  await capture('input', 'rest', findInput, { text: 'textarea, input', how: 'focus moved to the editor' });
  await capture('find-widget', 'rest', '.find-widget', {});
  const toggle = '.find-widget .monaco-findInput .monaco-custom-toggle';
  await states('toggle', toggle, {}, ['rest', 'hover', 'focus']);
  await js(`__vis.q(${JSON.stringify(toggle)}).click()`);
  await sleep(200);
  await away();
  await capture('toggle', 'checked', toggle, { how: 'clicked (Match Case on)' });
  await js(`__vis.q(${JSON.stringify(toggle)}).click()`);
  // validation error: regex on, an invalid pattern
  await js(`__vis.all('.find-widget .monaco-custom-toggle').find((t) => /Regular/.test(t.getAttribute('aria-label') || t.title)).click()`);
  await js(`__vis.q('.find-widget textarea, .find-widget input').focus()`);
  await c.key('a', ['ctrl']);
  await c.type('(');
  await sleep(500);
  await capture('input', 'invalid', findInput, { text: 'textarea, input', how: 'regex on, pattern "("' });
  await js(`__vis.all('.find-widget .monaco-custom-toggle').find((t) => /Regular/.test(t.getAttribute('aria-label') || t.title)).click()`);
  await c.key('Escape');
  await sleep(300);

  // ---- tab dirty: type into util.ts ----
  await openFile('util.ts');
  await c.type('x');
  await sleep(300);
  await away();
  await capture('tab', 'dirty-active', tab('util.ts'), { text: '.label-name', how: 'unsaved edit' });

  // ---- the settings editor: checkbox, select, number input, description ----
  await command('Preferences: Open Settings (UI)');
  await sleep(1500);
  await c.type('editor.minimap');
  await sleep(1500);
  const checkbox = '.settings-editor .setting-item-bool .setting-value-checkbox';
  await states('checkbox', checkbox, {}, ['rest', 'hover', 'focus', 'disabled']);
  const isOn = await js(`__vis.q(${JSON.stringify(checkbox)}).classList.contains('checked')`);
  await js(`__vis.q(${JSON.stringify(checkbox)}).click()`);
  await sleep(300);
  await away();
  await capture('checkbox', isOn ? 'unchecked' : 'checked', checkbox, { how: 'clicked' });
  await js(`__vis.q(${JSON.stringify(checkbox)}).click()`);
  await sleep(300);
  const select = '.settings-editor .setting-item-enum select.monaco-select-box';
  await states('select', select, {}, ['rest', 'hover', 'focus', 'disabled']);
  const number = '.settings-editor .setting-item-number .monaco-inputbox';
  await away();
  await capture('input-number', 'rest', number, { text: 'input' });
  await capture('button-primary-settings', 'rest', '.settings-editor .monaco-text-button:not(.secondary)', {});
  await capture('settings-editor', 'open', '.settings-editor', { pad: 0 });
  await capture('setting-row', 'rest', '.settings-editor .setting-item-number', { text: '.setting-item-description' });
  await capture('settings-label', 'rest', '.settings-editor .setting-item-number .setting-item-label', {});
  await js(`__vis.q(${JSON.stringify(number)} + ' input').setAttribute('disabled', '')`);
  await js(`__vis.q(${JSON.stringify(number)}).classList.add('disabled')`);
  await capture('input', 'disabled', number, { text: 'input', how: 'input[disabled] + `disabled` class' });
  await js(`__vis.q(${JSON.stringify(number)} + ' input').removeAttribute('disabled')`);
  await js(`__vis.q(${JSON.stringify(number)}).classList.remove('disabled')`);
  // the select's dropdown (VS Code's custom select box on Windows)
  await js(`__vis.q(${JSON.stringify(select)}).focus()`);
  await c.key('Space');
  await sleep(500);
  const drop = '.monaco-select-box-dropdown-container.visible';
  if (await js(`!!__vis.q(${JSON.stringify(drop)})`)) {
    await capture('select-dropdown', 'open', drop, {});
    await capture('select-option', 'focused', drop + ' .monaco-list-row.focused', { text: '.option-text' });
    await c.key('Escape');
  } else log('  select dropdown did not open (native select)');
  await sleep(300);
  await c.key('Escape');
  await sleep(300);
  if (await js(`!!__vis.q('.settings-editor')`)) await command('View: Close Editor');

  // ---- a context menu (custom menus: window.menuStyle custom) ----
  await command('View: Show Explorer');
  const pm = await center(row('main.ts'));
  await c.click(pm.x, pm.y, 'right');
  await sleep(600);
  const menu = '.monaco-menu-container .monaco-menu';
  if (await js(`!!__vis.q(${JSON.stringify(menu)})`)) {
    await capture('menu', 'open', '.monaco-menu-container', {});
    const item = (n) => `.monaco-menu-container .action-item:not(.disabled):nth-child(${n}) .action-menu-item`;
    await capture('menu-item', 'rest', item(3), { text: '.action-label' });
    await hoverMouse(item(1));
    await capture('menu-item', 'hover', item(1), { text: '.action-label', how: 'mouse over (focused item)' });
    await capture('menu-keybinding', 'rest', '.monaco-menu-container .action-item .keybinding', {});
    await capture('menu-separator', 'rest', '.monaco-menu-container .action-item.disabled .action-label.separator', { pad: 2 });
    await c.key('Escape');
  } else log('  no context menu');
  await sleep(300);

  // ---- a hover (tooltip) on an activity bar item ----
  await away();
  await hoverMouse('.activitybar .action-item:nth-child(2) .action-label');
  await sleep(1500);
  await capture('tooltip', 'open', '.workbench-hover, .monaco-hover.workbench-hover', { text: '.hover-contents, .hover-row' });
  await away();
  await sleep(500);

  // ---- a custom dialog: primary and secondary buttons ----
  // (closing the dirty util.ts asks to save)
  await openFile('util.ts');
  await c.key('F1');
  await c.waitFor(`!!document.querySelector('.quick-input-widget')?.offsetHeight`, 2000);
  await c.type('View: Close Editor');
  await sleep(600);
  await c.key('Enter');
  await sleep(1000);
  const dialog = '.monaco-dialog-box';
  if (await js(`!!__vis.q(${JSON.stringify(dialog)})`)) {
    await capture('dialog', 'open', dialog, { text: '.dialog-message-text' });
    await capture('dialog-modal-block', 'rest', '.monaco-dialog-modal-block', { pad: 0 });
    const prim = dialog + ' .dialog-buttons .monaco-text-button:not(.secondary)';
    const sec = dialog + ' .dialog-buttons .monaco-text-button.secondary';
    await states('button-secondary', sec, {});
    await states('button-primary-dialog', prim, {}, ['rest', 'hover', 'focus']);
    // Don't Save
    await js(`__vis.all(${JSON.stringify(sec)}).find((b) => /Don/.test(b.textContent)).click()`);
    await sleep(500);
  } else log('  no dialog');

  // ---- progress bar (CSS only: a running indicator is not reachable deterministically) ----
  const vars = await js('__vis.vars()');
  const progress = await js(`(() => {
    const el = __vis.q('.monaco-progress-container') ;
    if (!el) return null;
    const bit = el.querySelector('.progress-bit');
    const cs = getComputedStyle(el), b = bit && getComputedStyle(bit);
    return { height: cs.height, bit_bg: b && b.backgroundColor };
  })()`);
  items.push({ element: 'progress', state: 'rest', how: 'computed style of the hidden bar; colour = --vscode-progressBar-background', metrics: progress && { rect: { w: null, h: parseFloat(progress.height) }, bg: progress.bit_bg || vars['--vscode-progressBar-background'] } });

  fs.writeFileSync(path.join(OUT, 'metrics.json'), JSON.stringify({ meta: { ...meta, vscode: '1.133.0', scale: 1 }, items, vars }, null, 1));
  log(`wrote ${items.length} items to ${OUT}`);
  c.close();
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
