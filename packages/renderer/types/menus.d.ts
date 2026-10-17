// Menus and actions (X4, gap G7) on GTK. Hand-written, because the GTK kinds have no GIR class
// (crates/svelters-gtk/src/backend/menus.rs). Qt's menus are real classes with native names
// (QAction `action`, QMenu `menu`, QMenuBar `menu-bar`, QToolBar `tool-bar`), typed in qt.d.ts.
import type { GtkWidgetProps } from './gtk';

type SvlHandler = (event?: any) => void;

/** `<action>`: one item. Plain, checkable (`checkable`) or a member of a radio set (`group`). */
export interface ActionProps {
  /** Item text. A `_` or `&` mnemonic marker is toolkit specific. */
  label?: string;
  /** Theme icon name. */
  icon?: string;
  /** `Ctrl+O`, `Ctrl+Shift+P`, `F5`, `Alt+Left`. Active while the action's root (menu-bar, tool-bar, menu-button) is in a window. */
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

/** `<menu>`: a submenu inside a menu or menu-bar, or the popup menu of a `menu-button` (as its child). */
export interface MenuProps {
  label?: string;
  icon?: string;
  enabled?: boolean;
}

/** `<menu-bar>` and `<tool-bar>` are widgets: place them in a vertical `box`. Their children are `action`, `menu-separator` and `menu`. */
export interface MenuBarProps extends GtkWidgetProps {}
export interface ToolBarProps extends GtkWidgetProps {}

/** `<context-menu>` attaches to its parent widget and opens on the secondary click. */
export interface ContextMenuProps {
  label?: string;
}

export interface MenuElements {
  action: ActionProps;
  menu: MenuProps;
  'menu-bar': MenuBarProps;
  'menu-separator': { visible?: boolean };
  'tool-bar': ToolBarProps;
  'context-menu': ContextMenuProps;
}
