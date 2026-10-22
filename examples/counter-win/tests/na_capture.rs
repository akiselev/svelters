//! NA1 capture on WinUI 3 (WG3w): every pin of `conformance/pins/` on two `TextBox`es and a `Button`, made by
//! `counter-win --na-fixture <title>` (one process per pin, through the svelters-winui backend's ABI), driven from
//! this process by `svelters_conformance::win::UiaDriver`: `SendInput` for keys and clicks, UI Automation for
//! text, selection, caret (`TextPattern2.GetCaretRange`) and focus.
//!
//! Writes `target/na/windows/winui-<Microsoft.UI.Xaml.dll version>.json` and the traces; compares with
//! `conformance/golden/windows/` when that version has a golden; `NA_UPDATE=1` rewrites it. `NA_PINS=a,b` runs
//! only those pin ids. Needs the Windows App Runtime (docs/notes/windows-setup.md) and an unlocked interactive
//! session; do not touch the mouse while it runs.

#![cfg(windows)]

use std::io::BufRead;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
use svelters_conformance::golden::{Capture, finish, out_dir};
use svelters_conformance::pins::{Keys, load_pins};
use svelters_conformance::run_pin;
use svelters_conformance::win::{UiaDriver, WinFixture, WinResult, file_version, find_window, module_path, os_build};
use windows::Win32::Foundation::HWND;

#[derive(Default)]
struct WinUi {
    child: Option<Child>,
    /// `Microsoft.UI.Xaml.dll` of the first fixture process: path and file version.
    xaml: Option<(String, String)>,
}

impl WinFixture for WinUi {
    fn open(&mut self, title: &str, layout: Option<&str>) -> WinResult<HWND> {
        let mut args = vec!["--na-fixture", title];
        if let Some(l) = layout {
            args.extend(["--keyboard-layout", l]);
        }
        let child = Command::new(env!("CARGO_BIN_EXE_counter-win"))
            .args(args)
            .stdin(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("spawn counter-win: {e}"))?;
        let pid = child.id();
        self.child = Some(child);
        let end = Instant::now() + Duration::from_secs(30);
        let hwnd = loop {
            match find_window(title, 200) {
                Ok(h) => break h,
                Err(e) => {
                    let exited = self.child.as_mut().and_then(|c| c.try_wait().ok().flatten()).is_some();
                    if exited || Instant::now() > end {
                        return Err(format!("{e}{}", self.stderr_tail()));
                    }
                }
            }
        };
        if self.xaml.is_none() {
            let path = module_path(pid, "Microsoft.UI.Xaml.dll").unwrap_or_else(|| "not loaded".into());
            let v = file_version(&path).unwrap_or_else(|| "unknown".into());
            self.xaml = Some((path, v));
        }
        Ok(hwnd)
    }

    fn close(&mut self) -> WinResult<()> {
        let Some(mut c) = self.child.take() else { return Ok(()) };
        drop(c.stdin.take());
        let end = Instant::now() + Duration::from_secs(10);
        while Instant::now() < end {
            if c.try_wait().map_err(|e| e.to_string())?.is_some() {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let _ = c.kill();
        let _ = c.wait();
        Ok(())
    }
}

impl WinUi {
    fn stderr_tail(&mut self) -> String {
        let Some(c) = self.child.as_mut() else { return String::new() };
        if c.try_wait().ok().flatten().is_none() {
            return String::new();
        }
        let lines: Vec<String> = c.stderr.take().map(|e| std::io::BufReader::new(e).lines().map_while(Result::ok).collect()).unwrap_or_default();
        format!("\ncounter-win stderr:\n{}", lines.join("\n"))
    }
}

#[test]
fn winui_textbox_capture() {
    let keys = Keys::load("windows").unwrap();
    let mut pins = load_pins().unwrap();
    let only = std::env::var("NA_PINS").ok();
    if let Some(o) = &only {
        pins.retain(|p| o.split(',').any(|id| id == p.id));
    }
    let mut d = UiaDriver::new(WinUi::default()).unwrap();
    let mut results = Vec::new();
    let mut traces = Vec::new();
    for pin in &pins {
        let (r, t) = run_pin(&mut d, pin, &keys).unwrap_or_else(|e| {
            let _ = d.fixture.close();
            panic!("{e}")
        });
        eprintln!("NA1 winui: {} done", pin.id);
        results.push(r);
        traces.push((pin.id.clone(), t));
    }
    let (os, release) = os_build();
    let (path, version) = d.fixture.xaml.clone().unwrap_or_default();
    let package = path.split('\\').find(|s| s.starts_with("Microsoft.WindowsAppRuntime")).unwrap_or("unknown").to_string();
    let mut settings = vec![
        ("os".to_string(), format!("\"{os} ({release})\"")),
        ("xaml".to_string(), format!("\"Microsoft.UI.Xaml.dll {version}\"")),
        ("package".to_string(), format!("\"{package}\"")),
        ("fixture".to_string(), "\"counter-win --na-fixture: Window > StackPanel > TextBox, TextBox, Button\"".to_string()),
    ];
    settings.extend(d.settings.clone());
    settings.sort();
    let ev = d.evidence;
    let cap = Capture {
        platform: "windows".into(),
        toolkit: "winui".into(),
        version,
        injector: "sendinput+uia".into(),
        settings,
        observe: vec![
            ("focus".into(), format!("{{\"changes\":{},\"announced\":{}}}", ev.changes, ev.announced)),
            ("direction".into(), ev.directions_json()),
        ],
        pins: results,
    };
    if only.is_some() {
        let dir = out_dir("windows", "partial");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(cap.file_name()), cap.to_json()).unwrap();
        eprintln!("{}", cap.to_json());
        return;
    }
    finish(&cap, &traces).unwrap();
}
