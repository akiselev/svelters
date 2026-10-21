<!-- kinds: NSMenu NSMenuItem NSPopUpButton NSView -->
<script>
  // Menus: a pop-up button whose items are menu-item children (with a submenu), and a context menu
  // in a view's `menu` slot (right-click it).
  let picked = $state('nothing');
  let size = $state(12);
</script>

<stack-view orientation="vertical" alignment="leading" spacing="8">
  <pop-up-button>
    <menu title="Fruit">
      <menu-item title="Apple" onaction={() => (picked = 'Apple')} />
      <menu-item title="Banana" key-equivalent="b" onaction={() => (picked = 'Banana')} />
      <menu-item title="Cherry" state="on" onaction={() => (picked = 'Cherry')} />
      <menu-item title="More">
        <menu title="More">
          <menu-item title="Damson" indentation-level={1} onaction={() => (picked = 'Damson')} />
          <menu-item title="Elderberry" enabled={false} />
        </menu>
      </menu-item>
    </menu>
  </pop-up-button>
  <text-field string-value={`picked: ${picked}`} editable={false} bezeled={false} draws-background={false} />
  <text-field string-value="Right-click the view below for a context menu" editable={false} bezeled={false} draws-background={false} />
  <view frame={{ x: 0, y: 0, width: 240, height: 40 }} tool-tip="context menu">
    <text-field frame={{ x: 4, y: 10, width: 220, height: 20 }} string-value={`font size ${size}`} editable={false} bezeled={false} draws-background={false} />
    <menu data-slot="menu" title="Context">
      <menu-item title="Bigger" onaction={() => size++} />
      <menu-item title="Smaller" onaction={() => size--} />
    </menu>
  </view>
</stack-view>
