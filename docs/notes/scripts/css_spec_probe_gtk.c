// Probe: does GTK CSS count repeated class selectors (.a.a.a) in specificity?
#include <gtk/gtk.h>
static void probe(const char *css, const char *label) {
  GtkCssProvider *p = gtk_css_provider_new();
  gtk_css_provider_load_from_string(p, css);
  gtk_style_context_add_provider_for_display(gdk_display_get_default(), GTK_STYLE_PROVIDER(p), GTK_STYLE_PROVIDER_PRIORITY_APPLICATION);
  GtkWidget *w = gtk_window_new();
  GtkWidget *l = gtk_label_new("x");
  gtk_widget_add_css_class(l, "a");
  gtk_widget_add_css_class(l, "b");
  gtk_window_set_child(GTK_WINDOW(w), l);
  gtk_window_present(GTK_WINDOW(w));
  for (int i = 0; i < 50; i++) g_main_context_iteration(NULL, FALSE);
  GdkRGBA c;
  gtk_widget_get_color(l, &c);
  printf("%-48s -> r=%.0f g=%.0f b=%.0f\n", label, c.red * 255, c.green * 255, c.blue * 255);
  gtk_style_context_remove_provider_for_display(gdk_display_get_default(), GTK_STYLE_PROVIDER(p));
  gtk_window_destroy(GTK_WINDOW(w));
}
int main(void) {
  gtk_init();
  printf("GTK %d.%d.%d\n", gtk_get_major_version(), gtk_get_minor_version(), gtk_get_micro_version());
  probe(".a.a.a { color: rgb(255,0,0); } .a.b { color: rgb(0,0,255); }", "repeated .a.a.a then .a.b (red = counted)");
  probe(".a.b { color: rgb(0,0,255); } .a { color: rgb(255,0,0); }", "control: .a.b then .a (blue expected)");
  probe(".a { color: rgb(255,0,0); } .a { color: rgb(0,0,255); }", "control: equal spec, later wins (blue)");
  return 0;
}
