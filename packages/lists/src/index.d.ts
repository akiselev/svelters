// Types of @svelters/lists: `<ListView>` over a JS array (keyed diff) or a native list model (N3,
// plans/milestones/N3-native-models.md). The implementation is @svelters/native-runtime/lists.
import type { NativeObject } from '@svelters/types/core';

/** A GTK `GListModel` as the generated classes expose it (`Gio.ListStore`, `Gtk.DirectoryList`, `Gtk.FilterListModel`, ...). */
export interface GtkListModelLike {
  getItem(position: number): unknown;
  getNItems(): number;
}

/** A Qt `QAbstractItemModel` as the generated classes expose it (`QStringListModel`, `QStandardItemModel`, `QSortFilterProxyModel`, ...). */
export interface QtItemModelLike {
  rowCount(...args: any[]): number;
  index(...args: any[]): unknown;
}

/**
 * A native list model `<ListView>` shows directly (model mode): the toolkit's view reads it, JS keeps no
 * copy, no keys and no ids. GTK: any `GListModel`, a `GtkSelectionModel` included (shown as is). Qt: any
 * `QAbstractItemModel` (root rows of column 0).
 */
export type NativeListModel = NativeObject & (GtkListModelLike | QtItemModelLike);

export interface ListViewProps<T = any> {
  /** A JS array (or iterable; keyed diff, minimal splices) or a native list model (model mode). */
  items?: Iterable<T> | NativeListModel | null;
  /** A native list model; wins over `items`. */
  model?: NativeListModel | null;
  /** The key of an array item (array mode only; defaults to the index). */
  key?: (item: T, index: number) => unknown;
  /**
   * Native runtime, array mode (SV59): the row's label, its option's accessible name and type-ahead
   * text (as `<TreeView label>`).
   */
  label?: (item: T, index: number) => string;
  /**
   * Native runtime, array mode (SV59): the anchor of the row itself (the node assistive technology
   * sees as the option), so a combobox's `aria-activedescendant` can name it.
   */
  rowAnchor?: (item: T, index: number) => string;
  /**
   * Model mode, GTK: the generated class rows wrap their native item as (`FileInfo` for a
   * `DirectoryList`). An item that already has a wrapper keeps it (identity).
   */
  itemClass?: abstract new (...args: any[]) => T;
  /** Model mode: how a row finds its item (`raw` is the toolkit's item handle on GTK, null on Qt). */
  item?: (model: NativeListModel, position: number, raw: unknown) => T;
  /**
   * The row content (`{#snippet row(item, index)}`): an ordinary component instance per row slot, recycled
   * as the view scrolls. Typed like Svelte's `Snippet<[T, number]>` without depending on svelte.
   */
  row: (anchor: any, item: () => T, index: () => number) => void;
  layout?: 'list' | 'grid';
  selection?: 'none' | 'single';
  /** The selected position, -1 for none (`bind:selected`). */
  selected?: number;
  'row-height'?: number;
  onactivate?: (item: T, position: number) => void;
  [prop: string]: unknown;
}

/** `<ListView>`; `bind:this` gives `{ scrollTo(index), controller() }`. */
export declare function ListView<T = any>(anchor: any, props: ListViewProps<T>): { scrollTo(index: number): void; controller(): ListController };

export declare class ListController {
  /** Model mode: the native model shown, else null. */
  readonly model: NativeListModel | null;
  /** Native `splice` calls issued (array mode). */
  readonly splices: number;
  /** Change reports received from the native model (model mode). */
  readonly modelChanges: number;
  static isNativeModel(x: unknown): x is NativeListModel;
  indexOfKey(key: unknown): number;
}

export interface Splice {
  pos: number;
  removed: number;
  add: unknown[];
}
export declare function diffKeys(prev: readonly unknown[], next: readonly unknown[], maxOps?: number): Splice[];
export declare function applySplices<T>(list: T[], ops: readonly Splice[]): T[];
export declare function lis(seq: readonly number[]): number[];

/** A visible row of a `<TreeView>`, as `flattenTree` returns it. */
export interface TreeRow<T = any> {
  key: unknown;
  node: T;
  /** 0 for roots. */
  level: number;
  expandable: boolean;
  expanded: boolean;
}

export interface TreeViewProps<T = any> {
  roots: readonly T[];
  /** The node's children, an array or a Promise of one (loaded when the node first expands). Default `node.children`. */
  children?: (node: T) => readonly T[] | Promise<readonly T[]> | null | undefined;
  /** Whether the node shows a disclosure triangle. */
  hasChildren?: (node: T) => boolean;
  /** Default `node.id ?? node.key ?? node`. */
  key?: (node: T) => unknown;
  /** Type-ahead and the drag's `text/plain`. Default `node.label ?? node.name`. */
  label?: (node: T) => string;
  /** The drag's `rowDragType` entry. Default `JSON.stringify(key(node))`. */
  rowData?: (node: T) => string;
  /** `{#snippet row(node, level)}`; without it a row is a label. */
  row?: (anchor: any, node: () => T, level: () => number) => void;
  /** Shows the nodes that pass, and their ancestors (loaded children only). */
  filter?: (node: T) => boolean;
  selection?: 'none' | 'single' | 'multi' | 'toggle' | 'range';
  /** `bind:selected`: the selected keys; keys hidden by a filter or a collapse stay selected. */
  selected?: unknown[];
  /** `bind:expanded`: the expanded keys. */
  expanded?: unknown[];
  'row-height'?: number;
  /** Points per level. */
  indent?: number;
  /** Non-empty: rows drag, as this type (their `rowData`, a line each) plus `text/plain`. */
  rowDragType?: string;
  /** Non-empty: rows take drops of these types (`*`, `cad/*` patterns) and call `ondroprow`. */
  rowDropTypes?: string;
  onactivate?: (node: T, key: unknown) => void;
  onselect?: (keys: unknown[]) => void;
  onexpand?: (node: T, key: unknown) => void;
  oncollapse?: (node: T, key: unknown) => void;
  /** `key` is null for a drop below the last row. */
  ondroprow?: (key: unknown, where: 'before' | 'after' | 'into', type: string, data: string, node: T | undefined) => void;
  /** Ctrl+C; without it the selected rows go to `svelters:clipboard`. */
  oncopy?: (nodes: T[]) => void;
  [prop: string]: unknown;
}

/** `<TreeView>` (egui `tree-view`); `bind:this` gives `{ scrollTo(key), expand(key), collapse(key), controller() }`. */
export declare function TreeView<T = any>(
  anchor: any,
  props: TreeViewProps<T>,
): { scrollTo(key: unknown): void; expand(key: unknown): void; collapse(key: unknown): void; controller(): ListController };

export declare function flattenTree<T = any>(
  roots: readonly T[],
  opts: {
    key: (node: T) => unknown;
    childrenOf: (node: T, load: boolean) => readonly T[] | undefined;
    hasChildren: (node: T) => boolean;
    expanded: ReadonlySet<unknown>;
    filter?: (node: T) => boolean;
  },
): TreeRow<T>[];

/** OW5a1b: how the expand helpers read a tree (as on `<TreeView>`). */
export interface ExpandOptions<T = any> {
  key?: (node: T) => unknown;
  children?: (node: T) => readonly T[] | undefined;
}

/** The keys of every node with (loaded) children, for `bind:expanded` (Qt `expandAll`). */
export declare function expandAll<T = any>(roots: readonly T[], opts?: ExpandOptions<T>): unknown[];
/** `[]` (Qt `collapseAll`). */
export declare function collapseAll(): unknown[];
/** The nodes above `depth` open; 0 opens the roots (Qt `expandToDepth`). */
export declare function expandToDepth<T = any>(roots: readonly T[], depth: number, opts?: ExpandOptions<T>): unknown[];
/** `expanded` plus `node` and all its descendants (Qt `expandRecursively`). */
export declare function expandRecursively<T = any>(expanded: readonly unknown[] | undefined, node: T, opts?: ExpandOptions<T>): unknown[];
