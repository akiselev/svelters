/* svelters backend ABI, version 1 (draft). Source of truth: plans/architecture/backend-abi.md */
#ifndef SVELTERS_BACKEND_H
#define SVELTERS_BACKEND_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define SVL_ABI_VERSION 1u

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

struct svl_host;

typedef struct svl_host {
  void *self;
  void (*post_event)(void *self, svl_handle h, svl_id signal, const svl_value *args, size_t n);
  void (*checkpoint)(void *self);
  int (*call_sync)(void *self, svl_handle h, svl_id signal, const svl_value *args, size_t n, svl_value *out);
  void (*on_destroyed)(void *self, svl_handle h);
} svl_host;

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

  int (*run)(void *self, const svl_host *host); /* owns the toolkit loop; returns on quit */
  void (*wake)(void *self);                     /* thread-safe: schedule a host checkpoint */
  void (*quit)(void *self);

  const char *(*last_error)(void *self); /* NUL-terminated, valid until the next call */
} svl_backend;

/* Values written to `out` by a backend stay valid until the backend's next call. */

#ifdef __cplusplus
}
#endif
#endif
