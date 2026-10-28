<frame anchor="subject" frame-style="canvas" padding={12} height={300}>
  <label>Right-click the board</label>
  <context-menu open>
    <action label="Cut" shortcut="Ctrl+X" />
    <action label="Copy" shortcut="Ctrl+C" />
    <action label="Paste" shortcut="Ctrl+V" enabled={false} />
    <menu-separator />
    <action label="Show grid" checkable checked />
    <action label="Millimetres" group="units" checked />
    <action label="Inches" group="units" />
    <menu label="Align">
      <action label="Left" />
      <action label="Right" />
    </menu>
    <action label="Properties of the selected footprint and its pads" />
  </context-menu>
</frame>
