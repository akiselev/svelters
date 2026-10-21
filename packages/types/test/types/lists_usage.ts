// Type-checks `<ListView>` over native models (N3) against the generated model classes of both toolkits.
import { ListController, type ListViewProps, type NativeListModel } from '@svelters/lists';
import { File, FileInfo, ListStore } from 'svelters:gtk/gio';
import { CustomFilter, CustomSorter, DirectoryList, FilterListModel, SingleSelection, SortListModel, StringList, StringObject } from 'svelters:gtk/gtk';
import { QSortFilterProxyModel, QStringListModel } from 'svelters:qt/core';
import { QStandardItemModel } from 'svelters:qt/gui';

const dir = new DirectoryList({ file: File.newForPath('/usr/share')!, attributes: 'standard::display-name' });
const store = new ListStore({ itemType: StringObject });
store.sort((a, b) => (StringObject.from(a)!.string! < StringObject.from(b)!.string! ? -1 : 1));
const chain = new FilterListModel({
  model: new SortListModel({ model: store, sorter: CustomSorter.new(() => 0) }),
  filter: CustomFilter.new(() => true),
});
const gtkModels: NativeListModel[] = [dir, store, chain, new StringList({ strings: ['a'] }), new SingleSelection({ model: store })];
const qtModels: NativeListModel[] = [new QStringListModel(), new QStandardItemModel(), new QSortFilterProxyModel()];

const props: ListViewProps<FileInfo> = {
  items: dir,
  itemClass: FileInfo,
  row: (_anchor, item, index) => void [item().getDisplayName(), index() + 1],
  onactivate: (info, pos) => void [info.getName(), pos],
};
const arrayProps: ListViewProps<{ id: number }> = { items: [{ id: 1 }], key: (it) => it.id, row: () => {} };
const isModel: boolean = ListController.isNativeModel(dir);
void [gtkModels, qtModels, props, arrayProps, isModel];
