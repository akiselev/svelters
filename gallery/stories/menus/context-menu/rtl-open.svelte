<frame dir="rtl" anchor="subject" frame-style="canvas" padding={12} height={300}>
  <label>انقر بزر الفأرة الأيمن على اللوحة</label>
  <context-menu open>
    <action label="قص" shortcut="Ctrl+X" />
    <action label="نسخ" shortcut="Ctrl+C" />
    <action label="لصق" shortcut="Ctrl+V" enabled={false} />
    <menu-separator />
    <action label="إظهار الشبكة" checkable checked />
    <action label="مليمتر" group="units" checked />
    <action label="بوصة" group="units" />
    <menu label="محاذاة">
      <action label="يسار" />
      <action label="يمين" />
    </menu>
  </context-menu>
</frame>
