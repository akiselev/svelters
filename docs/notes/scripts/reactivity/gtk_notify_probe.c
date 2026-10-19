/* Observability probe for GTK 4 (plans/research/REACTIVITY.md section 3.1).
 *
 * Part A (sweep): every instantiable Gtk* GObject type that g_object_new() builds bare. For each
 * readable + writable, not construct-only, not deprecated property of a scalar type (bool, ints,
 * double, float, string, enum) declared by that class: set a new value, then set the read-back
 * value again, recording every `notify` emission. Prints one TSV row per property:
 *   owner prop explicit readback notify_on_change(count) notify_on_same cross-notified-props
 * Part B (scenarios): changes that are not property sets (buffer edits, clamping, freeze/thaw,
 * notebook page removal), counting notify emissions.
 *
 * Build: cc -O1 -o /tmp/gtk_notify_probe gtk_notify_probe.c $(pkg-config --cflags --libs gtk4)
 * Run:   ./run_gtk_notify_probe.sh > rows.tsv   (reruns with SVL_SKIP until no type crashes)
 */
#include <gtk/gtk.h>
#include <string.h>

static GPtrArray *log_names;

static void on_notify(GObject *o, GParamSpec *p, gpointer d) { g_ptr_array_add(log_names, (gpointer)g_intern_string(p->name)); }

static int count_name(const char *name) {
  int n = 0;
  for (guint i = 0; i < log_names->len; i++) n += g_ptr_array_index(log_names, i) == g_intern_string(name);
  return n;
}

static void drain(void) { while (g_main_context_pending(NULL)) g_main_context_iteration(NULL, FALSE); }

static gboolean scalar(GType t) {
  switch (G_TYPE_FUNDAMENTAL(t)) {
  case G_TYPE_BOOLEAN: case G_TYPE_INT: case G_TYPE_UINT: case G_TYPE_INT64: case G_TYPE_UINT64:
  case G_TYPE_DOUBLE: case G_TYPE_FLOAT: case G_TYPE_STRING: case G_TYPE_ENUM: return TRUE;
  default: return FALSE;
  }
}

/* A value different from cur within the pspec's range; FALSE if none. */
static gboolean next_value(GParamSpec *p, const GValue *cur, GValue *nv) {
  g_value_init(nv, p->value_type);
  switch (G_TYPE_FUNDAMENTAL(p->value_type)) {
  case G_TYPE_BOOLEAN: g_value_set_boolean(nv, !g_value_get_boolean(cur)); return TRUE;
#define INT_CASE(T, get, set, PS)                                                    \
  {                                                                               \
    PS *s = (PS *)p; T c = get(cur);                                               \
    T v = c < s->maximum ? c + 1 : c - 1;                                          \
    if (v < s->minimum || v > s->maximum || v == c) return FALSE;                 \
    set(nv, v); return TRUE;                                                       \
  }
  case G_TYPE_INT: INT_CASE(gint, g_value_get_int, g_value_set_int, GParamSpecInt)
  case G_TYPE_UINT: INT_CASE(guint, g_value_get_uint, g_value_set_uint, GParamSpecUInt)
  case G_TYPE_INT64: INT_CASE(gint64, g_value_get_int64, g_value_set_int64, GParamSpecInt64)
  case G_TYPE_UINT64: INT_CASE(guint64, g_value_get_uint64, g_value_set_uint64, GParamSpecUInt64)
  case G_TYPE_DOUBLE: {
    GParamSpecDouble *s = (GParamSpecDouble *)p; double c = g_value_get_double(cur);
    double v = c + 1.0 <= s->maximum ? c + 1.0 : c - 0.5;
    if (v < s->minimum || v > s->maximum) return FALSE;
    g_value_set_double(nv, v); return TRUE;
  }
  case G_TYPE_FLOAT: {
    GParamSpecFloat *s = (GParamSpecFloat *)p; float c = g_value_get_float(cur);
    float v = c + 1.0f <= s->maximum ? c + 1.0f : c - 0.5f;
    if (v < s->minimum || v > s->maximum) return FALSE;
    g_value_set_float(nv, v); return TRUE;
  }
  case G_TYPE_STRING: {
    const char *c = g_value_get_string(cur);
    char *v = g_strconcat(c ? c : "", "x", NULL);
    g_value_take_string(nv, v); return TRUE;
  }
  case G_TYPE_ENUM: {
    GEnumClass *ec = ((GParamSpecEnum *)p)->enum_class; int c = g_value_get_enum(cur);
    for (guint i = 0; i < ec->n_values; i++)
      if (ec->values[i].value != c) { g_value_set_enum(nv, ec->values[i].value); return TRUE; }
    return FALSE;
  }
  }
  return FALSE;
}

static const char *SKIP_TYPES[] = {"GtkPrintUnixDialog", "GtkPageSetupUnixDialog", "GtkDragIcon", "GtkTooltip",
                                   "GtkPrintJob", "GtkMountOperation", NULL};
static const char *SKIP_PROPS[] = {"visible", "has-focus", "is-focus", "can-target", "parent", "root", NULL};

static gboolean in_list(const char *s, const char **l) {
  for (; *l; l++) if (!strcmp(s, *l)) return TRUE;
  return FALSE;
}

/* Types that crashed in an earlier run (the driver passes them in SVL_SKIP, space separated). */
static gboolean env_skip(const char *t) {
  const char *e = g_getenv("SVL_SKIP");
  if (!e) return FALSE;
  char **v = g_strsplit(e, " ", -1);
  gboolean hit = g_strv_contains((const char *const *)v, t);
  g_strfreev(v);
  return hit;
}

static void probe_type(GType t) {
  if (G_TYPE_IS_ABSTRACT(t) || !g_str_has_prefix(g_type_name(t), "Gtk") || in_list(g_type_name(t), SKIP_TYPES) || env_skip(g_type_name(t))) return;
  printf("#begin\t%s\n", g_type_name(t));
  GObject *o = g_object_new(t, NULL);
  if (!o) return;
  if (g_object_is_floating(o)) g_object_ref_sink(o);
  gulong id = g_signal_connect(o, "notify", G_CALLBACK(on_notify), NULL);
  guint n;
  GParamSpec **ps = g_object_class_list_properties(G_OBJECT_GET_CLASS(o), &n);
  for (guint i = 0; i < n; i++) {
    GParamSpec *p = ps[i];
    if (p->owner_type != t) continue;
    if (!(p->flags & G_PARAM_READABLE) || !(p->flags & G_PARAM_WRITABLE) || (p->flags & G_PARAM_CONSTRUCT_ONLY) ||
        (p->flags & G_PARAM_DEPRECATED) || !scalar(p->value_type) || in_list(p->name, SKIP_PROPS))
      continue;
    GValue cur = G_VALUE_INIT, nv = G_VALUE_INIT, after = G_VALUE_INIT;
    g_value_init(&cur, p->value_type);
    g_object_get_property(o, p->name, &cur);
    if (!next_value(p, &cur, &nv)) { g_value_unset(&cur); g_value_unset(&nv); continue; }
    g_ptr_array_set_size(log_names, 0);
    g_object_set_property(o, p->name, &nv);
    drain();
    g_value_init(&after, p->value_type);
    g_object_get_property(o, p->name, &after);
    int on_change = count_name(p->name);
    GString *cross = g_string_new("");
    for (guint k = 0; k < log_names->len; k++) {
      const char *nm = g_ptr_array_index(log_names, k);
      if (nm != g_intern_string(p->name) && !strstr(cross->str, nm)) g_string_append_printf(cross, "%s%s", cross->len ? "," : "", nm);
    }
    const char *rb = g_param_values_cmp(p, &after, &nv) == 0 ? "exact" : g_param_values_cmp(p, &after, &cur) == 0 ? "ignored" : "normalized";
    g_ptr_array_set_size(log_names, 0);
    g_object_set_property(o, p->name, &after);
    drain();
    int on_same = count_name(p->name);
    printf("%s\t%s\t%d\t%s\t%d\t%d\t%s\n", g_type_name(t), p->name, (p->flags & G_PARAM_EXPLICIT_NOTIFY) ? 1 : 0, rb, on_change,
           on_same, cross->str);
    g_string_free(cross, TRUE);
    g_value_unset(&cur); g_value_unset(&nv); g_value_unset(&after);
  }
  g_free(ps);
  g_signal_handler_disconnect(o, id);
  if (GTK_IS_WINDOW(o)) gtk_window_destroy(GTK_WINDOW(o));
  g_object_unref(o);
  drain();
}

static void walk(GType t) {
  if (G_TYPE_IS_INSTANTIATABLE(t)) probe_type(t);
  guint n;
  GType *kids = g_type_children(t, &n);
  for (guint i = 0; i < n; i++) walk(kids[i]);
  g_free(kids);
}

/* ---- Part B ---- */
typedef struct { const char *names[6]; int counts[6]; } Counter;
static void on_count(GObject *o, GParamSpec *p, Counter *c) {
  for (int i = 0; c->names[i]; i++) if (!strcmp(c->names[i], p->name)) c->counts[i]++;
}
static void watch(gpointer obj, Counter *c) { memset(c->counts, 0, sizeof c->counts); g_signal_connect(obj, "notify", G_CALLBACK(on_count), c); }
static void report(const char *what, Counter *c) {
  printf("#scenario\t%s", what);
  for (int i = 0; c->names[i]; i++) printf("\t%s=%d", c->names[i], c->counts[i]);
  printf("\n");
  memset(c->counts, 0, sizeof c->counts);
}
static int changed_count;
static void on_changed(void) { changed_count++; }

static void scenarios(void) {
  GtkTextBuffer *b = gtk_text_buffer_new(NULL);
  Counter cb = {{"text", "cursor-position", "has-selection", "can-undo", "can-redo", NULL}};
  watch(b, &cb);
  g_signal_connect(b, "changed", G_CALLBACK(on_changed), NULL);
  gtk_text_buffer_insert_at_cursor(b, "hello", -1);
  report("textbuffer insert_at_cursor", &cb);
  GtkTextIter s, e;
  gtk_text_buffer_get_bounds(b, &s, &e);
  gtk_text_buffer_select_range(b, &s, &e);
  report("textbuffer select_range(all)", &cb);
  gtk_text_buffer_get_bounds(b, &s, &e);
  gtk_text_buffer_delete(b, &s, &e);
  report("textbuffer delete(all)", &cb);
  gtk_text_buffer_set_text(b, "abc", -1);
  gtk_text_buffer_set_text(b, "abc", -1);
  report("textbuffer set_text abc,abc", &cb);
  for (int i = 0; i < 100; i++) gtk_text_buffer_insert_at_cursor(b, "x", 1);
  report("textbuffer 100 x insert_at_cursor", &cb);
  printf("#scenario\ttextbuffer changed signals total\t%d\n", changed_count);

  GtkAdjustment *a = g_object_ref_sink(gtk_adjustment_new(0, 0, 100, 1, 10, 0));
  Counter ca = {{"value", "lower", "upper", "page-size", NULL}};
  watch(a, &ca);
  gtk_adjustment_set_value(a, 5); gtk_adjustment_set_value(a, 5);
  report("adjustment set_value 5,5", &ca);
  gtk_adjustment_set_value(a, 500);
  printf("#scenario\tadjustment clamped read\t%g\n", gtk_adjustment_get_value(a));
  report("adjustment set_value 500 (upper 100)", &ca);
  g_object_freeze_notify(G_OBJECT(a));
  for (int i = 0; i < 10; i++) gtk_adjustment_set_value(a, i);
  g_object_thaw_notify(G_OBJECT(a));
  report("adjustment 10 set_value inside freeze/thaw", &ca);
  gtk_adjustment_configure(a, 1, 0, 50, 1, 10, 0);
  report("adjustment configure(value 1, upper 50)", &ca);
  gtk_adjustment_set_upper(a, 0.5);
  printf("#scenario\tadjustment value after upper 0.5\t%g\n", gtk_adjustment_get_value(a));
  report("adjustment set_upper 0.5 (value 1 now out of range)", &ca);

  GtkWidget *sc = gtk_scale_new_with_range(GTK_ORIENTATION_HORIZONTAL, 0, 10, 1);
  g_object_ref_sink(sc);
  Counter cs = {{"adjustment", "fill-level", NULL}}, csa = {{"value", NULL}};
  watch(sc, &cs);
  watch(gtk_range_get_adjustment(GTK_RANGE(sc)), &csa);
  gtk_range_set_value(GTK_RANGE(sc), 3);
  report("scale set_value: widget", &cs);
  report("scale set_value: its adjustment", &csa);

  GtkWidget *en = g_object_ref_sink(gtk_entry_new());
  Counter ce = {{"text", "cursor-position", "text-length", NULL}};
  watch(en, &ce);
  int pos = 0;
  gtk_editable_insert_text(GTK_EDITABLE(en), "abc", -1, &pos);
  report("entry editable insert_text abc", &ce);
  Counter ceb = {{"text", "length", NULL}};
  watch(gtk_entry_get_buffer(GTK_ENTRY(en)), &ceb);
  gtk_editable_insert_text(GTK_EDITABLE(en), "d", -1, &pos);
  report("entry insert_text d: entry", &ce);
  report("entry insert_text d: its buffer", &ceb);

  GtkWidget *nb = g_object_ref_sink(gtk_notebook_new());
  for (int i = 0; i < 3; i++) gtk_notebook_append_page(GTK_NOTEBOOK(nb), gtk_label_new("p"), NULL);
  Counter cn = {{"page", NULL}};
  watch(nb, &cn);
  gtk_notebook_set_current_page(GTK_NOTEBOOK(nb), 2);
  report("notebook set_current_page 2", &cn);
  gtk_notebook_remove_page(GTK_NOTEBOOK(nb), 2);
  printf("#scenario\tnotebook page after removing current\t%d\n", gtk_notebook_get_current_page(GTK_NOTEBOOK(nb)));
  report("notebook remove current page", &cn);

  GtkWidget *lb = g_object_ref_sink(gtk_label_new(NULL));
  Counter cl = {{"label", "use-markup", NULL}};
  watch(lb, &cl);
  gtk_label_set_markup(GTK_LABEL(lb), "<b>x</b>");
  report("label set_markup", &cl);

  GtkWidget *tb = g_object_ref_sink(gtk_toggle_button_new());
  Counter ct = {{"active", NULL}};
  watch(tb, &ct);
  gtk_toggle_button_set_active(GTK_TOGGLE_BUTTON(tb), TRUE);
  gtk_toggle_button_set_active(GTK_TOGGLE_BUTTON(tb), TRUE);
  report("togglebutton set_active true,true", &ct);

  /* Window: props the window manager changes (no set from us) */
  GtkWidget *w = gtk_window_new();
  Counter cw = {{"default-width", "default-height", "is-active", "maximized", "fullscreened", NULL}};
  watch(w, &cw);
  gtk_window_present(GTK_WINDOW(w));
  for (int i = 0; i < 50; i++) { drain(); g_usleep(2000); }
  report("window present (WM-driven props)", &cw);
  gtk_window_maximize(GTK_WINDOW(w));
  for (int i = 0; i < 100; i++) { drain(); g_usleep(2000); }
  report("window maximize (async, WM)", &cw);
  gtk_window_destroy(GTK_WINDOW(w));
}

int main(void) {
  setvbuf(stdout, NULL, _IOLBF, 0);
  gtk_init();
  gtk_test_register_all_types();
  log_names = g_ptr_array_new();
  printf("#gtk\t%u.%u.%u\n", gtk_get_major_version(), gtk_get_minor_version(), gtk_get_micro_version());
  walk(G_TYPE_OBJECT);
  scenarios();
  return 0;
}
