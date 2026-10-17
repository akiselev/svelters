// X8 on Qt: event families and the canvas through the real host. Handles in creation order:
// main-window 1, box-layout 2, label 3, area 4 (a box-layout, the event target), canvas 5.
const h = globalThis.__svelters_host;
const win = h.create('main-window');
const box = h.create('box-layout');
const label = h.create('label');
const area = h.create('box-layout');
const cv = h.create('canvas');
h.setProp(cv, 'content-width', 80);
h.setProp(cv, 'content-height', 40);
h.setProp(cv, 'ops', JSON.stringify([['fillStyle', '#00ff00'], ['fillRect', 0, 0, 20, 20], ['fillStyle', '#ff00ff'], ['fillRect', 30, 0, 10, 10]]));
h.insert(area, label, null);
h.insert(box, area, null);
h.insert(box, cv, null);
h.insert(win, box, null);
h.listen(area, 'mouse-press', true);
h.listen(area, 'wheel', true);
h.listen(box, 'mouse-press', true, 2); // capture: sees the descendants' presses first
h.listen(cv, 'mouse-move', true);
h.listen(cv, 'resize', true);

const log = [];
const note = (s) => {
  log.push(s);
  h.setProp(label, 'text', log.join('|'));
};
globalThis.__svelters_dispatch_sync = (target, signal, args, flags) => {
  const [pos, button] = args;
  if (signal === 'mouse-press') {
    note(`${target === box ? 'capture' : 'press'} ${JSON.stringify(pos)} b${button}`);
    // the box captures presses of the left button but lets the right one through; the area consumes the right one
    if (target === box) return button === 1 ? true : null;
    return button === 2 ? true : null;
  }
  if (signal === 'wheel') {
    note(`wheel ${JSON.stringify(args[1])}`);
    return true;
  }
  note(`${signal} ${JSON.stringify(args)}`);
  return null;
};
globalThis.__svelters_dispatch = (_h, signal, args) => note(`async ${signal} ${JSON.stringify(args)}`);
h.call(win, 'present', []);
