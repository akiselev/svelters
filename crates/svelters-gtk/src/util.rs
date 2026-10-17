use gtk::prelude::*;

/// Widget after which a child inserted before `before` must be placed: the previous sibling of
/// `before`, or the last child of `parent` when appending.
pub fn prev_sibling(parent: &gtk::Widget, before: Option<&gtk::Widget>) -> Option<gtk::Widget> {
    match before {
        Some(b) => b.prev_sibling(),
        None => parent.last_child(),
    }
}
