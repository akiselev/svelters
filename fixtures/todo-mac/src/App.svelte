<script>
  // A styled todo list on AppKit: every colour, font, border and size below comes from the
  // <style> block through the decl dialect (metadata/style/appkit.yaml), with --sv-* tokens that
  // follow the colour scheme. `cargo run -p counter-mac -- --fixture todo-mac`.
  import { getColorScheme, setColorScheme } from '@svelters/native-runtime/style';

  let nextId = 4;
  let todos = $state([
    { id: 1, text: 'Try svelters on a Mac', done: true },
    { id: 2, text: 'Style the todo list', done: false },
    { id: 3, text: 'Ship it', done: false },
  ]);
  let draft = $state('');
  let filter = $state(0); // 0 all, 1 active, 2 done
  let dark = $state(getColorScheme() === 'dark');

  const shown = $derived(todos.filter((t) => (filter === 0 ? true : filter === 1 ? !t.done : t.done)));
  const left = $derived(todos.filter((t) => !t.done).length);
  const anyDone = $derived(todos.some((t) => t.done));

  function add() {
    const text = draft.trim();
    if (!text) return;
    todos.push({ id: nextId++, text, done: false });
    draft = '';
  }
  const toggle = (t) => (t.done = !t.done);
  const remove = (t) => (todos = todos.filter((x) => x !== t));
  const clearDone = () => (todos = todos.filter((t) => !t.done));
  function flipScheme() {
    dark = !dark;
    setColorScheme(dark ? 'dark' : 'light');
  }
</script>

<window title="Todos" content-size={{ width: 520, height: 600 }}>
  <stack-view class="app" orientation="vertical" alignment="width">
    <stack-view class="header" orientation="horizontal">
      <text-field class="title" string-value="Todos" editable={false} bezeled={false} draws-background={false} />
      <button class="ghost" title={dark ? 'Light' : 'Dark'} bezel-style="push" onaction={flipScheme} />
    </stack-view>

    <stack-view class="card" orientation="vertical" alignment="width">
      <stack-view class="add-row" orientation="horizontal">
        <text-field class="new" placeholder-string="What needs doing?" bind:string-value={draft} onaction={add} />
        <button class="primary" title="Add" bezel-style="push" enabled={draft.trim() !== ''} onaction={add} />
      </stack-view>

      <scroll-view class="scroller" has-vertical-scroller={true} autohides-scrollers={true} draws-background={false}>
        <stack-view class="list" orientation="vertical" alignment="width">
          {#each shown as t (t.id)}
            <stack-view class="row" class:done={t.done} orientation="horizontal">
              <button class="check" button-type="switch" title="" state={t.done ? 'on' : 'off'} onaction={() => toggle(t)} />
              <text-field class="label" string-value={t.text} editable={false} bezeled={false} draws-background={false} />
              <button class="delete" title="Delete" bezel-style="push" onaction={() => remove(t)} />
            </stack-view>
          {:else}
            <text-field class="empty" string-value={filter === 2 ? 'Nothing done yet' : 'All clear'} editable={false} bezeled={false} draws-background={false} />
          {/each}
        </stack-view>
      </scroll-view>
    </stack-view>

    <stack-view class="footer" orientation="horizontal">
      <text-field class="muted" string-value={`${left} ${left === 1 ? 'item' : 'items'} left`} editable={false} bezeled={false} draws-background={false} />
      <segmented-control labels={['All', 'Active', 'Done']} bind:selected-segment={filter} />
      <button class="link" title="Clear completed" bezel-style="push" enabled={anyDone} onaction={clearDone} />
    </stack-view>
  </stack-view>
</window>

<style>
  .app {
    gap: 16px;
    padding: 24px;
    background-color: var(--sv-bg);
  }
  .header {
    gap: 12px;
  }
  .title {
    font-size: 28px;
    font-weight: 700;
    color: var(--sv-fg);
    flex-grow: 1;
  }
  .card {
    flex-grow: 1;
    gap: 8px;
    padding: 12px;
    background-color: var(--sv-surface);
    border: 1px solid var(--sv-border);
    border-radius: 10px;
    box-shadow: 0 2px 10px rgba(0, 0, 0, 0.15);
  }
  .add-row {
    gap: 8px;
    padding: 4px;
  }
  .new {
    font-size: 15px;
    flex-grow: 1;
    height: 26px;
  }
  .new:focus {
    background-color: var(--sv-bg);
  }
  .primary {
    background-color: var(--sv-accent);
    color: var(--sv-accent-fg);
    border-radius: 6px;
    font-weight: 600;
    min-width: 64px;
    height: 26px;
  }
  .primary:hover {
    background-color: var(--sv-accent-hover);
  }
  .primary:active {
    opacity: 0.8;
  }
  .primary:disabled {
    opacity: 0.4;
  }
  .scroller {
    flex-grow: 1;
  }
  .list {
    gap: 2px;
  }
  .row {
    flex-grow: 0;
    gap: 10px;
    padding: 8px;
    border-radius: 6px;
  }
  .row:hover {
    background-color: rgba(127, 127, 127, 0.14);
  }
  .label {
    font-size: 14px;
    color: var(--sv-fg);
    flex-grow: 1;
  }
  .row.done .label {
    color: var(--sv-muted);
    text-decoration: line-through;
  }
  .delete {
    color: var(--sv-danger);
    font-size: 12px;
  }
  .empty {
    color: var(--sv-muted);
    font-style: italic;
    align-self: stretch;
    text-align: center;
  }
  .footer {
    gap: 12px;
  }
  .muted {
    color: var(--sv-muted);
    font-size: 12px;
    flex-grow: 1;
  }
  .link {
    color: var(--sv-accent);
  }
  .ghost {
    min-width: 64px;
  }
</style>
