// Menus and actions (X4, gap G7): the portable elements both toolkits provide. Hand-written, because
// the GTK kinds have no GIR class (crates/svelters-gtk/src/backend/menus.rs) and the Qt ones are
// generated from QAction.yaml / QMenu.yaml / QMenuBar.yaml / QToolBar.yaml; both agree on these names.
import type { GtkWidgetProps } from './gtk';

type SvlHandler = (event?: any) => void;

/** `<action>`: one item. Plain, checkable (`checkable`) or a member of a radio set (`group`). */
export interface ActionProps {
  /** Item text. A `_` or `&` mnemonic marker is toolkit specific. */
  label?: string;
  /** Theme icon name. */
  icon?: string;
  /** `Ctrl+O`, `Ctrl+Shift+P`, `F5`, `Alt+Left`. Active while the action's root (menubar, toolbar, menubutton) is in a window. */
  shortcut?: string;
  tooltip?: string;
  enabled?: boolean;
  visible?: boolean;
  /** The item has a check state (`checked`, `bind:checked`). Implied by `group`. */
  checkable?: boolean;
  checked?: boolean;
  /** Actions of one parent with the same group name are exclusive (radio items); each keeps its own `checked`. */
  group?: string;
  /** The user activated the item (click, Enter, shortcut). The argument is the new checked state (false when not checkable). */
  onactivate?: (checked: boolean) => void;
  /** The checked state changed because of the user (JS writes do not report back). */
  ontoggled?: (checked: boolean) => void;
}

/** `<menu>`: a submenu inside a menu or menubar, or the popup menu of a `menubutton` (as its child). */
export interface MenuProps {
  label?: string;
  icon?: string;
  enabled?: boolean;
}

/** `<menubar>` and `<toolbar>` are widgets: place them in a vertical `box`. Their children are `action`, `menuseparator` and `menu`. */
export interface MenuBarProps extends GtkWidgetProps {}
export interface ToolBarProps extends GtkWidgetProps {}

/** `<contextmenu>` attaches to its parent widget and opens on the secondary click. */
export interface ContextMenuProps {
  label?: string;
}

export interface MenuElements {
  action: ActionProps;
  menu: MenuProps;
  menubar: MenuBarProps;
  menuseparator: { visible?: boolean };
  toolbar: ToolBarProps;
  contextmenu: ContextMenuProps;
}
