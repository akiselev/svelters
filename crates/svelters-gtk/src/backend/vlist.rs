//! The `virtuallist` kind: a JS-backed model behind `GtkListView`/`GtkGridView`
//! (plans/architecture/backend-abi.md, "Model-view protocol (M5)").
//!
//! The widget is a `GtkScrolledWindow` around the view. The model is a `GListModel` of ids
//! (`IdModel`); JS changes it with `splice`, and `n_items`/`get_item` never call JS. A
//! `GtkSignalListItemFactory` turns the toolkit's row lifecycle into the `setup`, `bind`, `unbind`
//! and `teardown` signals, which are delivered with `call_sync` so a row has its content before it
//! is first painted. When JS is already running (our own `splice` makes GTK unbind rows at once)
//! the host answers busy and queues the event; the `gen` argument lets JS discard a stale unbind.

use super::*;
use gtk::gio;
use gtk::glib::subclass::prelude::*;
use gtk::gio::subclass::prelude::*;

pub const KIND_VLIST: u32 = 0x4002;

const P_SELECTED: u32 = 0x4121;
const P_SELECTION: u32 = 0x4122;
const P_LAYOUT: u32 = 0x4123;
const P_ROW_HEIGHT: u32 = 0x4124;
const M_SPLICE: u32 = 0x4221;
const M_COUNT: u32 = 0x4222;
const M_SCROLL_TO: u32 = 0x4223;
const M_SET_ROW: u32 = 0x4224;
const M_ROW_AT: u32 = 0x4225;
const S_SETUP: u32 = 0x4321;
const S_BIND: u32 = 0x4322;
const S_UNBIND: u32 = 0x4323;
const S_TEARDOWN: u32 = 0x4324;
const S_ACTIVATE: u32 = 0x4325;
const S_SELECT: u32 = 0x4326;

/// Properties, methods and signals live in `0x4121..0x4326`; anything below `0x4000` is a generic
/// widget member of the underlying `GtkScrolledWindow`.
pub fn is_own_member(id: u32) -> bool {
    id >= 0x4000
}

pub fn find_member(what: u32, name: &str) -> u32 {
    match (what, name) {
        (SVL_PROP, "selected") => P_SELECTED,
        (SVL_PROP, "selection") => P_SELECTION,
        (SVL_PROP, "layout") => P_LAYOUT,
        (SVL_PROP, "rowHeight" | "row-height") => P_ROW_HEIGHT,
        (SVL_METHOD, "splice") => M_SPLICE,
        (SVL_METHOD, "count") => M_COUNT,
        (SVL_METHOD, "scrollTo") => M_SCROLL_TO,
        (SVL_METHOD, "setRow") => M_SET_ROW,
        (SVL_METHOD, "rowAt") => M_ROW_AT,
        (SVL_SIGNAL, "setup") => S_SETUP,
        (SVL_SIGNAL, "bind") => S_BIND,
        (SVL_SIGNAL, "unbind") => S_UNBIND,
        (SVL_SIGNAL, "teardown") => S_TEARDOWN,
        (SVL_SIGNAL, "activate") => S_ACTIVATE,
        (SVL_SIGNAL, "select") => S_SELECT,
        _ => 0,
    }
}

fn signal_bit(signal: u32) -> Option<u32> {
    (S_SETUP..=S_SELECT).contains(&signal).then(|| 1 << (signal - S_SETUP))
}

mod item_imp {
    use super::*;

    #[derive(Default)]
    pub struct IdItem {
        pub id: Cell<u64>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for IdItem {
        const NAME: &'static str = "SveltersIdItem";
        type Type = super::IdItem;
    }

    impl ObjectImpl for IdItem {}
}

glib::wrapper! {
    /// One item of the model: only the JS-assigned id.
    pub struct IdItem(ObjectSubclass<item_imp::IdItem>);
}

impl IdItem {
    fn new(id: u64) -> IdItem {
        let o: IdItem = glib::Object::new();
        o.imp().id.set(id);
        o
    }

    pub fn id(&self) -> u64 {
        self.imp().id.get()
    }
}

mod model_imp {
    use super::*;

    #[derive(Default)]
    pub struct IdModel {
        pub ids: RefCell<Vec<u64>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for IdModel {
        const NAME: &'static str = "SveltersIdModel";
        type Type = super::IdModel;
        type Interfaces = (gio::ListModel,);
    }

    impl ObjectImpl for IdModel {}

    impl ListModelImpl for IdModel {
        fn item_type(&self) -> glib::Type {
            IdItem::static_type()
        }

        fn n_items(&self) -> u32 {
            self.ids.borrow().len() as u32
        }

        fn item(&self, position: u32) -> Option<glib::Object> {
            self.ids.borrow().get(position as usize).map(|id| IdItem::new(*id).upcast())
        }
    }
}

glib::wrapper! {
    /// A `GListModel` over a vector of ids that JS replaces range by range.
    pub struct IdModel(ObjectSubclass<model_imp::IdModel>) @implements gio::ListModel;
}

impl IdModel {
    fn new() -> IdModel {
        glib::Object::new()
    }

    fn len(&self) -> usize {
        self.imp().ids.borrow().len()
    }

    /// Replaces `removed` ids at `pos` with `add` and raises one `items-changed`.
    fn splice(&self, pos: usize, removed: usize, add: &[u64]) -> Result<(), String> {
        {
            let mut ids = self.imp().ids.borrow_mut();
            if pos > ids.len() || removed > ids.len() - pos {
                return Err(format!("splice({pos}, {removed}) is out of range for {} items", ids.len()));
            }
            if add.len() == removed && add.len() < 64 {
                ids[pos..pos + removed].copy_from_slice(add);
            } else {
                ids.splice(pos..pos + removed, add.iter().copied());
            }
        }
        self.items_changed(pos as u32, removed as u32, add.len() as u32);
        Ok(())
    }
}

struct RowSlot {
    item: gtk::ListItem,
    epoch: u64,
}

pub(super) struct VList {
    sw: gtk::ScrolledWindow,
    view: RefCell<gtk::Widget>,
    grid: Cell<bool>,
    model: IdModel,
    selection: RefCell<gtk::SelectionModel>,
    sel_none: Cell<bool>,
    row_height: Cell<i64>,
    rows: Rc<RefCell<HashMap<u64, RowSlot>>>,
    next_row: Cell<u64>,
    /// Signals JS listens to, as `signal_bit` bits.
    listened: Cell<u32>,
    /// Set while a property write changes the selection, so it is not reported back.
    quiet: Cell<bool>,
}

fn row_of(item: &gtk::ListItem) -> u64 {
    unsafe { item.data::<u64>("sv-row").map(|p| *p.as_ref()).unwrap_or(0) }
}

fn i64_arg(args: &[svl_value], i: usize, what: &str) -> Result<i64, String> {
    let v = args.get(i).ok_or_else(|| format!("missing argument `{what}`"))?;
    match v.tag {
        SVL_I64 => Ok(unsafe { v.u.i }),
        SVL_F64 => Ok(unsafe { v.u.f } as i64),
        _ => Err(format!("`{what}` must be a number")),
    }
}

fn ids_arg(v: &svl_value) -> Result<Vec<u64>, String> {
    if v.tag != SVL_LIST {
        return Err("`ids` must be a list".into());
    }
    let l = unsafe { v.u.l };
    if l.len == 0 || l.ptr.is_null() {
        return Ok(Vec::new());
    }
    let items = unsafe { std::slice::from_raw_parts(l.ptr, l.len) };
    items
        .iter()
        .map(|x| match x.tag {
            SVL_I64 => Ok(unsafe { x.u.i } as u64),
            SVL_F64 => Ok(unsafe { x.u.f } as u64),
            _ => Err("`ids` must hold numbers".to_string()),
        })
        .collect()
}

fn int_value(i: i64) -> svl_value {
    svl_value::new(SVL_I64, svl_value_u { i })
}

impl Inner {
    pub(super) fn create_vlist(self: &Rc<Self>) -> Res<u64> {
        let sw = gtk::ScrolledWindow::new();
        let model = IdModel::new();
        let h = {
            let mut st = self.state.borrow_mut();
            st.next += 1;
            let h = st.next;
            st.entries.insert(
                h,
                Entry { obj: sw.upcast_ref::<glib::Object>().downgrade(), hold: Some(sw.clone().upcast()), class: SVL_LIFE_OWNED, owner: 0, note: std::ptr::null_mut(), kind: KIND_VLIST, signals: HashMap::new(), slot: None },
            );
            h
        };
        self.tag_object(sw.upcast_ref(), h);
        let selection: gtk::SelectionModel = {
            let s = gtk::SingleSelection::new(Some(model.clone()));
            s.set_autoselect(false);
            s.set_can_unselect(true);
            s.upcast()
        };
        let vl = Rc::new(VList {
            sw: sw.clone(),
            view: RefCell::new(gtk::Widget::from(gtk::Label::new(None))),
            grid: Cell::new(false),
            model,
            selection: RefCell::new(selection),
            sel_none: Cell::new(false),
            row_height: Cell::new(32),
            rows: Rc::new(RefCell::new(HashMap::new())),
            next_row: Cell::new(0),
            listened: Cell::new(0),
            quiet: Cell::new(false),
        });
        self.vlists.borrow_mut().insert(h, vl.clone());
        self.vl_build_view(h, &vl);
        self.vl_watch_selection(h, &vl);
        Ok(h)
    }

    pub(super) fn destroy_vlist(&self, h: u64) {
        let Some(vl) = self.vlists.borrow_mut().remove(&h) else { return };
        vl.sw.set_child(None::<&gtk::Widget>);
    }

    fn vl_of(&self, h: u64) -> Res<Rc<VList>> {
        self.vlists.borrow().get(&h).cloned().ok_or_else(|| format!("stale or foreign handle {h}"))
    }

    /// Raises a protocol signal if JS listens to it. `sync` signals use `call_sync` (JS runs now).
    fn vl_emit(&self, h: u64, signal: u32, args: &[i64], sync: bool) {
        let Some(bit) = signal_bit(signal) else { return };
        let listened = self.vlists.borrow().get(&h).map(|v| v.listened.get()).unwrap_or(0);
        if listened & bit == 0 {
            return;
        }
        let host = self.host.load(Ordering::Acquire);
        if host.is_null() {
            return;
        }
        let vals: Vec<svl_value> = args.iter().map(|i| int_value(*i)).collect();
        if sync {
            let mut out = convert::null_value();
            unsafe { ((*host).call_sync)((*host).self_, h, signal, vals.as_ptr(), vals.len(), &mut out) };
        } else {
            unsafe { ((*host).post_event)((*host).self_, h, signal, vals.as_ptr(), vals.len()) };
        }
    }

    fn vl_factory(self: &Rc<Self>, h: u64, vl: &Rc<VList>) -> gtk::SignalListItemFactory {
        let f = gtk::SignalListItemFactory::new();
        let weak = Rc::downgrade(self);
        let w = weak.clone();
        let v = Rc::downgrade(vl);
        f.connect_setup(move |_, obj| {
            let (Some(inner), Some(vl)) = (w.upgrade(), v.upgrade()) else { return };
            let Some(item) = obj.downcast_ref::<gtk::ListItem>() else { return };
            let row = vl.next_row.get() + 1;
            vl.next_row.set(row);
            unsafe { item.set_data("sv-row", row) };
            // Until JS provides the row's widget (it may be busy, so `setup` can arrive late) a
            // placeholder of the minimum height stands in; an empty slot would measure 0px and the
            // first layout would create a slot for every item that fits in 0px.
            let min = vl.row_height.get();
            if min > 0 {
                let ph = gtk::Box::new(gtk::Orientation::Horizontal, 0);
                ph.set_size_request(-1, min as i32);
                item.set_child(Some(&ph));
            }
            vl.rows.borrow_mut().insert(row, RowSlot { item: item.clone(), epoch: 0 });
            inner.vl_emit(h, S_SETUP, &[row as i64], true);
        });
        let w = weak.clone();
        let v = Rc::downgrade(vl);
        f.connect_bind(move |_, obj| {
            let (Some(inner), Some(vl)) = (w.upgrade(), v.upgrade()) else { return };
            let Some(item) = obj.downcast_ref::<gtk::ListItem>() else { return };
            let row = row_of(item);
            let id = item.item().and_downcast::<IdItem>().map(|i| i.id()).unwrap_or(0);
            let epoch = {
                let mut rows = vl.rows.borrow_mut();
                let Some(slot) = rows.get_mut(&row) else { return };
                slot.epoch += 1;
                slot.epoch
            };
            inner.vl_emit(h, S_BIND, &[row as i64, item.position() as i64, id as i64, epoch as i64], true);
        });
        let w = weak.clone();
        let v = Rc::downgrade(vl);
        f.connect_unbind(move |_, obj| {
            let (Some(inner), Some(vl)) = (w.upgrade(), v.upgrade()) else { return };
            let Some(item) = obj.downcast_ref::<gtk::ListItem>() else { return };
            let row = row_of(item);
            let epoch = vl.rows.borrow().get(&row).map(|s| s.epoch).unwrap_or(0);
            inner.vl_emit(h, S_UNBIND, &[row as i64, epoch as i64], true);
        });
        let w = weak;
        let v = Rc::downgrade(vl);
        f.connect_teardown(move |_, obj| {
            let (Some(inner), Some(vl)) = (w.upgrade(), v.upgrade()) else { return };
            let Some(item) = obj.downcast_ref::<gtk::ListItem>() else { return };
            let row = row_of(item);
            inner.vl_emit(h, S_TEARDOWN, &[row as i64], true);
            vl.rows.borrow_mut().remove(&row);
            item.set_child(None::<&gtk::Widget>);
        });
        f
    }

    /// (Re)creates the `GtkListView`/`GtkGridView` for the current layout and selection model.
    fn vl_build_view(self: &Rc<Self>, h: u64, vl: &Rc<VList>) {
        let factory = self.vl_factory(h, vl);
        let sel = vl.selection.borrow().clone();
        let weak = Rc::downgrade(self);
        let activate = move |pos: u32| {
            if let Some(inner) = weak.upgrade() {
                inner.vl_emit(h, S_ACTIVATE, &[pos as i64], false);
            }
        };
        let view: gtk::Widget = if vl.grid.get() {
            let v = gtk::GridView::new(Some(sel), Some(factory));
            v.connect_activate(move |_, pos| activate(pos));
            v.upcast()
        } else {
            let v = gtk::ListView::new(Some(sel), Some(factory));
            v.connect_activate(move |_, pos| activate(pos));
            v.upcast()
        };
        vl.sw.set_child(Some(&view));
        *vl.view.borrow_mut() = view;
    }

    fn vl_watch_selection(self: &Rc<Self>, h: u64, vl: &Rc<VList>) {
        let Some(single) = vl.selection.borrow().clone().downcast::<gtk::SingleSelection>().ok() else { return };
        let weak = Rc::downgrade(self);
        let v = Rc::downgrade(vl);
        single.connect_selected_notify(move |s| {
            let (Some(inner), Some(vl)) = (weak.upgrade(), v.upgrade()) else { return };
            if vl.quiet.get() {
                return;
            }
            let pos = if s.selected() == gtk::INVALID_LIST_POSITION { -1 } else { s.selected() as i64 };
            inner.vl_emit(h, S_SELECT, &[pos], false);
        });
    }

    fn vl_set_selection_mode(self: &Rc<Self>, h: u64, vl: &Rc<VList>, none: bool) {
        if vl.sel_none.get() == none {
            return;
        }
        vl.sel_none.set(none);
        let sel: gtk::SelectionModel = if none {
            gtk::NoSelection::new(Some(vl.model.clone())).upcast()
        } else {
            let s = gtk::SingleSelection::new(Some(vl.model.clone()));
            s.set_autoselect(false);
            s.set_can_unselect(true);
            s.upcast()
        };
        *vl.selection.borrow_mut() = sel;
        self.vl_build_view(h, vl);
        self.vl_watch_selection(h, vl);
    }

    pub(super) fn vlist_set_prop(self: &Rc<Self>, h: u64, prop: u32, v: &svl_value) -> Res<()> {
        let vl = self.vl_of(h)?;
        match prop {
            P_SELECTED => {
                let pos = match v.tag {
                    SVL_NULL => -1,
                    SVL_I64 => unsafe { v.u.i },
                    SVL_F64 => (unsafe { v.u.f }) as i64,
                    _ => return Err("`selected` must be a number".into()),
                };
                if let Some(single) = vl.selection.borrow().clone().downcast::<gtk::SingleSelection>().ok() {
                    vl.quiet.set(true);
                    single.set_selected(if pos < 0 { gtk::INVALID_LIST_POSITION } else { pos as u32 });
                    vl.quiet.set(false);
                }
                Ok(())
            }
            P_SELECTION => {
                let s = unsafe { convert::as_str(v) }.ok_or("`selection` must be a string")?;
                match &*s {
                    "none" => self.vl_set_selection_mode(h, &vl, true),
                    "single" => self.vl_set_selection_mode(h, &vl, false),
                    other => return Err(format!("unknown selection mode `{other}` (none | single)")),
                }
                Ok(())
            }
            P_LAYOUT => {
                let s = unsafe { convert::as_str(v) }.ok_or("`layout` must be a string")?;
                let grid = match &*s {
                    "list" => false,
                    "grid" => true,
                    other => return Err(format!("unknown layout `{other}` (list | grid)")),
                };
                if vl.grid.get() != grid {
                    vl.grid.set(grid);
                    self.vl_build_view(h, &vl);
                }
                Ok(())
            }
            P_ROW_HEIGHT => {
                vl.row_height.set(i64_arg(std::slice::from_ref(v), 0, "rowHeight")?);
                Ok(())
            }
            _ => Err("unknown property".into()),
        }
    }

    pub(super) fn vlist_get_prop(&self, h: u64, prop: u32, out: &mut svl_value) -> Res<()> {
        let vl = self.vl_of(h)?;
        let mut buf = self.out.borrow_mut();
        *buf = StrBuf::default();
        *out = match prop {
            P_SELECTED => {
                let pos = vl.selection.borrow().clone().downcast::<gtk::SingleSelection>().ok().map(|s| s.selected()).unwrap_or(gtk::INVALID_LIST_POSITION);
                int_value(if pos == gtk::INVALID_LIST_POSITION { -1 } else { pos as i64 })
            }
            P_SELECTION => buf.str_value(if vl.sel_none.get() { "none" } else { "single" }),
            P_LAYOUT => buf.str_value(if vl.grid.get() { "grid" } else { "list" }),
            P_ROW_HEIGHT => int_value(vl.row_height.get()),
            _ => return Err("unknown property".into()),
        };
        Ok(())
    }

    pub(super) fn vlist_listen(&self, h: u64, signal: u32, on: bool) -> Res<bool> {
        let Some(bit) = signal_bit(signal) else { return Ok(false) };
        let vl = self.vl_of(h)?;
        vl.listened.set(if on { vl.listened.get() | bit } else { vl.listened.get() & !bit });
        Ok(true)
    }

    pub(super) fn vlist_call(&self, h: u64, method: u32, args: &[svl_value], out: &mut svl_value) -> Result<(), CallErr> {
        let vl = self.vl_of(h)?;
        *out = convert::null_value();
        match method {
            M_SPLICE => {
                let pos = i64_arg(args, 0, "pos")?;
                let removed = i64_arg(args, 1, "removed")?;
                let ids = ids_arg(args.get(2).ok_or("missing argument `ids`")?)?;
                if pos < 0 || removed < 0 {
                    return Err("splice: negative range".into());
                }
                // `items-changed` runs GTK's callbacks (rows unbind, selection moves) before this returns.
                vl.model.splice(pos as usize, removed as usize, &ids)?;
            }
            M_COUNT => *out = int_value(vl.model.len() as i64),
            M_SCROLL_TO => {
                let pos = i64_arg(args, 0, "pos")?;
                if pos < 0 || pos as usize >= vl.model.len() {
                    return Err(format!("scrollTo: position {pos} is out of range").into());
                }
                // `list.scroll-to-item` exists on every GtkListBase since GTK 4.0 (the typed API needs 4.12).
                let view = vl.view.borrow().clone();
                view.activate_action("list.scroll-to-item", Some(&(pos as u32).to_variant())).map_err(|e| e.to_string())?;
            }
            M_SET_ROW => {
                let row = i64_arg(args, 0, "row")? as u64;
                let item = vl.rows.borrow().get(&row).map(|s| s.item.clone());
                // A row torn down before JS answered (a deferred event) is not an error.
                let Some(item) = item else { return Ok(()) };
                match args.get(1) {
                    Some(v) if v.tag == SVL_HANDLE => {
                        let (w, _) = self.widget_of(unsafe { v.u.h })?;
                        // A row that is still empty (its content is bound later) would measure 0px
                        // and the first layout would create a row slot per item that fits in 0px.
                        let min = vl.row_height.get();
                        if min > 0 {
                            w.set_size_request(-1, min as i32);
                        }
                        item.set_child(Some(&w));
                    }
                    Some(v) if v.tag == SVL_NULL => item.set_child(None::<&gtk::Widget>),
                    _ => return Err("setRow: expected a widget handle or null".into()),
                }
            }
            M_ROW_AT => {
                let pos = i64_arg(args, 0, "pos")?;
                let found = vl.rows.borrow().iter().find(|(_, s)| s.item.position() as i64 == pos && s.item.item().is_some()).map(|(r, _)| *r as i64);
                *out = int_value(found.unwrap_or(-1));
            }
            _ => return Err("unknown method".into()),
        }
        Ok(())
    }
}
