<frame anchor="subject" frame-style="canvas" padding={12} height={160}>
  <label>Right-click the board</label>
  <context-menu>
    <action label="Cut" shortcut="Ctrl+X" />
    <action label="Copy" shortcut="Ctrl+C" />
    <action label="Paste" shortcut="Ctrl+V" enabled={false} />
    <action label="Show grid" checkable checked />
    <action label="Millimetres" group="units" checked />
    <action label="Inches" group="units" />
    <action label="Properties of the selected footprint and its pads" />
  </context-menu>
</frame>
