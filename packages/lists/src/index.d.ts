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
