// X8: typed sync results. A spin button shows its value as hex ("0xAB"), formatted by the `output`
// handler and parsed back by the `input` handler (an out-parameter result). Handles: window 1, spin 2.
const h = globalThis.__svelters_host;
const win = h.create('window');
h.setProp(win, 'title', 'Typed sync');
const spin = h.create('spinbutton');
h.call(spin, 'set_range', [0, 255]);
h.setProp(spin, 'updatePolicy', 'if-valid');
h.insert(win, spin, null);
h.listen(spin, 'output', true);
h.listen(spin, 'input', true);

const log = [];
globalThis.__log = () => log.join('|');
globalThis.__svelters_dispatch_sync = (_h, signal, args) => {
  if (signal === 'output') {
    const v = h.getProp(spin, 'value');
    h.setProp(spin, 'text', '0x' + Math.round(v).toString(16).toUpperCase().padStart(2, '0'));
    log.push(`output ${v}`);
    return true;
  }
  const text = h.getProp(spin, 'text');
  log.push(`input ${text} args=${args.length}`);
  if (text === 'bad') return { ret: -1 };
  if (text === 'plain') return null;
  if (text === 'num') return 7; // shorthand for {ret: 1, newValue: 7}
  const n = parseInt(text.replace(/^0x/i, ''), 16);
  return { newValue: n };
};
globalThis.__svelters_dispatch = (_h, signal) => log.push(`async ${signal}`);
h.call(win, 'present', []);
