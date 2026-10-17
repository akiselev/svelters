# 1000 QPushButtons: cost of QSS strategies. "apply" = API calls; "frame" = processEvents +
# synchronous repaint of the window (polish happens lazily on show / events).
import sys, time, json
from PySide6.QtWidgets import QApplication, QWidget, QPushButton, QGridLayout, QScrollArea
MODE = sys.argv[1]; N = int(sys.argv[2]) if len(sys.argv) > 2 else 1000
app = QApplication(['bench'])
res = {'mode': MODE, 'n': N, 'style': app.style().name()}
win = QScrollArea(); win.resize(1200, 900); inner = QWidget(); grid = QGridLayout(inner)
btns = [QPushButton(str(i)) for i in range(N)]
for i, b in enumerate(btns): grid.addWidget(b, i // 40, i % 40)
win.setWidget(inner); win.setWidgetResizable(True)
def frame():
    t0 = time.perf_counter(); app.processEvents(); win.grab(); app.processEvents()
    return time.perf_counter() - t0
def repolish(w): s = w.style(); s.unpolish(w); s.polish(w); w.update()
if MODE == 'app_sheet_preset':  # app stylesheet installed BEFORE show (the normal app case)
    app.setStyleSheet('QPushButton[class~="sv-a"] { color: white; background: #3584e4; padding: 4px 10px; }')
    for b in btns: b.setProperty('class', 'sv-a')
win.show()
res['baseline_frame'] = frame(); frame(); res['steady_frame'] = frame()
t0 = time.perf_counter()
if MODE == 'app_sheet_class':    # one app-wide sheet, class via dynamic property + repolish
    app.setStyleSheet('QPushButton[class~="sv-a"] { color: white; background: #3584e4; padding: 4px 10px; }')
    for b in btns: b.setProperty('class', 'sv-a'); repolish(b)
elif MODE == 'app_sheet_preset':
    pass
elif MODE == 'inline_setstylesheet':  # per-widget sheet (inline style)
    for i, b in enumerate(btns): b.setStyleSheet(f'color: rgb({i%256},0,0); padding: {i%7}px;')
elif MODE == 'app_sheet_n_rules':     # inline styles as 1000 objectName rules in the app sheet
    for i, b in enumerate(btns): b.setObjectName(f'sv-i{i}')
    app.setStyleSheet(''.join(f'#sv-i{i} {{ color: rgb({i%256},0,0); padding: {i%7}px; }}\n' for i in range(N)))
elif MODE == 'window_sheet_class':    # sheet on the window (scoped subtree), class property
    for b in btns: b.setProperty('class', 'sv-a')
    win.setStyleSheet('QPushButton[class~="sv-a"] { color: white; background: #3584e4; padding: 4px 10px; }')
res['apply'] = time.perf_counter() - t0; res['first_frame'] = frame()
# update one widget
t0 = time.perf_counter()
if MODE in ('app_sheet_class', 'app_sheet_preset', 'window_sheet_class'):
    btns[0].setProperty('class', 'sv-b'); repolish(btns[0])
elif MODE == 'inline_setstylesheet':
    btns[0].setStyleSheet('color: green; padding: 0px;')
elif MODE == 'app_sheet_n_rules':
    app.setStyleSheet(app.styleSheet().replace('#sv-i0 { color: rgb(0,0,0)', '#sv-i0 { color: rgb(0,200,0)'))
res['update_one_apply'] = time.perf_counter() - t0; res['update_one_frame'] = frame()
t0 = time.perf_counter()
if MODE in ('app_sheet_class', 'app_sheet_preset', 'window_sheet_class'): btns[1].setProperty('class', 'sv-b'); repolish(btns[1])
elif MODE == 'inline_setstylesheet': btns[1].setStyleSheet('color: blue; padding: 1px;')
elif MODE == 'app_sheet_n_rules': app.setStyleSheet(app.styleSheet().replace('#sv-i1 { color: rgb(1,0,0)', '#sv-i1 { color: rgb(0,0,200)'))
res['update_two_apply'] = time.perf_counter() - t0; res['update_two_frame'] = frame()
if MODE in ('app_sheet_class', 'app_sheet_preset', 'window_sheet_class'):
    t0 = time.perf_counter()
    for b in btns: b.setProperty('class', ''); repolish(b)
    res['toggle_all_apply'] = time.perf_counter() - t0; res['toggle_all_frame'] = frame()
    t0 = time.perf_counter()
    for b in btns: b.setProperty('class', 'sv-a')
    res['toggle_all_noRepolish_apply'] = time.perf_counter() - t0; res['toggle_all_noRepolish_frame'] = frame()
# HMR: replace the app-wide sheet
t0 = time.perf_counter()
app.setStyleSheet((app.styleSheet() or '') + ' QPushButton { margin: 1px; }' + ''.join(f' .hmr{i} {{ color: red; }}' for i in range(50)))
res['hmr_reload_apply'] = time.perf_counter() - t0; res['hmr_reload_frame'] = frame()
print(json.dumps({k: (round(v * 1000, 2) if isinstance(v, float) else v) for k, v in res.items()}))
