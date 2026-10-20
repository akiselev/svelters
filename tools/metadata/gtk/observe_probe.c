/* GTK observability probe (plans/research/REACTIVITY.md sections 3.1 and 4.5; milestone R0).
 *
 * Run by `tools/metadata/gtk/probe.py --section observe`, which builds it, feeds it the types to probe on
 * stdin and turns its output into metadata/probe/gtk/observe/<version>.json. Seeded by
 * docs/notes/scripts/reactivity/gtk_notify_probe.c (the research sweep); this version is driven by the
 * metadata instead of the whole type tree, records every property's flags (GIR has no EXPLICIT_NOTIFY), and
 * diffs every readable property around each change, so read-only properties get evidence too.
 *
 * argv[1]: a file with one type per line, `<GIR name>\t<GType name>\t<get_type symbol>` (not stdin: the
 * virtual-display wrapper does not pass it through).
 * stdout (TSV, one record per line):
 *   #gtk     <major.minor.micro>
 *   #begin   <GType>                                    (the driver's crash bookkeeping)
 *   T  <GType> abstract|interface|object|missing
 *   I  <GType>                                          a bare instance (g_object_new, no properties) was made and
 *                                                       dropped without a crash
 *   F  <owner GType> <prop> <flags> <value GType>       every pspec a type declares itself
 *   S  <inst GType> <owner> <prop> <readback> <notify on change> <notify on same>
 *                                                       a scalar property set natively to a new value, then to the
 *                                                       value read back (readback: exact|normalized|ignored)
 *   E  <inst GType> <cause> <owner> <prop> <notify> <changed>
 *                                                       a side effect of `cause` (`set:<owner>:<prop>` or
 *                                                       `scenario:<name>`): <prop> notified and/or changed value
 *   C  <scenario> <note>                                a free-form scenario observation
 * Owners are the GType that declares the property; an interface property implemented through
 * g_object_class_override_property is reported on the interface.
 *
 * Build: cc -O1 -w -o observe_probe observe_probe.c $(pkg-config --cflags --libs gtk4) -ldl
 * The driver reruns with SVL_SKIP="<GType> ..." until no type crashes.
 */
#include <dlfcn.h>
#include <gtk/gtk.h>
#include <string.h>

static GPtrArray *notified; /* GParamSpec* of every notify since the last reset (the sweep's instance) */

/* d: the log of the object's watch (scenarios), or NULL for the sweep's global log */
static void on_notify(GObject *o, GParamSpec *p, gpointer d) { g_ptr_array_add(d ? (GPtrArray *)d : notified, p); }

static void drain(void) {
  while (g_main_context_pending(NULL)) g_main_context_iteration(NULL, FALSE);
}

static GParamSpec *canon(GParamSpec *p) {
  GParamSpec *t = g_param_spec_get_redirect_target(p);
  return t ? t : p;
}

static gboolean scalar(GType t) {
  switch (G_TYPE_FUNDAMENTAL(t)) {
  case G_TYPE_BOOLEAN: case G_TYPE_INT: case G_TYPE_UINT: case G_TYPE_INT64: case G_TYPE_UINT64:
  case G_TYPE_DOUBLE: case G_TYPE_FLOAT: case G_TYPE_STRING: case G_TYPE_ENUM: return TRUE;
  default: return FALSE;
  }
}

/* A value different from cur within the pspec's range; FALSE if none. Same rule as the generated
 * verification tests (crates/svelters-gtk/src/testing.rs, next_value). */
static gboolean next_value(GParamSpec *p, const GValue *cur, GValue *nv) {
  g_value_init(nv, p->value_type);
  if (G_IS_PARAM_SPEC_UNICHAR(p)) { /* a G_TYPE_UINT value without a range: the next code point */
    gunichar c = g_value_get_uint(cur) + 1;
    g_value_set_uint(nv, g_unichar_validate(c) ? c : 'A');
    return TRUE;
  }
  switch (G_TYPE_FUNDAMENTAL(p->value_type)) {
  case G_TYPE_BOOLEAN: g_value_set_boolean(nv, !g_value_get_boolean(cur)); return TRUE;
#define INT_CASE(T, get, set, PS)                                \
  {                                                             \
    PS *s = (PS *)p; T c = get(cur);                             \
    T v = c < s->maximum ? c + 1 : c - 1;                        \
    if (v < s->minimum || v > s->maximum || v == c) return FALSE; \
    set(nv, v); return TRUE;                                     \
  }
  case G_TYPE_INT: INT_CASE(gint, g_value_get_int, g_value_set_int, GParamSpecInt)
  case G_TYPE_UINT: INT_CASE(guint, g_value_get_uint, g_value_set_uint, GParamSpecUInt)
  case G_TYPE_INT64: INT_CASE(gint64, g_value_get_int64, g_value_set_int64, GParamSpecInt64)
  case G_TYPE_UINT64: INT_CASE(guint64, g_value_get_uint64, g_value_set_uint64, GParamSpecUInt64)
  /* The first candidate inside the range that differs from cur: c + 1 and c - 0.5 vanish next to a huge default
   * (GtkRange:fill-level is G_MAXDOUBLE), so 0, 1 and the middle of the range follow. */
  case G_TYPE_DOUBLE: {
    GParamSpecDouble *s = (GParamSpecDouble *)p; double c = g_value_get_double(cur);
    double cand[] = {c + 1.0, c - 0.5, 0.0, 1.0, s->minimum / 2 + s->maximum / 2};
    for (guint i = 0; i < G_N_ELEMENTS(cand); i++)
      if (cand[i] >= s->minimum && cand[i] <= s->maximum && cand[i] != c) { g_value_set_double(nv, cand[i]); return TRUE; }
    return FALSE;
  }
  case G_TYPE_FLOAT: {
    GParamSpecFloat *s = (GParamSpecFloat *)p; float c = g_value_get_float(cur);
    float cand[] = {c + 1.0f, c - 0.5f, 0.0f, 1.0f, s->minimum / 2 + s->maximum / 2};
    for (guint i = 0; i < G_N_ELEMENTS(cand); i++)
      if (cand[i] >= s->minimum && cand[i] <= s->maximum && cand[i] != c) { g_value_set_float(nv, cand[i]); return TRUE; }
    return FALSE;
  }
  case G_TYPE_STRING: {
    const char *c = g_value_get_string(cur);
    g_value_take_string(nv, g_strconcat(c ? c : "", "x", NULL)); return TRUE;
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

/* Instances the probe never creates (they grab the display, spawn processes or need a parent). */
static const char *SKIP_TYPES[] = {"GtkPrintUnixDialog", "GtkPageSetupUnixDialog", "GtkDragIcon", "GtkTooltip",
                                   "GtkPrintJob", "GtkMountOperation", "GtkPrintOperation", "GApplication",
                                   "GtkApplication", "GDBusConnection", "GSubprocess", "GSubprocessLauncher",
                                   "GSocketService", "GThreadedSocketService", "GNotification", NULL};
/* Properties never set (focus/visibility/tree plumbing: they change other widgets' state). */
static const char *SKIP_SET[] = {"visible", "has-focus", "is-focus", "can-target", "parent", "root", "has-default",
                                 "receives-default", NULL};
/* Properties never read in snapshots (reading them has side effects or warns on an unrealized widget). */
static const char *SKIP_READ[] = {"root", "parent", "native", "display", "surface", "renderer", "scale-factor",
                                  "focus-widget", "default-widget", "gl-context", NULL};

static gboolean in_list(const char *s, const char **l) {
  for (; *l; l++) if (!strcmp(s, *l)) return TRUE;
  return FALSE;
}

static gboolean env_skip(const char *t) {
  const char *e = g_getenv("SVL_SKIP");
  if (!e) return FALSE;
  char **v = g_strsplit(e, " ", -1);
  gboolean hit = g_strv_contains((const char *const *)v, t);
  g_strfreev(v);
  return hit;
}

/* ---- snapshots: every readable property of an instance ---- */
typedef struct {
  GParamSpec **ps;
  GValue *v;
  gboolean *ok;
  guint n;
} Snap;

static Snap snapshot(GObject *o) {
  Snap s;
  s.ps = g_object_class_list_properties(G_OBJECT_GET_CLASS(o), &s.n);
  s.v = g_new0(GValue, s.n);
  s.ok = g_new0(gboolean, s.n);
  for (guint i = 0; i < s.n; i++) {
    GParamSpec *p = s.ps[i];
    if (!(p->flags & G_PARAM_READABLE) || in_list(p->name, SKIP_READ)) continue;
    g_value_init(&s.v[i], p->value_type);
    g_object_get_property(o, p->name, &s.v[i]);
    s.ok[i] = TRUE;
  }
  return s;
}

static void snap_free(Snap *s) {
  for (guint i = 0; i < s->n; i++) if (s->ok[i]) g_value_unset(&s->v[i]);
  g_free(s->v); g_free(s->ok); g_free(s->ps);
}

static int count_in(GPtrArray *log, GParamSpec *p) {
  int n = 0;
  for (guint i = 0; i < log->len; i++) {
    GParamSpec *q = g_ptr_array_index(log, i);
    n += q == p || canon(q) == canon(p) || !strcmp(q->name, p->name);
  }
  return n;
}

static int count_notify(GParamSpec *p) { return count_in(notified, p); }

/* Whether two values of p differ, or -1 when the probe cannot tell: boxed values other than a strv compare by
 * pointer, and a getter may return a fresh copy each time. */
static int differs(GParamSpec *p, const GValue *a, const GValue *b) {
  GType f = G_TYPE_FUNDAMENTAL(p->value_type);
  if (p->value_type == G_TYPE_STRV) {
    const char *const *x = g_value_get_boxed(a), *const *y = g_value_get_boxed(b);
    if (!x || !y) return x != y;
    return !g_strv_equal(x, y);
  }
  if (f == G_TYPE_BOXED || f == G_TYPE_POINTER || f == G_TYPE_VARIANT || f == G_TYPE_PARAM) return -1;
  return g_param_values_cmp(p, a, b) != 0;
}

/* E rows for every property of `o` that changed or notified between `before` and now, except `self`.
 * `log`: the notifications seen on `o`. A value the probe cannot compare counts as unchanged. */
static void effects(GObject *o, Snap *before, const char *cause, GParamSpec *self, GPtrArray *log) {
  Snap after = snapshot(o);
  for (guint i = 0; i < after.n; i++) {
    GParamSpec *p = after.ps[i];
    if (self && canon(p) == canon(self)) continue;
    gboolean changed = before->ok[i] && after.ok[i] && differs(p, &before->v[i], &after.v[i]) == 1;
    int n = count_in(log, p);
    if (!changed && !n) continue;
    printf("E\t%s\t%s\t%s\t%s\t%d\t%d\n", G_OBJECT_TYPE_NAME(o), cause, g_type_name(canon(p)->owner_type),
           canon(p)->name, n, changed ? 1 : 0);
  }
  snap_free(&after);
}

/* ---- part A: flags and the set sweep ---- */
static GHashTable *swept; /* "owner:prop" swept on an earlier instance (abstract owners, interfaces) */

static void flags_of(GType t) {
  guint n = 0;
  GParamSpec **ps = NULL;
  gpointer klass = NULL;
  if (G_TYPE_IS_INTERFACE(t)) {
    klass = g_type_default_interface_ref(t);
    ps = g_object_interface_list_properties(klass, &n);
  } else {
    klass = g_type_class_ref(t);
    ps = g_object_class_list_properties(klass, &n);
  }
  for (guint i = 0; i < n; i++) {
    GParamSpec *p = ps[i];
    if (p->owner_type != t || G_IS_PARAM_SPEC_OVERRIDE(p)) continue;
    printf("F\t%s\t%s\t%u\t%s\n", g_type_name(t), p->name, (unsigned)p->flags, g_type_name(p->value_type));
  }
  g_free(ps);
  if (G_TYPE_IS_INTERFACE(t)) g_type_default_interface_unref(klass); else g_type_class_unref(klass);
}

static GObject *fresh(GType t) {
  GObject *o = g_object_new(t, NULL);
  if (o && g_object_is_floating(o)) g_object_ref_sink(o);
  return o;
}

static void dispose_obj(GObject *o) {
  if (GTK_IS_WINDOW(o)) gtk_window_destroy(GTK_WINDOW(o));
  g_object_unref(o);
  drain();
}

/* Every property is set on a fresh instance, as the generated verification test does: a property whose change
 * depends on an earlier set (GtkTextTag `font` after `family-set`) shows that dependency only as a side effect. */
static void sweep(GType t) {
  GObjectClass *klass = g_type_class_ref(t);
  guint n;
  GParamSpec **ps = g_object_class_list_properties(klass, &n);
  for (guint i = 0; i < n; i++) {
    GParamSpec *p = ps[i], *c = canon(p);
    GType owner = c->owner_type;
    char *key = g_strdup_printf("%s:%s", g_type_name(owner), c->name);
    /* own props always; abstract classes' and interfaces' props on the first concrete subclass */
    gboolean own = owner == t;
    gboolean shared = (G_TYPE_IS_ABSTRACT(owner) || G_TYPE_IS_INTERFACE(owner)) && !g_hash_table_contains(swept, key);
    if (!(own || shared) || !(p->flags & G_PARAM_READABLE) || !(p->flags & G_PARAM_WRITABLE) ||
        (p->flags & G_PARAM_CONSTRUCT_ONLY) || (p->flags & G_PARAM_DEPRECATED) || !scalar(p->value_type) ||
        in_list(p->name, SKIP_SET)) {
      g_free(key);
      continue;
    }
    GObject *o = fresh(t);
    if (!o) {
      g_free(key);
      break;
    }
    gulong id = g_signal_connect(o, "notify", G_CALLBACK(on_notify), NULL);
    GValue cur = G_VALUE_INIT, nv = G_VALUE_INIT, after = G_VALUE_INIT;
    g_value_init(&cur, p->value_type);
    g_object_get_property(o, p->name, &cur);
    if (!next_value(p, &cur, &nv)) {
      g_value_unset(&cur); g_value_unset(&nv); g_free(key);
      g_signal_handler_disconnect(o, id);
      dispose_obj(o);
      continue;
    }
    if (!own) g_hash_table_add(swept, g_strdup(key));
    Snap before = snapshot(o);
    g_ptr_array_set_size(notified, 0);
    g_object_set_property(o, p->name, &nv);
    drain();
    g_value_init(&after, p->value_type);
    g_object_get_property(o, p->name, &after);
    int on_change = count_notify(p);
    char *cause = g_strdup_printf("set:%s", key);
    effects(o, &before, cause, p, notified);
    snap_free(&before);
    const char *rb = g_param_values_cmp(p, &after, &nv) == 0 ? "exact"
                     : g_param_values_cmp(p, &after, &cur) == 0 ? "ignored" : "normalized";
    g_ptr_array_set_size(notified, 0);
    g_object_set_property(o, p->name, &after);
    drain();
    int on_same = count_notify(p);
    printf("S\t%s\t%s\t%s\t%s\t%d\t%d\n", g_type_name(t), g_type_name(owner), c->name, rb, on_change, on_same);
    g_free(cause); g_free(key);
    g_value_unset(&cur); g_value_unset(&nv); g_value_unset(&after);
    g_signal_handler_disconnect(o, id);
    dispose_obj(o);
  }
  g_free(ps);
  g_type_class_unref(klass);
}

static void probe_type(const char *gtype, const char *sym) {
  GType t = g_type_from_name(gtype);
  if (!t && sym && *sym) {
    GType (*get_type)(void) = (GType(*)(void))dlsym(RTLD_DEFAULT, sym);
    if (get_type) t = get_type();
  }
  if (!t) {
    printf("T\t%s\tmissing\n", gtype);
    return;
  }
  gboolean crashed = env_skip(gtype); /* crashed in an earlier run: flags only, no instance */
  if (!crashed) printf("#begin\t%s\n", gtype);
  if (G_TYPE_IS_INTERFACE(t)) {
    printf("T\t%s\tinterface\n", gtype);
    flags_of(t);
    return;
  }
  if (!g_type_is_a(t, G_TYPE_OBJECT)) {
    printf("T\t%s\tnot-an-object\n", gtype);
    return;
  }
  printf("T\t%s\t%s\n", gtype, G_TYPE_IS_ABSTRACT(t) ? "abstract" : "object");
  flags_of(t);
  if (!G_TYPE_IS_ABSTRACT(t) && !in_list(gtype, SKIP_TYPES) && !crashed) {
    /* a bare instance, made and dropped once: `I` says the generated tests can construct this type the same way */
    GObject *o = fresh(t);
    if (o) {
      dispose_obj(o);
      printf("I\t%s\n", gtype);
    }
    sweep(t);
  }
}

/* ---- part B: scenarios (changes that are not property sets) ---- */
typedef struct {
  GObject *o;
  Snap before;
  gulong id;
  GPtrArray *log;
} Watch;

/* Several objects can be watched at once (an entry and its buffer): each watch has its own log. */
static Watch watch(gpointer obj) {
  Watch w = {G_OBJECT(obj), snapshot(G_OBJECT(obj)), 0, g_ptr_array_new()};
  w.id = g_signal_connect(obj, "notify", G_CALLBACK(on_notify), w.log);
  return w;
}

static void done(Watch *w, const char *name) {
  drain();
  char *cause = g_strdup_printf("scenario:%s", name);
  effects(w->o, &w->before, cause, NULL, w->log);
  g_free(cause);
  snap_free(&w->before);
  g_signal_handler_disconnect(w->o, w->id);
  g_ptr_array_free(w->log, TRUE);
}

static void scenarios(void) {
  {
    GtkTextBuffer *b = gtk_text_buffer_new(NULL);
    Watch w = watch(b);
    gtk_text_buffer_insert_at_cursor(b, "hello", -1);
    done(&w, "TextBuffer insert_at_cursor");
    GtkTextIter s, e;
    w = watch(b);
    gtk_text_buffer_get_bounds(b, &s, &e);
    gtk_text_buffer_select_range(b, &s, &e);
    done(&w, "TextBuffer select_range all");
    w = watch(b);
    gtk_text_buffer_get_bounds(b, &s, &e);
    gtk_text_buffer_delete(b, &s, &e);
    done(&w, "TextBuffer delete all");
    w = watch(b);
    gtk_text_buffer_undo(b);
    done(&w, "TextBuffer undo");
    g_object_unref(b);
  }
  {
    GtkAdjustment *a = g_object_ref_sink(gtk_adjustment_new(0, 0, 100, 1, 10, 0));
    Watch w = watch(a);
    gtk_adjustment_set_value(a, 500);
    done(&w, "Adjustment set_value 500 with upper 100");
    w = watch(a);
    gtk_adjustment_configure(a, 1, 0, 50, 1, 10, 0);
    done(&w, "Adjustment configure");
    w = watch(a);
    gtk_adjustment_set_upper(a, 0.5);
    done(&w, "Adjustment set_upper below value");
    g_object_unref(a);
  }
  {
    GtkWidget *en = g_object_ref_sink(gtk_entry_new());
    GtkEntryBuffer *eb = gtk_entry_get_buffer(GTK_ENTRY(en));
    Watch w = watch(en), wb = watch(eb);
    int pos = 0;
    gtk_editable_insert_text(GTK_EDITABLE(en), "abc", -1, &pos);
    done(&w, "Entry insert_text");
    done(&wb, "Entry insert_text");
    g_object_unref(en);
  }
  {
    GtkWidget *sc = g_object_ref_sink(gtk_scale_new_with_range(GTK_ORIENTATION_HORIZONTAL, 0, 10, 1));
    GtkAdjustment *adj = gtk_range_get_adjustment(GTK_RANGE(sc));
    Watch w = watch(sc), wa = watch(adj);
    gtk_range_set_value(GTK_RANGE(sc), 3);
    done(&w, "Scale set_value");
    done(&wa, "Scale set_value");
    g_object_unref(sc);
  }
  {
    GtkWidget *nb = g_object_ref_sink(gtk_notebook_new());
    for (int i = 0; i < 3; i++) gtk_notebook_append_page(GTK_NOTEBOOK(nb), gtk_label_new("p"), NULL);
    gtk_notebook_set_current_page(GTK_NOTEBOOK(nb), 2);
    Watch w = watch(nb);
    gtk_notebook_remove_page(GTK_NOTEBOOK(nb), 2);
    done(&w, "Notebook remove the current page");
    g_object_unref(nb);
  }
  {
    GtkWidget *lb = g_object_ref_sink(gtk_label_new(NULL));
    Watch w = watch(lb);
    gtk_label_set_markup(GTK_LABEL(lb), "<b>x</b>");
    done(&w, "Label set_markup");
    g_object_unref(lb);
  }
  {
    GListStore *st = g_list_store_new(G_TYPE_OBJECT);
    GtkStringList *sl = gtk_string_list_new(NULL);
    Watch w = watch(st), ws = watch(sl);
    GObject *x = g_object_new(G_TYPE_OBJECT, NULL);
    g_list_store_append(st, x);
    gtk_string_list_append(sl, "a");
    done(&w, "ListStore append");
    done(&ws, "StringList append");
    GtkSingleSelection *sel = gtk_single_selection_new(G_LIST_MODEL(g_object_ref(sl)));
    gtk_string_list_append(sl, "b");
    Watch wsel = watch(sel);
    gtk_single_selection_set_selected(sel, 1);
    done(&wsel, "SingleSelection set_selected");
    g_object_unref(sel);
    g_object_unref(x);
    g_object_unref(st);
    g_object_unref(sl);
  }
  {
    /* Window: props the window manager changes (no set from us); async, so the loop runs a while */
    GtkWidget *win = gtk_window_new();
    Watch w = watch(win);
    gtk_window_present(GTK_WINDOW(win));
    for (int i = 0; i < 50; i++) { drain(); g_usleep(2000); }
    done(&w, "Window present");
    w = watch(win);
    gtk_window_maximize(GTK_WINDOW(win));
    for (int i = 0; i < 100; i++) { drain(); g_usleep(2000); }
    done(&w, "Window maximize");
    printf("C\tWindow maximize\tmaximized=%d (the window manager decides, asynchronously)\n",
           gtk_window_is_maximized(GTK_WINDOW(win)));
    gtk_window_destroy(GTK_WINDOW(win));
    drain();
  }
}

int main(int argc, char **argv) {
  setvbuf(stdout, NULL, _IOLBF, 0);
  gtk_init();
  gtk_test_register_all_types();
  notified = g_ptr_array_new();
  swept = g_hash_table_new_full(g_str_hash, g_str_equal, g_free, NULL);
  printf("#gtk\t%u.%u.%u\n", gtk_get_major_version(), gtk_get_minor_version(), gtk_get_micro_version());
  char line[1024];
  FILE *in = argc > 1 ? fopen(argv[1], "r") : NULL;
  while (in && fgets(line, sizeof line, in)) {
    g_strchomp(line);
    char **f = g_strsplit(line, "\t", 3);
    if (f[0] && f[1]) probe_type(f[1], f[2]);
    g_strfreev(f);
  }
  if (in) fclose(in);
  if (!env_skip("@scenarios")) {
    printf("#begin\t@scenarios\n");
    scenarios();
  }
  printf("#end\n");
  return 0;
}
