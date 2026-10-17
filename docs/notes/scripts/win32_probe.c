/* win32_probe.c: feasibility probe for a Win32 + comctl32 v6 svelters backend.
 *
 * Exercises the pieces the backend ABI needs, headless and self-checking:
 *   - create: top-level window + STATIC label + BUTTON + EDIT child HWNDs (comctl32 v6 via
 *     an embedded activation context manifest),
 *   - layout: a trivial vertical "stack" done by us on WM_SIZE (Win32 has no layout system),
 *   - events: BN_CLICKED / EN_CHANGE through WM_COMMAND on the parent (subclass-free),
 *   - set_prop/get_prop: SetWindowTextW / GetWindowTextW,
 *   - wake(): PostMessageW(WM_APP_WAKE) from a second thread -> "checkpoint",
 *   - schedule_wake(ms): one SetTimer id re-armed / KillTimer for ms < 0,
 *   - accessibility: MSAA name of the button via AccessibleObjectFromWindow (UIA proxies
 *     wrap the same data on real Windows).
 *
 * Build (Linux, mingw-w64):
 *   x86_64-w64-mingw32-windres win32_probe.rc -O coff -o win32_probe_res.o
 *   x86_64-w64-mingw32-gcc -O1 -municode win32_probe.c win32_probe_res.o -o win32_probe.exe \
 *       -lcomctl32 -loleacc -lole32 -loleaut32 -luuid -lshlwapi
 * Run headless:
 *   WINEPREFIX=$SCRATCH/wineprefix xvfb-run -a wine win32_probe.exe   (prints to stdout)
 */
#ifndef UNICODE
#define UNICODE
#endif
#ifndef _UNICODE
#define _UNICODE
#endif
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <commctrl.h>
#include <oleacc.h>
#include <shlwapi.h>
#include <stdio.h>

/* comctl32 v6 needs an application manifest (RT_MANIFEST id 1). MSVC: the pragma below;
   mingw: compile win32_probe.rc (references win32_probe.manifest) with windres and link it. */
#ifdef _MSC_VER
#pragma comment(linker, "\"/manifestdependency:type='win32' name='Microsoft.Windows.Common-Controls' version='6.0.0.0' processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'\"")
#endif

#define WM_APP_WAKE (WM_APP + 1)
#define ID_LABEL 101
#define ID_BUTTON 102
#define ID_EDIT 103
#define TIMER_WAKE 1

static HWND g_win, g_label, g_button, g_edit;
static int g_clicks, g_checkpoints, g_timer_fired, g_edit_changes;
static DWORD g_main_tid;

static void out(const char *fmt, ...) {
  va_list ap; va_start(ap, fmt); vprintf(fmt, ap); va_end(ap); fflush(stdout);
}

/* "host checkpoint": in the real backend this calls host->checkpoint(). */
static void checkpoint(const char *why) {
  g_checkpoints++;
  out("checkpoint #%d (%s) on main thread: %s\n", g_checkpoints, why,
      GetCurrentThreadId() == g_main_tid ? "yes" : "NO");
}

/* schedule_wake(ms): single one-shot timer, re-armed each call; ms < 0 cancels. */
static void schedule_wake(long long ms) {
  if (ms < 0) { KillTimer(g_win, TIMER_WAKE); return; }
  SetTimer(g_win, TIMER_WAKE, (UINT)ms, NULL); /* same id replaces the pending one */
}

/* the "stack" layout container: we own measurement and placement. */
static void layout_stack(HWND parent) {
  RECT rc; GetClientRect(parent, &rc);
  int y = 8, w = rc.right - 16;
  HWND kids[] = {g_label, g_button, g_edit};
  int heights[] = {20, 30, 24};
  for (int i = 0; i < 3; i++) {
    SetWindowPos(kids[i], NULL, 8, y, w, heights[i], SWP_NOZORDER | SWP_NOACTIVATE);
    y += heights[i] + 6;
  }
}

static DWORD WINAPI waker_thread(LPVOID p) {
  (void)p;
  Sleep(50);
  PostMessageW(g_win, WM_APP_WAKE, 0, 0); /* backend->wake(): thread-safe */
  return 0;
}

static LRESULT CALLBACK wndproc(HWND h, UINT msg, WPARAM wp, LPARAM lp) {
  switch (msg) {
  case WM_SIZE: if (g_label) layout_stack(h); return 0;
  case WM_COMMAND:
    if (LOWORD(wp) == ID_BUTTON && HIWORD(wp) == BN_CLICKED) {
      g_clicks++;
      wchar_t buf[64]; swprintf(buf, 64, L"count: %d", g_clicks);
      SetWindowTextW(g_label, buf); /* set_prop(label, "text") */
      out("event click -> label set\n");
    } else if (LOWORD(wp) == ID_EDIT && HIWORD(wp) == EN_CHANGE) {
      g_edit_changes++;
    }
    return 0;
  case WM_APP_WAKE: checkpoint("wake from thread"); return 0;
  case WM_TIMER:
    if (wp == TIMER_WAKE) {
      KillTimer(h, TIMER_WAKE); /* one-shot semantics */
      g_timer_fired++;
      checkpoint("schedule_wake timer");
    }
    return 0;
  case WM_CLOSE: DestroyWindow(h); return 0;
  case WM_DESTROY: PostQuitMessage(0); return 0;
  }
  return DefWindowProcW(h, msg, wp, lp);
}

int wmain(void) {
  g_main_tid = GetCurrentThreadId();
  INITCOMMONCONTROLSEX icc = {sizeof icc, ICC_STANDARD_CLASSES | ICC_LISTVIEW_CLASSES};
  out("InitCommonControlsEx: %d\n", InitCommonControlsEx(&icc));
  DLLVERSIONINFO dvi = {sizeof dvi};
  HMODULE cc = GetModuleHandleW(L"comctl32.dll");
  DLLGETVERSIONPROC gv = cc ? (DLLGETVERSIONPROC)(void *)GetProcAddress(cc, "DllGetVersion") : NULL;
  if (gv && SUCCEEDED(gv(&dvi))) out("comctl32 version %lu.%lu\n", dvi.dwMajorVersion, dvi.dwMinorVersion);

  WNDCLASSW wc = {0};
  wc.lpfnWndProc = wndproc; wc.hInstance = GetModuleHandleW(NULL);
  wc.lpszClassName = L"SveltersProbe"; wc.hCursor = LoadCursor(NULL, IDC_ARROW);
  wc.hbrBackground = (HBRUSH)(COLOR_WINDOW + 1);
  RegisterClassW(&wc);
  g_win = CreateWindowExW(0, L"SveltersProbe", L"svelters win32 probe", WS_OVERLAPPEDWINDOW,
                          CW_USEDEFAULT, CW_USEDEFAULT, 320, 200, NULL, NULL, wc.hInstance, NULL);
  g_label = CreateWindowExW(0, WC_STATICW, L"count: 0", WS_CHILD | WS_VISIBLE, 0, 0, 0, 0,
                            g_win, (HMENU)ID_LABEL, wc.hInstance, NULL);
  g_button = CreateWindowExW(0, WC_BUTTONW, L"Increment", WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_PUSHBUTTON,
                             0, 0, 0, 0, g_win, (HMENU)ID_BUTTON, wc.hInstance, NULL);
  g_edit = CreateWindowExW(WS_EX_CLIENTEDGE, WC_EDITW, L"", WS_CHILD | WS_VISIBLE | WS_TABSTOP | ES_AUTOHSCROLL,
                           0, 0, 0, 0, g_win, (HMENU)ID_EDIT, wc.hInstance, NULL);
  SendMessageW(g_edit, EM_SETCUEBANNER, TRUE, (LPARAM)L"type here"); /* v6-only message */
  layout_stack(g_win);
  ShowWindow(g_win, SW_SHOW);

  /* scripted interaction (what the fixture harness would inject) */
  SendMessageW(g_button, BM_CLICK, 0, 0);
  SendMessageW(g_button, BM_CLICK, 0, 0);
  SetWindowTextW(g_edit, L"hello"); /* fires EN_CHANGE */
  wchar_t buf[64]; GetWindowTextW(g_label, buf, 64);
  out("label text after 2 clicks: %ls\n", buf);

  RECT r; GetWindowRect(g_edit, &r); MapWindowPoints(NULL, g_win, (POINT *)&r, 2);
  out("edit placed by our stack layout at y=%ld h=%ld\n", r.top, r.bottom - r.top);

  /* MSAA (UIA's legacy proxy source) sees the button's name and role */
  CoInitializeEx(NULL, COINIT_APARTMENTTHREADED);
  IAccessible *acc = NULL;
  if (SUCCEEDED(AccessibleObjectFromWindow(g_button, OBJID_CLIENT, &IID_IAccessible, (void **)&acc)) && acc) {
    VARIANT self; VariantInit(&self); self.vt = VT_I4; self.lVal = CHILDID_SELF;
    BSTR name = NULL; VARIANT role; VariantInit(&role);
    acc->lpVtbl->get_accName(acc, self, &name);
    acc->lpVtbl->get_accRole(acc, self, &role);
    out("MSAA button name=%ls role=%ld (ROLE_SYSTEM_PUSHBUTTON=%d)\n", name ? name : L"(null)",
        role.vt == VT_I4 ? role.lVal : -1, ROLE_SYSTEM_PUSHBUTTON);
    SysFreeString(name); acc->lpVtbl->Release(acc);
  } else {
    out("MSAA: AccessibleObjectFromWindow failed\n");
  }

  /* wake from another thread, then a re-armed one-shot timer: 500ms replaced by 30ms */
  CloseHandle(CreateThread(NULL, 0, waker_thread, NULL, 0, NULL));
  schedule_wake(500);
  schedule_wake(30);

  DWORD start = GetTickCount();
  MSG msg;
  while (GetTickCount() - start < 400) {
    /* the backend's run(): a plain message pump; MsgWait lets us also exit on time */
    MsgWaitForMultipleObjectsEx(0, NULL, 50, QS_ALLINPUT, MWMO_INPUTAVAILABLE);
    while (PeekMessageW(&msg, NULL, 0, 0, PM_REMOVE)) {
      if (!IsDialogMessageW(g_win, &msg)) { TranslateMessage(&msg); DispatchMessageW(&msg); }
    }
  }
  out("clicks=%d edit_changes=%d timer_fired=%d checkpoints=%d\n", g_clicks, g_edit_changes,
      g_timer_fired, g_checkpoints);
  int ok = g_clicks == 2 && g_edit_changes >= 1 && g_timer_fired == 1 && g_checkpoints == 2 &&
           wcscmp(buf, L"count: 2") == 0;
  out("RESULT %s\n", ok ? "PASS" : "FAIL");
  DestroyWindow(g_win);
  return ok ? 0 : 1;
}
