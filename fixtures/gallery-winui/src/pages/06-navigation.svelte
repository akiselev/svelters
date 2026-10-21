<!-- kinds: TabView TabViewItem NavigationView NavigationViewItem SelectorBar SelectorBarItem -->
<stack-panel orientation="vertical" spacing="6">
  <tab-view height={120} is-add-tab-button-visible={false}>
    <tab-view-item header="First tab" is-closable={false}><text-block>First tab content</text-block></tab-view-item>
    <tab-view-item header="Second tab" is-closable={false}><text-block>Second tab content</text-block></tab-view-item>
  </tab-view>
  <navigation-view pane-display-mode="left" pane-title="Navigation view" is-settings-visible={false} height={220}>
    <navigation-view-item slot="menu-items" content="Home" />
    <navigation-view-item slot="menu-items" content="Documents">
      <navigation-view-item slot="menu-items" content="Recent" />
    </navigation-view-item>
    <navigation-view-item slot="footer-menu-items" content="Account" />
    <text-block>Navigation view content</text-block>
  </navigation-view>
  <selector-bar>
    <selector-bar-item text="Recent" />
    <selector-bar-item text="Shared" />
    <selector-bar-item text="Favorites" />
  </selector-bar>
</stack-panel>
