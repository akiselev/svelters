#!/bin/sh
# Octane kernels on QuickJS-NG (qjs from rquickjs-sys sources), JSC LLInt-only and JSC JIT
# (WebKitGTK javascriptcoregtk-4.1). See docs/notes/apple-gui-backend.md §8.1.
# usage: jsc_vs_quickjs_bench.sh <octane-dir> <qjs> <jsc_capi_probe-binary>
#   build probe: gcc -O2 jsc_capi_probe.c -o jsc_run $(pkg-config --cflags --libs javascriptcoregtk-4.1)
#   microtask probe: ./jsc_run --microtask
set -e
O=$1; Q=$2; J=$3; T=$(mktemp -d)
cat > $T/runner.js <<'JS'
var __t0 = Date.now();
BenchmarkSuite.RunSuites({
  NotifyResult: function(n, r) { print(n + ': ' + r); },
  NotifyError: function(n, e) { print(n + ': ERROR ' + e); },
  NotifyScore: function(s) { print('Score: ' + s + '  wall_ms: ' + (Date.now() - __t0)); }
});
JS
echo 'globalThis.print = (...a) => console.log(...a);' > $T/qp.js
for k in richards deltablue raytrace splay navier-stokes; do
  cat $T/qp.js $O/base.js $O/$k.js $T/runner.js > $T/q.js
  echo "$k | qjs: $($Q --script $T/q.js | tr '\n' ' ') | jsc-nojit: $(JSC_useJIT=false $J $O/base.js $O/$k.js $T/runner.js | tr '\n' ' ') | jsc-jit: $($J $O/base.js $O/$k.js $T/runner.js | tr '\n' ' ')"
done
