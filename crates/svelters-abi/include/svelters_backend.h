/* svelters backend ABI, version 2 (2.1 additive tail members). Source of truth: plans/architecture/backend-abi.md */
#ifndef SVELTERS_BACKEND_H
#define SVELTERS_BACKEND_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define SVL_ABI_VERSION 2u

typedef uint64_t svl_handle; /* 0 = null; issued by the backend on create */
typedef uint32_t svl_id;     /* resolved kind/prop/signal/method/slot id; 0 = unknown */

typedef enum svl_tag {
  SVL_NULL = 0,
  SVL_BOOL = 1,
  SVL_I64 = 2,
  SVL_F64 = 3,
  SVL_STR = 4,
  SVL_HANDLE = 5,
  SVL_LIST = 6
} svl_tag;

typedef struct svl_str {
  const char *ptr; /* UTF-8, not NUL-terminated, borrowed for the call */
  size_t len;
} svl_str;

struct svl_value;
typedef struct svl_list {
  const struct svl_value *ptr; /* borrowed for the call */
  size_t len;
} svl_list;

typedef struct svl_value {
  uint32_t tag; /* svl_tag */
  union {
    bool b;
    int64_t i;
    double f;
    svl_handle h;
    svl_str s;
    svl_list l;
  } u;
} svl_value;

typedef enum svl_name_kind {
  SVL_KIND = 0,
  SVL_PROP = 1,
  SVL_SIGNAL = 2,
  SVL_METHOD = 3,
  SVL_SLOT = 4
} svl_name_kind;

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

struct svl_host;

typedef struct svl_host {
  void *self;
  void (*post_event)(void *self, svl_handle h, svl_id signal, const svl_value *args, size_t n);
  void (*checkpoint)(void *self);
  int (*call_sync)(void *self, svl_handle h, svl_id signal, const svl_value *args, size_t n, svl_value *out);
  void (*on_destroyed)(void *self, svl_handle h, uint32_t reason /* svl_gone */);
  /* ABI v2.1: a frame requested with backend->request_frame is due. Main thread, from the
     toolkit's frame clock. The host runs requestAnimationFrame callbacks. */
  void (*frame)(void *self);
} svl_host;

/* call_sync return codes (ABI v2.1 semantics). `out` holds the handler result when 0:
   null = no decision (use the toolkit default), true/false = stop/continue for bool_stop signals. */
#define SVL_SYNC_OK 0
#define SVL_SYNC_FAILED (-1) /* nothing ran or the host failed: use the default */
#define SVL_SYNC_BUSY (-2)   /* JS is running: the host queued the event for async delivery */

typedef struct svl_backend {
  uint32_t abi_version; /* SVL_ABI_VERSION */
  void *self;

  /* names -> ids; `kind` scopes prop/signal/method/slot lookups (0 for SVL_KIND) */
  svl_id (*resolve)(void *self, uint32_t what /* svl_name_kind */, svl_id kind, const char *name, size_t len);

  svl_handle (*create)(void *self, svl_id kind);
  int (*insert)(void *self, svl_handle parent, svl_handle child, svl_handle before /* 0 = end */, svl_id slot /* 0 = default */);
  int (*remove)(void *self, svl_handle parent, svl_handle child);
  void (*destroy)(void *self, svl_handle h);

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

  /* ABI v2.1: ask for one host->frame callback on the toolkit's next frame (vsync). Idempotent
     while a request is pending. Main thread only. */
  void (*request_frame)(void *self);
} svl_backend;

/* Values written to `out` by a backend stay valid until the backend's next call. */

#ifdef __cplusplus
}
#endif
#endif
