<!-- backends: gtk -->
<script>
  let text = $state('a'), active = $state(false), pos = $state(0), once = $state('b'), pos0 = $state(0), view = $state(''), value = $state(1), checked = $state(false), level = $state(0);
</script>
<window>
  <box orientation="vertical">
    <entry bind:text />
    <check-button bind:active />
    <entry bind:cursor-position={pos} />
    <entry bind:text|nonreactive={once} />
    <entry bind:cursor-position|nonreactive={pos0} />
    <text-view bind:text={() => view, (v) => (view = v)} />
    <scale bind:value lower={0} upper={10} />
    <menu-bar>
      <menu label="File">
        <action label="Autosave" checkable bind:checked />
      </menu>
    </menu-bar>
    <level-bar bind:value={level} />
  </box>
</window>
