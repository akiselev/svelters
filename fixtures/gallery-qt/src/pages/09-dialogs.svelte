<!-- kinds: QLabel -->
<script>
  // Dialogs (dialog, messagebox, inputdialog, progressdialog, errormessage, filedialog,
  // colordialog, fontdialog, wizard, wizardpage) are toplevel kinds. A Svelte app cannot mount
  // a toplevel other than <window> yet (the renderer refuses any other tag at top level, gap G9),
  // so open() cannot be called on a mounted dialog through bind:this.
  // See docs/notes/a1-qt-dialogs.md and metadata/A1-GAPS.md G9. This page therefore
  // documents the surface instead of mounting dialogs; the facts are verified by the generated
  // native tests (metadata/verified/qt.json).
</script>

<box orientation="vertical" spacing="6">
  <label label="dialog: title, modal, sizeGripEnabled; accepted, rejected, finished; open, accept, reject, done" />
  <label label="messagebox: text, informativeText, detailedText, icon, textFormat, standardButtons" />
  <label label="inputdialog: labelText, textValue, intValue, intMinimum, intMaximum, inputMode; textValueChanged, intValueChanged" />
  <label label="progressdialog: value, minimum, maximum, labelText, autoClose, autoReset; canceled; reset, cancel" />
  <label label="errormessage: showMessage(text); filedialog: fileMode, acceptMode, viewMode, defaultSuffix, setDirectory" />
  <label label="colordialog: currentColor, options; fontdialog: currentFont, options; wizard: wizardStyle, options; wizardpage: title, subTitle" />
</box>
