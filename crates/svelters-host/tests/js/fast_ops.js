// W-J1: the same tree built through the raw fast ops (`createId`, `insertId`, `setp`, `setter`,
// `listenId`) as `slow_ops.js` builds through the general ops; tests/fast_ops.rs compares the logs.
const h = globalThis.__svelters_host;
const K_WIN = h.kindId('window');
const K_BOX = h.kindId('box');
const K_LABEL = h.kindId('label');
const K_BUTTON = h.kindId('button');
const P_LABEL = h.propId(K_LABEL, 'label');
const P_LINES = h.propId(K_LABEL, 'lines');
const P_OPACITY = h.propId(K_LABEL, 'opacity');
const S_CLICKED = h.signalId(K_BUTTON, 'clicked');
const win = h.createId(K_WIN);
const box = h.createId(K_BOX);
h.setp(box, 'spacing', 6);
h.setp(box, 'homogeneous', true);
h.insertId(win, box, null, null);
const lbl = h.createId(K_LABEL);
h.setp(lbl, P_LABEL, 'hello');
h.setp(lbl, P_LINES, 3);
h.setp(lbl, P_OPACITY, 0.5);
h.setp(lbl, P_OPACITY, 1);
h.setp(lbl, 'label', 'héllo wörld');
const setLabel = h.setter(P_LABEL);
setLabel(lbl, 'again');
h.insertId(box, lbl, null, null);
const btn = h.createId(K_BUTTON);
h.insertId(box, btn, lbl, null);
h.listen(btn, 'clicked', true);
h.listenId(btn, S_CLICKED, false);
h.listenId(btn, S_CLICKED, true, 0);
// what the general path does with values the scalar rules leave to it (by name: the general
// path names a prop given by id without a descriptor `#<id>` in its errors)
for (const [prop, v] of [['lines', 2.5], [P_LINES, '7'], [P_LABEL, 42], ['label', {}], ['no-such-prop', 1]]) {
  try {
    h.setp(lbl, prop, v);
    console.log(`ok ${v}`);
  } catch (e) {
    console.log(`${e.name}: ${e.message}`);
  }
}
