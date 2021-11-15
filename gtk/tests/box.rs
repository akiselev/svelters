mod common;

#[test]
fn is_gtk_widget_prop() {
    common::eval(
        r#"
const btn = new GtkButton();
assert(btn.isGtkWidget, "Button is not a widget");
assert(btn.isGObject, "Button is not a widget");
const box = new GtkBox("horizontal");
assert(box.isGtkWidget, "Button is not a widget");
assert(box.isGObject, "Button is not a widget");
"#,
    )
    .unwrap();
}
