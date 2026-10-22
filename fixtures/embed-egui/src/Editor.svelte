<script>
  // EM2: a text field that moves between viewports (DOCK-W4). Every instance created emits `init`
  // with a runtime-wide counter, so a remount would show; the text reaches the host as the title.
  import { root } from 'svelters:embed';

  const r = root();
  globalThis.__editorInstances = (globalThis.__editorInstances ?? 0) + 1;
  r.emit('init', { instances: globalThis.__editorInstances });
  let text = $state('');
  $effect(() => {
    r.title = text;
  });
</script>

<box gap={4}>
  <text-edit anchor="edit" hint-text="Type here" bind:text />
  <label>in {r.viewport}, focused {r.focused}</label>
</box>
