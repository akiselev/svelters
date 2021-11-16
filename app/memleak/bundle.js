// crates/svelte/internal/index.mjs
function noop() {
}
function add_location(element2, file3, line, column, char) {
  element2.__svelte_meta = {
    loc: { file: file3, line, column, char }
  };
}
function run(fn) {
  return fn();
}
function blank_object() {
  return Object.create(null);
}
function run_all(fns) {
  fns.forEach(run);
}
function is_function(thing) {
  return typeof thing === "function";
}
function safe_not_equal(a, b) {
  return a != a ? b == b : a !== b || (a && typeof a === "object" || typeof a === "function");
}
function is_empty(obj) {
  return Object.keys(obj).length === 0;
}
var tasks = new Set();
var is_hydrating = false;
function start_hydrating() {
  is_hydrating = true;
}
function end_hydrating() {
  is_hydrating = false;
}
function append(target, node) {
  target.appendChild(node);
}
function append_styles(target, style_sheet_id, styles) {
  const append_styles_to = get_root_for_style(target);
  if (!append_styles_to.getElementById(style_sheet_id)) {
    const style = element("style");
    style.id = style_sheet_id;
    style.textContent = styles;
    append_stylesheet(append_styles_to, style);
  }
}
function get_root_for_style(node) {
  if (!node)
    return document;
  const root = node.getRootNode ? node.getRootNode() : node.ownerDocument;
  if (root && root.host) {
    return root;
  }
  return node.ownerDocument;
}
function append_stylesheet(node, style) {
  append(node.head || node, style);
}
function insert(target, node, anchor) {
  target.insertBefore(node, anchor || null);
}
function detach(node) {
  node.parentNode.removeChild(node);
}
function element(name) {
  return document.createElement(name);
}
function text(data) {
  return document.createTextNode(data);
}
function space() {
  return text(" ");
}
function listen(node, event, handler, options) {
  node.addEventListener(event, handler, options);
  return () => node.removeEventListener(event, handler, options);
}
function children(element2) {
  return Array.from(element2.childNodes);
}
function custom_event(type, detail, bubbles = false) {
  const e = document.createEvent("CustomEvent");
  e.initCustomEvent(type, bubbles, false, detail);
  return e;
}
var active_docs = new Set();
var current_component;
function set_current_component(component) {
  current_component = component;
}
var dirty_components = [];
var binding_callbacks = [];
var render_callbacks = [];
var flush_callbacks = [];
var resolved_promise = Promise.resolve();
var update_scheduled = false;
function schedule_update() {
  if (!update_scheduled) {
    update_scheduled = true;
    resolved_promise.then(flush);
  }
}
function add_render_callback(fn) {
  render_callbacks.push(fn);
}
var flushing = false;
var seen_callbacks = new Set();
function flush() {
  if (flushing)
    return;
  flushing = true;
  do {
    for (let i = 0; i < dirty_components.length; i += 1) {
      const component = dirty_components[i];
      set_current_component(component);
      update(component.$$);
    }
    set_current_component(null);
    dirty_components.length = 0;
    while (binding_callbacks.length)
      binding_callbacks.pop()();
    for (let i = 0; i < render_callbacks.length; i += 1) {
      const callback = render_callbacks[i];
      if (!seen_callbacks.has(callback)) {
        seen_callbacks.add(callback);
        callback();
      }
    }
    render_callbacks.length = 0;
  } while (dirty_components.length);
  while (flush_callbacks.length) {
    flush_callbacks.pop()();
  }
  update_scheduled = false;
  flushing = false;
  seen_callbacks.clear();
}
function update($$) {
  if ($$.fragment !== null) {
    $$.update();
    run_all($$.before_update);
    const dirty = $$.dirty;
    $$.dirty = [-1];
    $$.fragment && $$.fragment.p($$.ctx, dirty);
    $$.after_update.forEach(add_render_callback);
  }
}
var outroing = new Set();
var outros;
function group_outros() {
  outros = {
    r: 0,
    c: [],
    p: outros
  };
}
function check_outros() {
  if (!outros.r) {
    run_all(outros.c);
  }
  outros = outros.p;
}
function transition_in(block, local) {
  if (block && block.i) {
    outroing.delete(block);
    block.i(local);
  }
}
function transition_out(block, local, detach2, callback) {
  if (block && block.o) {
    if (outroing.has(block))
      return;
    outroing.add(block);
    outros.c.push(() => {
      outroing.delete(block);
      if (callback) {
        if (detach2)
          block.d(1);
        callback();
      }
    });
    block.o(local);
  }
}
var globals = typeof window !== "undefined" ? window : typeof globalThis !== "undefined" ? globalThis : global;
var boolean_attributes = new Set([
  "allowfullscreen",
  "allowpaymentrequest",
  "async",
  "autofocus",
  "autoplay",
  "checked",
  "controls",
  "default",
  "defer",
  "disabled",
  "formnovalidate",
  "hidden",
  "ismap",
  "loop",
  "multiple",
  "muted",
  "nomodule",
  "novalidate",
  "open",
  "playsinline",
  "readonly",
  "required",
  "reversed",
  "selected"
]);
function create_component(block) {
  block && block.c();
}
function mount_component(component, target, anchor, customElement) {
  const { fragment, on_mount, on_destroy, after_update } = component.$$;
  fragment && fragment.m(target, anchor);
  if (!customElement) {
    add_render_callback(() => {
      const new_on_destroy = on_mount.map(run).filter(is_function);
      if (on_destroy) {
        on_destroy.push(...new_on_destroy);
      } else {
        run_all(new_on_destroy);
      }
      component.$$.on_mount = [];
    });
  }
  after_update.forEach(add_render_callback);
}
function destroy_component(component, detaching) {
  const $$ = component.$$;
  if ($$.fragment !== null) {
    run_all($$.on_destroy);
    $$.fragment && $$.fragment.d(detaching);
    $$.on_destroy = $$.fragment = null;
    $$.ctx = [];
  }
}
function make_dirty(component, i) {
  if (component.$$.dirty[0] === -1) {
    dirty_components.push(component);
    schedule_update();
    component.$$.dirty.fill(0);
  }
  component.$$.dirty[i / 31 | 0] |= 1 << i % 31;
}
function init(component, options, instance3, create_fragment3, not_equal, props, append_styles2, dirty = [-1]) {
  const parent_component = current_component;
  set_current_component(component);
  const $$ = component.$$ = {
    fragment: null,
    ctx: null,
    props,
    update: noop,
    not_equal,
    bound: blank_object(),
    on_mount: [],
    on_destroy: [],
    on_disconnect: [],
    before_update: [],
    after_update: [],
    context: new Map(options.context || (parent_component ? parent_component.$$.context : [])),
    callbacks: blank_object(),
    dirty,
    skip_bound: false,
    root: options.target || parent_component.$$.root
  };
  append_styles2 && append_styles2($$.root);
  let ready = false;
  $$.ctx = instance3 ? instance3(component, options.props || {}, (i, ret, ...rest) => {
    const value = rest.length ? rest[0] : ret;
    if ($$.ctx && not_equal($$.ctx[i], $$.ctx[i] = value)) {
      if (!$$.skip_bound && $$.bound[i])
        $$.bound[i](value);
      if (ready)
        make_dirty(component, i);
    }
    return ret;
  }) : [];
  $$.update();
  ready = true;
  run_all($$.before_update);
  $$.fragment = create_fragment3 ? create_fragment3($$.ctx) : false;
  if (options.target) {
    if (options.hydrate) {
      start_hydrating();
      const nodes = children(options.target);
      $$.fragment && $$.fragment.l(nodes);
      nodes.forEach(detach);
    } else {
      $$.fragment && $$.fragment.c();
    }
    if (options.intro)
      transition_in(component.$$.fragment);
    mount_component(component, options.target, options.anchor, options.customElement);
    end_hydrating();
    flush();
  }
  set_current_component(parent_component);
}
var SvelteElement;
if (typeof HTMLElement === "function") {
  SvelteElement = class extends HTMLElement {
    constructor() {
      super();
      this.attachShadow({ mode: "open" });
    }
    connectedCallback() {
      const { on_mount } = this.$$;
      this.$$.on_disconnect = on_mount.map(run).filter(is_function);
      for (const key in this.$$.slotted) {
        this.appendChild(this.$$.slotted[key]);
      }
    }
    attributeChangedCallback(attr, _oldValue, newValue) {
      this[attr] = newValue;
    }
    disconnectedCallback() {
      run_all(this.$$.on_disconnect);
    }
    $destroy() {
      destroy_component(this, 1);
      this.$destroy = noop;
    }
    $on(type, callback) {
      const callbacks = this.$$.callbacks[type] || (this.$$.callbacks[type] = []);
      callbacks.push(callback);
      return () => {
        const index = callbacks.indexOf(callback);
        if (index !== -1)
          callbacks.splice(index, 1);
      };
    }
    $set($$props) {
      if (this.$$set && !is_empty($$props)) {
        this.$$.skip_bound = true;
        this.$$set($$props);
        this.$$.skip_bound = false;
      }
    }
  };
}
var SvelteComponent = class {
  $destroy() {
    destroy_component(this, 1);
    this.$destroy = noop;
  }
  $on(type, callback) {
    const callbacks = this.$$.callbacks[type] || (this.$$.callbacks[type] = []);
    callbacks.push(callback);
    return () => {
      const index = callbacks.indexOf(callback);
      if (index !== -1)
        callbacks.splice(index, 1);
    };
  }
  $set($$props) {
    if (this.$$set && !is_empty($$props)) {
      this.$$.skip_bound = true;
      this.$$set($$props);
      this.$$.skip_bound = false;
    }
  }
};
function dispatch_dev(type, detail) {
  document.dispatchEvent(custom_event(type, { version: "3.44.0", ...detail }, true));
}
function append_dev(target, node) {
  dispatch_dev("SvelteDOMInsert", { target, node });
  append(target, node);
}
function insert_dev(target, node, anchor) {
  dispatch_dev("SvelteDOMInsert", { target, node, anchor });
  insert(target, node, anchor);
}
function detach_dev(node) {
  dispatch_dev("SvelteDOMRemove", { node });
  detach(node);
}
function listen_dev(node, event, handler, options, has_prevent_default, has_stop_propagation) {
  const modifiers = options === true ? ["capture"] : options ? Array.from(Object.keys(options)) : [];
  if (has_prevent_default)
    modifiers.push("preventDefault");
  if (has_stop_propagation)
    modifiers.push("stopPropagation");
  dispatch_dev("SvelteDOMAddEventListener", { node, event, handler, modifiers });
  const dispose = listen(node, event, handler, options);
  return () => {
    dispatch_dev("SvelteDOMRemoveEventListener", { node, event, handler, modifiers });
    dispose();
  };
}
function set_data_dev(text2, data) {
  data = "" + data;
  if (text2.wholeText === data)
    return;
  dispatch_dev("SvelteDOMSetData", { node: text2, data });
  text2.data = data;
}
function validate_slots(name, slot, keys) {
  for (const slot_key of Object.keys(slot)) {
    if (!~keys.indexOf(slot_key)) {
      console.warn(`<${name}> received an unexpected slot "${slot_key}".`);
    }
  }
}
var SvelteComponentDev = class extends SvelteComponent {
  constructor(options) {
    if (!options || !options.target && !options.$$inline) {
      throw new Error("'target' is a required option");
    }
    super();
  }
  $destroy() {
    super.$destroy();
    this.$destroy = () => {
      console.warn("Component was already destroyed");
    };
  }
  $capture_state() {
  }
  $inject_state() {
  }
};

// app/memleak/Component.svelte
var file = "app/memleak/Component.svelte";
function create_fragment(ctx) {
  let button;
  let t0;
  let t1;
  let t2;
  let mounted;
  let dispose;
  const block = {
    c: function create() {
      button = document.createElementNS("https://svelte.dev/docs#svelte_options", "button");
      t0 = text("Clicked ");
      t1 = text(ctx[0]);
      t2 = text(" times");
      add_location(button, file, 7, 0, 101);
    },
    l: function claim(nodes) {
      throw new Error("options.hydrate only works if the component was compiled with the `hydratable: true` option");
    },
    m: function mount(target, anchor) {
      insert_dev(target, button, anchor);
      append_dev(button, t0);
      append_dev(button, t1);
      append_dev(button, t2);
      if (!mounted) {
        dispose = listen_dev(button, "click", ctx[1], false, false, false);
        mounted = true;
      }
    },
    p: function update2(ctx2, [dirty]) {
      if (dirty & 1)
        set_data_dev(t1, ctx2[0]);
    },
    i: noop,
    o: noop,
    d: function destroy(detaching) {
      if (detaching)
        detach_dev(button);
      mounted = false;
      dispose();
    }
  };
  dispatch_dev("SvelteRegisterBlock", {
    block,
    id: create_fragment.name,
    type: "component",
    source: "",
    ctx
  });
  return block;
}
function instance($$self, $$props, $$invalidate) {
  let { $$slots: slots = {}, $$scope } = $$props;
  validate_slots("Component", slots, []);
  let count = 0;
  const onClick = () => {
    $$invalidate(0, count = count + 1);
  };
  const writable_props = [];
  Object.keys($$props).forEach((key) => {
    if (!~writable_props.indexOf(key) && key.slice(0, 2) !== "$$" && key !== "slot")
      console.warn(`<Component> was created with unknown prop '${key}'`);
  });
  $$self.$capture_state = () => ({ count, onClick });
  $$self.$inject_state = ($$props2) => {
    if ("count" in $$props2)
      $$invalidate(0, count = $$props2.count);
  };
  if ($$props && "$$inject" in $$props) {
    $$self.$inject_state($$props.$$inject);
  }
  return [count, onClick];
}
var Component = class extends SvelteComponentDev {
  constructor(options) {
    super(options);
    init(this, options, instance, create_fragment, safe_not_equal, {});
    dispatch_dev("SvelteRegisterComponent", {
      component: this,
      tagName: "Component",
      options,
      id: create_fragment.name
    });
  }
};
var Component_default = Component;

// app/memleak/App.svelte
var file2 = "app/memleak/App.svelte";
function add_css(target) {
  append_styles(target, "svelte-urz98w", "button{margin:16px;;padding:4px;;border:1px solid #333;}");
}
function create_if_block(ctx) {
  let component;
  let current;
  component = new Component_default({ $$inline: true });
  const block = {
    c: function create() {
      create_component(component.$$.fragment);
    },
    m: function mount(target, anchor) {
      mount_component(component, target, anchor);
      current = true;
    },
    i: function intro(local) {
      if (current)
        return;
      transition_in(component.$$.fragment, local);
      current = true;
    },
    o: function outro(local) {
      transition_out(component.$$.fragment, local);
      current = false;
    },
    d: function destroy(detaching) {
      destroy_component(component, detaching);
    }
  };
  dispatch_dev("SvelteRegisterBlock", {
    block,
    id: create_if_block.name,
    type: "if",
    source: "(14:0) {#if isOpen}",
    ctx
  });
  return block;
}
function create_fragment2(ctx) {
  let button0;
  let t0;
  let t1;
  let t2;
  let button1;
  let t3;
  let current;
  let mounted;
  let dispose;
  let if_block = ctx[1] && create_if_block(ctx);
  const block = {
    c: function create() {
      button0 = document.createElementNS("https://svelte.dev/docs#svelte_options", "button");
      t0 = text("Click me");
      t1 = space();
      if (if_block)
        if_block.c();
      t2 = space();
      button1 = document.createElementNS("https://svelte.dev/docs#svelte_options", "button");
      t3 = text("Destroy all");
      add_location(button0, file2, 12, 0, 180);
      add_location(button1, file2, 16, 0, 262);
    },
    l: function claim(nodes) {
      throw new Error("options.hydrate only works if the component was compiled with the `hydratable: true` option");
    },
    m: function mount(target, anchor) {
      insert_dev(target, button0, anchor);
      append_dev(button0, t0);
      insert_dev(target, t1, anchor);
      if (if_block)
        if_block.m(target, anchor);
      insert_dev(target, t2, anchor);
      insert_dev(target, button1, anchor);
      append_dev(button1, t3);
      current = true;
      if (!mounted) {
        dispose = [
          listen_dev(button0, "click", ctx[2], false, false, false),
          listen_dev(button1, "click", function() {
            if (is_function(ctx[0]))
              ctx[0].apply(this, arguments);
          }, false, false, false)
        ];
        mounted = true;
      }
    },
    p: function update2(new_ctx, [dirty]) {
      ctx = new_ctx;
      if (ctx[1]) {
        if (if_block) {
          if (dirty & 2) {
            transition_in(if_block, 1);
          }
        } else {
          if_block = create_if_block(ctx);
          if_block.c();
          transition_in(if_block, 1);
          if_block.m(t2.parentNode, t2);
        }
      } else if (if_block) {
        group_outros();
        transition_out(if_block, 1, 1, () => {
          if_block = null;
        });
        check_outros();
      }
    },
    i: function intro(local) {
      if (current)
        return;
      transition_in(if_block);
      current = true;
    },
    o: function outro(local) {
      transition_out(if_block);
      current = false;
    },
    d: function destroy(detaching) {
      if (detaching)
        detach_dev(button0);
      if (detaching)
        detach_dev(t1);
      if (if_block)
        if_block.d(detaching);
      if (detaching)
        detach_dev(t2);
      if (detaching)
        detach_dev(button1);
      mounted = false;
      run_all(dispose);
    }
  };
  dispatch_dev("SvelteRegisterBlock", {
    block,
    id: create_fragment2.name,
    type: "component",
    source: "",
    ctx
  });
  return block;
}
function instance2($$self, $$props, $$invalidate) {
  let { $$slots: slots = {}, $$scope } = $$props;
  validate_slots("App", slots, []);
  let { destroy } = $$props;
  let isOpen = false;
  const onClick = () => {
    $$invalidate(1, isOpen = !isOpen);
  };
  const writable_props = ["destroy"];
  Object.keys($$props).forEach((key) => {
    if (!~writable_props.indexOf(key) && key.slice(0, 2) !== "$$" && key !== "slot")
      console.warn(`<App> was created with unknown prop '${key}'`);
  });
  $$self.$$set = ($$props2) => {
    if ("destroy" in $$props2)
      $$invalidate(0, destroy = $$props2.destroy);
  };
  $$self.$capture_state = () => ({ Component: Component_default, destroy, isOpen, onClick });
  $$self.$inject_state = ($$props2) => {
    if ("destroy" in $$props2)
      $$invalidate(0, destroy = $$props2.destroy);
    if ("isOpen" in $$props2)
      $$invalidate(1, isOpen = $$props2.isOpen);
  };
  if ($$props && "$$inject" in $$props) {
    $$self.$inject_state($$props.$$inject);
  }
  return [destroy, isOpen, onClick];
}
var App = class extends SvelteComponentDev {
  constructor(options) {
    super(options);
    init(this, options, instance2, create_fragment2, safe_not_equal, { destroy: 0 }, add_css);
    dispatch_dev("SvelteRegisterComponent", {
      component: this,
      tagName: "App",
      options,
      id: create_fragment2.name
    });
    const { ctx } = this.$$;
    const props = options.props || {};
    if (ctx[0] === void 0 && !("destroy" in props)) {
      console.warn("<App> was created without expected prop 'destroy'");
    }
  }
  get destroy() {
    return this.$$.ctx[0];
  }
  set destroy(destroy) {
    this.$$set({ destroy });
    flush();
  }
};
var App_default = App;

// app/memleak/index.ts
window.app = new App_default({
  target: document.getElementById("app"),
  props: {
    destroy: () => {
      window.app.$destroy();
      window.app = null;
      let node = document.getElementById("app");
      node.parentNode.removeChild(node);
    }
  }
});
