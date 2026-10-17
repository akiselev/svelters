<script>
  import { getAllContexts, untrack } from 'svelte';
  import { ListController } from './controller.svelte.js';

  let {
    items,
    key = (_item, index) => index,
    row,
    layout = 'list',
    selection = 'single',
    selected = $bindable(-1),
    'row-height': rowHeight = 32,
    onactivate,
    ...rest
  } = $props();

  const contexts = getAllContexts();
  const ctl = new ListController({
    key: (item, index) => key(item, index),
    row: () => row,
    context: () => contexts,
    onactivate: (pos, item) => onactivate?.(item, pos),
  });
  let view = $state(null);
  let attached = false;
  let lastSelected = -1; // last position known natively: skips echoing a native change back

  $effect(() => {
    if (!view) return;
    if (!attached) {
      ctl.attach(view);
      attached = true;
    }
    ctl.update(items);
  });

  $effect(() => {
    const sel = selected;
    if (view && attached && sel !== lastSelected) {
      lastSelected = sel;
      untrack(() => view.setProp('selected', sel));
    }
  });

  $effect(() => () => ctl.detach());

  /** Scrolls item `index` into view. */
  export function scrollTo(index) {
    view?.call('scrollTo', index);
  }

  /** The controller (tests and tools). */
  export function controller() {
    return ctl;
  }
</script>

<virtual-list
  bind:this={view}
  {layout}
  {selection}
  row-height={rowHeight}
  onsetup={(_e, r) => ctl.setup(r)}
  onbind={(_e, r, pos, id, gen) => ctl.bind(r, pos, id, gen)}
  onunbind={(_e, r, gen) => ctl.unbind(r, gen)}
  onteardown={(_e, r) => ctl.teardown(r)}
  onactivate={(pos) => ctl.activate(pos)}
  onselect={(pos) => {
    lastSelected = pos;
    selected = pos;
  }}
  {...rest}
/>
