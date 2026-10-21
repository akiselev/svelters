// G0: mounted from the started hook (start_with / run_with) or in the desktop order. Building the
// filter model calls the JS filter function (invoke_callback) during mount, before any loop turn.
import { StringList } from './types/gtk/gtk/StringList.js';
import { StringObject } from './types/gtk/gtk/StringObject.js';
import { CustomFilter } from './types/gtk/gtk/CustomFilter.js';
import { FilterListModel } from './types/gtk/gtk/FilterListModel.js';

const h = globalThis.__svelters_host;
const list = new StringList({ strings: ['apple', 'banana', 'cherry'] });
const filter = new CustomFilter();
filter.setFilterFunc((item) => StringObject.from(item).string.startsWith('b'));
const model = new FilterListModel({ model: list, filter });
console.log('mount items ' + model.getNItems());

const win = h.create('window');
const label = h.create('label');
h.setProp(label, 'label', 'hello');
h.insert(win, label, null);
console.log('label ' + label);
setTimeout(() => console.log('ALL DONE'), 10);
