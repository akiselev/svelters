mod common;

#[test]
fn is_gtk_widget_prop() {
    common::eval(
        r#"
assert((new Button()).isGtkWidget, "Button is not a widget");
assert((new Box("horizontal")).isGtkWidget, "Button is not a widget");
"#,
    )
    .unwrap();
}
