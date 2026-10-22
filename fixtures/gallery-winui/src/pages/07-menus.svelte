<!-- kinds: MenuBar MenuBarItem MenuFlyoutItem MenuFlyoutSeparator ToggleMenuFlyoutItem MenuFlyoutSubItem CommandBar AppBarButton AppBarToggleButton AppBarSeparator -->
<script>
  let last = $state('none');
  let wrap = $state(false);
</script>

<stack-panel orientation="vertical" spacing="6">
  <menu-bar>
    <menu-bar-item title="File">
      <menu-flyout-item text="New" onclick={() => (last = 'new')} />
      <menu-flyout-item text="Open" onclick={() => (last = 'open')} />
      <menu-flyout-separator />
      <menu-flyout-sub-item text="Recent">
        <menu-flyout-item text="notes.txt" onclick={() => (last = 'notes.txt')} />
      </menu-flyout-sub-item>
    </menu-bar-item>
    <menu-bar-item title="View">
      <toggle-menu-flyout-item text="Word wrap" is-checked={wrap} onclick={() => (wrap = !wrap)} />
    </menu-bar-item>
  </menu-bar>
  <command-bar default-label-position="right">
    <app-bar-button label="Add" onclick={() => (last = 'add')} />
    <app-bar-separator />
    <app-bar-toggle-button label="Bold" />
    <app-bar-button slot="secondary-commands" label="Settings" onclick={() => (last = 'settings')} />
  </command-bar>
  <text-block>Last command: {last}; word wrap: {wrap ? 'on' : 'off'}</text-block>
  <border padding="8" background="#10808080">
    <text-block>Right-click here (context flyout)</text-block>
    <menu-flyout slot="context-flyout">
      <menu-flyout-item text="Copy" onclick={() => (last = 'copy')} />
    </menu-flyout>
  </border>
</stack-panel>
