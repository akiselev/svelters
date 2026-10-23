// @svelters/native-runtime/embed (EM1, plans/research/ARCH-EMBED.md §4.3): many roots in one runtime,
// mounted by a native embedder (svelters-egui's `Embed`) into slots it creates.
//
// Importing this module installs `globalThis.__svelters_embed`, the host's only entry into roots. The
// host calls it from its logic phase, one entry into JS per call, under its turn budget, followed by
// a job drain (svelters-host `root_call`):
//
//   mount(Component, rootId, targetHandle, props)   first render into the target; throws on failure
//   props(rootId, patch)                            shallow merge into the root's $state props
//                                                   (`patch.$$delete`: keys to remove)
//   info(rootId, patch)                             the root's host fields (visible, size, viewport…)
//   unmount(rootId)                                 dispose; the target itself stays
//   retarget(rootId, { target })                    EM2 `move_root`: the host moved the root's natives
//                                                   under `target`; new ones go there too
//   close(rootId) -> boolean                        the root's onCloseRequest handlers decide
//
// None of them flushes (FX6b, plans/milestones/FX6-embed-destroy-mutation.md): what a call changed
// renders in Svelte's own microtask flush, which the host's job drain right after the call runs, as
// for every other entry. A `flushSync` here would leave Svelte's `is_flushing_sync` set for good if
// the turn budget stopped it (QuickJS's interrupt skips `finally`), and from then on writes outside
// a flush (handlers, timers) would no longer schedule one. The host returns the drain's errors as
// the call's own, so they still reach it attributed to the root and the call.
//
// Each root has
// - its own error boundary: render and effect errors no <svelte:boundary> handled are reported with
//   `__svelters_host.rootFault(rootId, …)` (the host disposes that root only) instead of being thrown
//   into whatever flush ran them;
// - its own animation-frame queue: `requestAnimationFrame` called from the root's effects, from its
//   event handlers (EM2: dispatches to its nodes run in its context) or while the host is inside one
//   of the calls above for it is queued per root, and a hidden root's callbacks wait until it is shown
//   again (SV19); calls with no root context (a timer, a shared microtask) use the global queue;
// - a `RootHandle` (`import { root } from 'svelters:embed'`), whose fields are reactive: the host's
//   fields arrive through `info` only when they changed (no JS per frame), and `title`/`dirty` written
//   by components go back to the host (`__svelters_host.rootInfo`), as does `emit`.
import * as $ from './core/index.js';
import { active } from './core/svelte.js';
import { getContext, mount, onDestroy, unmount } from './svelte.js';
import { onRecover, setDispatchHook, targetNode } from './index.js';

const host = () => globalThis.__svelters_host;

/** The context key `root()` reads (set by `mount` for the root component and its children). */
export const ROOT_KEY = Symbol('svelters:embed root');

/** rootId -> root record. */
const roots = new Map();
/** A root's top effect -> its record (per-root rAF tagging). */
const effectRoots = new WeakMap();
/** A root's mount target node -> its record (per-root dispatch tagging, EM2). */
const targetRoots = new WeakMap();
/** The root the host is calling into right now. */
let current = null;

function rootOfActive() {
  for (let e = active(); e; e = e.parent) {
    const r = effectRoots.get(e);
    if (r) return r;
  }
  return null;
}

/** What a component sees of its root (`root()`). */
class RootHandle {
  #r;
  constructor(r) {
    this.#r = r;
  }
  get id() {
    return this.#r.id;
  }
  get key() {
    return this.#r.info.key;
  }
  get visible() {
    return this.#r.info.visible;
  }
  get width() {
    return this.#r.info.width;
  }
  get height() {
    return this.#r.info.height;
  }
  get scale() {
    return this.#r.info.scale;
  }
  get viewport() {
    return this.#r.info.viewport;
  }
  get focused() {
    return this.#r.info.focused;
  }
  get title() {
    return this.#r.info.title;
  }
  set title(v) {
    v = String(v);
    // untracked: an effect that sets the title must not depend on it
    if ($.untrack(() => this.#r.info.title) === v) return;
    this.#r.info.title = v;
    host().rootInfo(this.#r.id, v, null);
  }
  get dirty() {
    return this.#r.info.dirty;
  }
  set dirty(v) {
    v = !!v;
    if ($.untrack(() => this.#r.info.dirty) === v) return;
    this.#r.info.dirty = v;
    host().rootInfo(this.#r.id, null, v);
  }
  /** Sends `EmbedEvent::Emitted { name, value }` to the host (`value`: JSON-serializable). */
  emit(name, value) {
    host().rootEmit(this.#r.id, String(name), JSON.stringify(value ?? null));
  }
  /**
   * `fn()` runs when the host asks the root to close; returning `false` keeps it open. Returns the
   * remover; registered during component initialisation, it is removed with the component.
   */
  onCloseRequest(fn) {
    const set = this.#r.closers;
    set.add(fn);
    const off = () => set.delete(fn);
    try {
      onDestroy(off);
    } catch {
      // outside component initialisation: the caller removes it
    }
    return off;
  }
}

/** The root of the calling component (during its initialisation), or of the host's current call. */
export function root() {
  try {
    const r = getContext(ROOT_KEY);
    if (r) return r;
  } catch {
    // not in component initialisation
  }
  const r = current ?? rootOfActive();
  if (r) return r.handle;
  throw new Error('root(): not inside an embedded root (call it while a component of one initialises)');
}

function fault(r, e) {
  if (r.faulted) return;
  r.faulted = true;
  const message = e instanceof Error ? e.message : String(e);
  host().rootFault(r.id, message, e instanceof Error ? (e.stack ?? null) : null);
}

/** The root whose target `node` is inserted under (its native parents up to a mount target). */
function rootOfNode(node) {
  for (let n = node; n; n = n.np) {
    const r = targetRoots.get(n);
    if (r) return r;
  }
  return null;
}

function within(r, fn) {
  const prev = current;
  current = r;
  try {
    return fn();
  } finally {
    current = prev;
  }
}

// ---- per-root animation frames ----

const baseRaf = globalThis.requestAnimationFrame;
const baseCancel = globalThis.cancelAnimationFrame;
/** Ids of per-root callbacks (above the prelude's) -> their root. */
const rafOwner = new Map();
let rafId = 2 ** 30;

// `r.visible` is a plain mirror of `r.info.visible`: reading the reactive field here would make the
// effect that called requestAnimationFrame depend on the root's visibility.
function armRoot(r) {
  if (r.rafBase != null || r.raf.size === 0 || !r.visible) return;
  r.rafBase = baseRaf((ts) => {
    r.rafBase = null;
    if (!roots.has(r.id) || !r.visible) return; // hidden meanwhile: held until shown
    const batch = [...r.raf];
    r.raf.clear();
    let first;
    within(r, () => {
      for (const [id, cb] of batch) {
        rafOwner.delete(id);
        try {
          cb(ts);
        } catch (e) {
          if (first === undefined) first = e;
          else console.error(e);
        }
      }
    });
    if (first !== undefined) throw first;
  });
}

function installRaf() {
  if (typeof baseRaf !== 'function') return;
  globalThis.requestAnimationFrame = (cb) => {
    const r = current ?? rootOfActive();
    if (!r) return baseRaf(cb);
    if (typeof cb !== 'function') throw new TypeError('requestAnimationFrame: callback must be a function');
    const id = rafId++;
    r.raf.set(id, cb);
    rafOwner.set(id, r);
    armRoot(r);
    return id;
  };
  globalThis.cancelAnimationFrame = (id) => {
    const r = rafOwner.get(id);
    if (!r) return baseCancel(id);
    rafOwner.delete(id);
    r.raf.delete(id);
  };
}

// ---- the host's entry ----

const api = {
  mount(Component, id, target, props) {
    if (roots.has(id)) throw new Error(`root ${id} is already mounted`);
    const r = {
      id,
      info: $.proxy({ key: '', visible: true, width: 0, height: 0, scale: 1, viewport: '', focused: false, title: '', dirty: false }),
      props: $.proxy(props ?? {}),
      visible: true,
      raf: new Map(),
      rafBase: null,
      closers: new Set(),
      exports: null,
      faulted: false,
    };
    r.handle = new RootHandle(r);
    // our own target node, so `retarget` can move it (index.js's mount uses a Native as is)
    r.target = targetNode(target);
    targetRoots.set(r.target, r);
    roots.set(id, r);
    const Tagged = ($$anchor, p) => {
      const e = active();
      if (e) effectRoots.set(e, r);
      return Component($$anchor, p);
    };
    try {
      within(r, () => {
        r.exports = mount(Tagged, {
          target: r.target,
          props: r.props,
          context: new Map([[ROOT_KEY, r.handle]]),
          onerror: (e) => fault(r, e),
        });
      });
    } catch (e) {
      roots.delete(id);
      throw e;
    }
  },

  props(id, patch) {
    const r = roots.get(id);
    if (!r) return;
    const del = patch.$$delete;
    within(r, () => {
      for (const k of Object.keys(patch)) {
        if (k !== '$$delete') r.props[k] = patch[k];
      }
      if (Array.isArray(del)) for (const k of del) delete r.props[k];
    });
  },

  info(id, patch) {
    const r = roots.get(id);
    if (!r) return;
    if ('visible' in patch) r.visible = !!patch.visible;
    within(r, () => {
      for (const k of Object.keys(patch)) {
        if ($.untrack(() => r.info[k]) !== patch[k]) r.info[k] = patch[k];
      }
    });
    armRoot(r);
  },

  unmount(id) {
    const r = roots.get(id);
    if (!r) return;
    roots.delete(id);
    for (const rid of r.raf.keys()) rafOwner.delete(rid);
    r.raf.clear();
    if (r.rafBase != null) baseCancel(r.rafBase);
    r.closers.clear();
    // the teardown's microtasks (a subscriber's last unsubscribe) run in the host's drain
    if (r.exports) within(r, () => unmount(r.exports));
  },

  retarget(id, patch) {
    const r = roots.get(id);
    if (!r) return;
    r.target.h = patch.target;
  },

  close(id) {
    const r = roots.get(id);
    if (!r) return true;
    return within(r, () => {
      for (const fn of [...r.closers]) {
        if (fn() === false) return false;
      }
      return true;
    });
  },

  /** Live roots (tests). */
  roots() {
    return [...roots.keys()];
  },
};

if (!globalThis.__svelters_embed) {
  installRaf();
  // EM2: an event handler of a root's node runs in that root's context (root(), per-root rAF)
  setDispatchHook((node, run) => {
    const r = current ?? rootOfNode(node);
    return r ? within(r, run) : run();
  });
  // FX6: an aborted entry skipped `within`'s finally
  onRecover(() => {
    current = null;
  });
  globalThis.__svelters_embed = api;
}
