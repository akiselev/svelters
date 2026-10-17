const h = globalThis.__svelters_host;
const win = h.create('window');
const grid = h.create('grid');
const a = h.create('label');
const b = h.create('label');
h.insert(win, grid, null);
h.insert(grid, a, null);
h.setChildAttr(grid, a, 'column', 2);
h.setChildAttr(grid, a, 'row', 'x');
const out = [h.getChildAttr(grid, a, 'column'), h.getChildAttr(grid, a, 'row')];
h.setChildAttr(grid, a, 'column', null);
out.push(h.getChildAttr(grid, a, 'column'));
try {
  h.setChildAttr(grid, b, 'column', 1);
} catch (e) {
  out.push(/not a child|inserted/.test(String(e.message)));
}
h.remove(grid, a);
h.insert(grid, a, null);
out.push(h.getChildAttr(grid, a, 'row'));
h.setProp(win, 'title', JSON.stringify(out));
globalThis.__svelters_dispatch_sync = (_h, signal, _args, flags) => {
  h.setProp(win, 'title', `${signal}/${flags}`);
  return null;
};
globalThis.__svelters_dispatch = (_h, signal, _args, deferred, flags) => {
  h.setProp(win, 'title', `${signal}/${deferred}/${flags}`);
};
h.listen(win, 'close-request', true, 2);
h.listen(win, 'close-request', true, 1);
h.listen(win, 'close-request', false, 1);
h.listen(win, 'close-request', true);
