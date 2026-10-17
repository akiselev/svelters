/* svelters backend ABI, version 3 (draft; additive tail members: 2.1 request_frame, frame; 2.2 set_child_attr, get_child_attr, listen_ex). Source of truth: plans/architecture/backend-abi.md
 *
 * v3 = the value model from docs/notes/js-native-serdes.md section 5.1 on top of v2 (timers, handle
 * lifetime). */
#ifndef SVELTERS_BACKEND_H
#define SVELTERS_BACKEND_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define SVL_ABI_VERSION 3u

typedef uint64_t svl_handle; /* 0 = null; issued by the backend on create. Objects and boxed values share the space. */
typedef uint32_t svl_id;     /* resolved kind/prop/signal/method/slot/type id; 0 = unknown */

/* ---- values ---------------------------------------------------------------------------------- */

typedef enum svl_tag {
  /* v1 */
  SVL_NULL = 0,
  SVL_BOOL = 1,
  SVL_I64 = 2,
  SVL_F64 = 3,
  SVL_STR = 4,
  SVL_HANDLE = 5,
  SVL_LIST = 6,
  /* v3 */
  SVL_ENUM = 7,     /* i: native value;                  type: enum type id */
  SVL_FLAGS = 8,    /* u: bit mask;                      type: flags type id */
  SVL_RECORD = 9,   /* l: fields in descriptor order;    type: record type id (well-known or backend) */
  SVL_BYTES = 10,   /* bytes: {ptr,len} borrowed for the call */
  SVL_BOXED = 11,   /* h: handle of a boxed/refcounted/value-class instance; type: boxed type id */
  SVL_CALLBACK = 12, /* u: host callback id (generation in the high 16 bits); type: signature id */
  SVL_MAP = 13,     /* l: 2n values, alternating key (STR) and value; variants only */
  SVL_ERROR = 14    /* out only. l: [domain STR, code ENUM|I64, message STR] */
} svl_tag;

enum { /* svl_value.flags */
  SVL_F_UTF16 = 1u << 0,    /* STR: ptr is const uint16_t*, len in UTF-16 code units (else UTF-8 bytes) */
  SVL_F_NUL_TERM = 1u << 1, /* STR: a terminating NUL follows ptr[len] */
  SVL_F_UNSIGNED = 1u << 2, /* I64: read u.u */
  SVL_F_OWNED_BY_CALLEE = 1u << 3 /* reserved: transfer instead of borrow (not used in v3) */
};

typedef struct svl_str {
  const void *ptr; /* UTF-8 (default) or UTF-16 (SVL_F_UTF16), borrowed for the call */
  size_t len;      /* code units */
} svl_str;

struct svl_value;
typedef struct svl_list {
  const struct svl_value *ptr; /* borrowed for the call */
  size_t len;
} svl_list;

typedef struct svl_bytes {
  const uint8_t *ptr; /* borrowed for the call */
  size_t len;
} svl_bytes;

typedef struct svl_value {
  uint16_t tag;   /* svl_tag */
  uint16_t flags; /* SVL_F_* */
  svl_id type;    /* 0 unless ENUM/FLAGS/RECORD/BOXED/CALLBACK, or an expected kind on HANDLE */
  union {
    bool b;
    int64_t i;
    uint64_t u;
    double f;
    svl_handle h;
    svl_str s;
    svl_list l;
    svl_bytes bytes;
  } u;
} svl_value; /* 24 bytes on LP64 */

/* Well-known neutral value types; toolkit types get backend-assigned ids >= 0x10000. */
enum {
  SVL_T_RECT = 1,    /* {x, y, width, height} */
  SVL_T_SIZE = 2,    /* {width, height} */
  SVL_T_POINT = 3,   /* {x, y} */
  SVL_T_INSETS = 4,  /* {top, right, bottom, left} */
  SVL_T_COLOR = 5,   /* {red, green, blue, alpha}, 0..1; also accepted as a CSS color string */
  SVL_T_DATE = 6,    /* {year, month, day}, month 1-based */
  SVL_T_TIME = 7,    /* {hour, minute, second, ms} */
  SVL_T_DATETIME = 8, /* F64 epoch ms */
  SVL_T_VARIANT = 9,
  SVL_T_TYPED_VARIANT = 10, /* RECORD {signature: string, value}: a variant with an exact GVariant type signature */
  SVL_T_FIRST_BACKEND = 0x10000
};

/* ---- descriptors: static, generated, owned by the backend for its lifetime --------------------- */

enum { /* svl_slot_desc.flags */
  SVL_D_NULLABLE = 1,
  SVL_D_OPTIONAL = 2,
  SVL_D_OUT = 4,
  SVL_D_INOUT = 8,
  SVL_D_UNSIGNED = 16,
  SVL_D_INTEGRAL = 32,
  SVL_D_BIGINT = 64,
  SVL_D_BORROWED_ARG = 128,
  SVL_D_STRING_FORM = 256, /* RECORD/BOXED slot that also accepts a string in the type's canonical form */
  SVL_D_READONLY = 512     /* property slots: no setter */
};
enum { /* svl_member_desc.flags */
  SVL_M_THROWS = 1,
  SVL_M_ASYNC = 2,
  SVL_M_SYNC_SAFE = 4,
  SVL_M_BLOCKING = 8
};

typedef struct svl_slot_desc {
  const char *name;  /* JS-facing name (param, field); NULL for an unnamed slot */
  uint16_t tag;      /* expected wire tag */
  uint16_t flags;    /* SVL_D_* */
  svl_id type;       /* type id for ENUM/FLAGS/RECORD/BOXED/CALLBACK, LIST element, HANDLE kind */
  double min, max;   /* numeric range, NaN when unbounded */
} svl_slot_desc;

/* A prop: params[0] is the written value (n_params == 0 when read-only), ret is the read value.
 * A method: its parameters and return. ret.tag == SVL_NULL with name == NULL means void. */
typedef struct svl_member_desc {
  const char *name;
  const svl_slot_desc *params;
  uint32_t n_params;
  svl_slot_desc ret;
  uint32_t flags; /* SVL_M_* */
} svl_member_desc;

typedef struct svl_enum_member {
  const char *name;
  int64_t value;
} svl_enum_member;

typedef struct svl_type_desc {
  const char *name;  /* "Gtk.Orientation", "Rect", "Pango.AttrList" */
  uint16_t kind;     /* SVL_ENUM | SVL_FLAGS | SVL_RECORD | SVL_BOXED | SVL_CALLBACK | SVL_LIST */
  uint16_t scope;    /* CALLBACK: SVL_CB_* lifetime scope of the callback (0 for other kinds) */
  const svl_enum_member *members;
  uint32_t n_members; /* ENUM/FLAGS */
  const svl_slot_desc *fields;
  uint32_t n_fields; /* RECORD fields / CALLBACK params / LIST element (fields[0]) */
  svl_slot_desc ret; /* CALLBACK return */
} svl_type_desc;

/* ---- names ------------------------------------------------------------------------------------- */

typedef enum svl_name_kind {
  SVL_KIND = 0,
  SVL_PROP = 1,
  SVL_SIGNAL = 2,
  SVL_METHOD = 3,
  SVL_SLOT = 4,
  SVL_ATTR = 5 /* ABI v2.2: child attribute, scoped by the parent's kind */
} svl_name_kind;

/* ABI v2.2: listen_ex flags, and where they ride back in the signal id of post_event/call_sync. */
enum { SVL_LISTEN_AFTER = 1, SVL_LISTEN_CAPTURE = 2 };

/* ABI v2.3 `style` ops. */
enum {
  SVL_STYLE_TARGET = 0,   /* h = 0, v = NULL: returns the dialect below */
  SVL_STYLE_CLASSES = 1,  /* widget h: space separated user classes */
  SVL_STYLE_NAME = 2,     /* widget h: widget name / object name (the `id` attribute) */
  SVL_STYLE_INLINE = 3,   /* widget h: declaration lines in the backend's dialect */
  SVL_STYLE_OPACITY = 4,  /* widget h: "0".."1" through the widget API */
  SVL_STYLE_SHEET = 5,    /* h = 0: "id\x01text" component sheet; "@tokens" is the token layer */
  SVL_STYLE_SCHEME = 6    /* h = 0: "light"|"dark" then "\x01role=#rrggbb" palette pairs */
};
enum { SVL_STYLE_TARGET_NONE = 0, SVL_STYLE_TARGET_CSS = 1, SVL_STYLE_TARGET_QSS = 2 };
#define SVL_SIGNAL_FLAGS_SHIFT 28
#define SVL_SIGNAL_ID_MASK 0x0FFFFFFFu

/* Lifetime scope of a CALLBACK argument, from the callable's GIR annotation. */
enum {
  SVL_CB_CALL = 1,     /* valid for the call only: the host releases it when the call returns */
  SVL_CB_ASYNC = 2,    /* released after its first invocation (async completion) */
  SVL_CB_NOTIFIED = 3, /* released when the backend calls host->release_callback (GDestroyNotify, owner gone) */
  SVL_CB_FOREVER = 4   /* never released: refused by the host */
};

enum { SVL_CAP_UTF16_IN = 1, SVL_CAP_UTF16_OUT = 2, SVL_CAP_BYTES_RETAIN = 4 };

/* Lifetime class of a handle; see plans/architecture/backend-abi.md "ABI v2". */
typedef enum svl_lifetime {
  SVL_LIFE_OWNED = 1,
  SVL_LIFE_TOPLEVEL = 2,
  SVL_LIFE_BORROWED = 3,
  SVL_LIFE_VALUE = 4
} svl_lifetime;

/* Why a handle died; the `reason` of host->on_destroyed. */
typedef enum svl_gone {
  SVL_GONE_UNKNOWN = 0,
  SVL_GONE_CLOSED = 1,
  SVL_GONE_PARENT = 2,
  SVL_GONE_OWNER = 3,
  SVL_GONE_REPLACED = 4,
  SVL_GONE_SHUTDOWN = 5
} svl_gone;

typedef struct svl_handle_info {
  svl_id kind;
  uint32_t lifetime; /* svl_lifetime */
  uint32_t flags;
} svl_handle_info;

/* ---- host callbacks ----------------------------------------------------------------------------- */

struct svl_host;

typedef struct svl_host {
  void *self;
  void (*post_event)(void *self, svl_handle h, svl_id signal, const svl_value *args, size_t n);
  void (*checkpoint)(void *self);
  int (*call_sync)(void *self, svl_handle h, svl_id signal, const svl_value *args, size_t n, svl_value *out);
  void (*on_destroyed)(void *self, svl_handle h, uint32_t reason /* svl_gone */);
  /* v3: run the JS function behind a CALLBACK value. A call after release_callback (or after the owner
   * died) is a clean error (-1), never a crash. Returns the callback's result in `out`; on failure `out`
   * holds an SVL_ERROR value (domain "svelters") that says why. */
  int (*invoke_callback)(void *self, uint64_t callback, const svl_value *args, size_t n, svl_value *out);
  /* v3: the native side dropped a callback (GDestroyNotify, scope end, async done). Idempotent. */
  void (*release_callback)(void *self, uint64_t callback);
  /* ABI v2.1: a frame requested with backend->request_frame is due. Main thread, from the
     toolkit's frame clock. The host runs requestAnimationFrame callbacks. */
  void (*frame)(void *self);
} svl_host;

/* call_sync return codes (ABI v2.1 semantics). `out` holds the handler result when 0:
   null = no decision (use the toolkit default), true/false = stop/continue for bool_stop signals. */
#define SVL_SYNC_OK 0
#define SVL_SYNC_FAILED (-1) /* nothing ran or the host failed: use the default */
#define SVL_SYNC_BUSY (-2)   /* JS is running: the host queued the event for async delivery */


/* ---- backend ------------------------------------------------------------------------------------ */

typedef struct svl_backend {
  uint32_t abi_version; /* SVL_ABI_VERSION */
  void *self;

  /* names -> ids; `kind` scopes prop/signal/method/slot lookups (0 for SVL_KIND) */
  svl_id (*resolve)(void *self, uint32_t what /* svl_name_kind */, svl_id kind, const char *name, size_t len);

  svl_handle (*create)(void *self, svl_id kind);
  int (*insert)(void *self, svl_handle parent, svl_handle child, svl_handle before /* 0 = end */, svl_id slot /* 0 = default */);
  int (*remove)(void *self, svl_handle parent, svl_handle child);
  void (*destroy)(void *self, svl_handle h); /* also releases BOXED handles */

  int (*set_prop)(void *self, svl_handle h, svl_id prop, const svl_value *v);
  int (*get_prop)(void *self, svl_handle h, svl_id prop, svl_value *out);
  int (*call)(void *self, svl_handle h, svl_id method, const svl_value *args, size_t n, svl_value *out);

  int (*listen)(void *self, svl_handle h, svl_id signal, bool on);
  int (*handle_info)(void *self, svl_handle h, svl_handle_info *out);

  int (*run)(void *self, const svl_host *host); /* owns the toolkit loop; returns on quit */
  void (*wake)(void *self);                     /* thread-safe: schedule a host checkpoint */
  /* (re)arm a single one-shot wakeup `ms` from now, replacing any pending one; ms < 0 cancels.
     On expiry the backend calls host->checkpoint. Main thread only. */
  void (*schedule_wake)(void *self, int64_t ms);
  void (*quit)(void *self);

  const char *(*last_error)(void *self); /* NUL-terminated, valid until the next call */

  /* v3: appended after the v2 members. */
  uint32_t caps; /* SVL_CAP_* */
  /* Descriptors may be NULL (no type information: the host converts dynamically). The returned
   * pointers stay valid for the backend's lifetime. NULL return = unknown member / type. */
  const svl_member_desc *(*describe)(void *self, uint32_t what /* svl_name_kind */, svl_id kind, svl_id member);
  const svl_type_desc *(*describe_type)(void *self, svl_id type);
  /* async: cancel the operation completing `callback` (g_cancellable_cancel); 0 if already done. */
  int (*cancel)(void *self, uint64_t callback);

  /* ABI v2.1: ask for one host->frame callback on the toolkit's next frame (vsync). Idempotent
     while a request is pending. Main thread only. */
  void (*request_frame)(void *self);

  /* ABI v2.2 (all three may be NULL). Child attributes: a per-child fact owned by the parent (grid
     cell, tab label). Valid only while `child` is inserted into `parent`; SVL_NULL resets to the
     default. `attr` comes from resolve(SVL_ATTR, parent_kind, name). */
  int (*set_child_attr)(void *self, svl_handle parent, svl_handle child, svl_id attr, const svl_value *v);
  int (*get_child_attr)(void *self, svl_handle parent, svl_handle child, svl_id attr, svl_value *out);
  /* Like `listen` with SVL_LISTEN_* flags; events of that connection carry
     `signal | flags << SVL_SIGNAL_FLAGS_SHIFT` in post_event/call_sync. */
  int (*listen_ex)(void *self, svl_handle h, svl_id signal, bool on, uint32_t flags);

  /* ABI v2.3 (X6 styling; may be NULL). One member, many ops: `op` is SVL_STYLE_*, `v` is an
     SVL_STR (or SVL_NULL to clear). Returns >= 0 on success; SVL_STYLE_TARGET returns the dialect
     (SVL_STYLE_TARGET_CSS / _QSS / _NONE). Payload formats: plans/architecture/backend-abi.md. */
  int (*style)(void *self, uint32_t op, svl_handle h, const svl_value *v);
} svl_backend;

/* Values written to `out` by a backend stay valid until the backend's next call. A backend returning an
 * error may also write an SVL_ERROR value to `out`. */

#ifdef __cplusplus
}
#endif
#endif
