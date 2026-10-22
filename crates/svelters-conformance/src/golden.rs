//! Golden files: `conformance/golden/<platform>/<toolkit>-<version>.json`, one per toolkit version,
//! written by a capture (NA1) and read by the staleness check (NA5) and the egui measurement.

use crate::pins::{conformance_dir, repo_root};
use crate::trace::{Checkpoint, State};
use serde_json::Value;
use std::path::PathBuf;

pub const SCHEMA: &str = "svelters-na-golden/v1";

#[derive(Clone, Debug)]
pub struct PinResult {
    pub id: String,
    pub pin: u32,
    pub fingerprint: String,
    /// `(after, checkpoint)`: the first is `setup`.
    pub checkpoints: Vec<(String, Checkpoint)>,
}

#[derive(Clone, Debug)]
pub struct Capture {
    pub platform: String,
    pub toolkit: String,
    pub version: String,
    /// How input reached the widgets (`xtest`, `kittest`).
    pub injector: String,
    /// Toolkit settings that decide the behaviour (double-click time, drag threshold), as JSON values.
    pub settings: Vec<(String, String)>,
    /// Observation evidence: per observed prop, how many changes the checkpoints saw and how many
    /// of them the backend announced through `prop_changed` (ABI v2.6) on the user path.
    pub observe: Vec<(String, String)>,
    pub pins: Vec<PinResult>,
}

fn obj(pairs: &[(String, String)]) -> String {
    let body: Vec<String> = pairs
        .iter()
        .map(|(k, v)| format!("{}:{v}", serde_json::to_string(k).unwrap()))
        .collect();
    format!("{{{}}}", body.join(","))
}

pub fn checkpoint_json(after: &str, cp: &Checkpoint) -> String {
    let mut parts = vec![format!(
        "\"after\":{}",
        serde_json::to_string(after).unwrap()
    )];
    for (anchor, s) in cp {
        parts.push(format!(
            "{}:{}",
            serde_json::to_string(anchor).unwrap(),
            s.json()
        ));
    }
    format!("{{{}}}", parts.join(","))
}

impl Capture {
    pub fn file_name(&self) -> String {
        format!("{}-{}.json", self.toolkit, self.version)
    }

    pub fn golden_path(&self) -> PathBuf {
        conformance_dir()
            .join("golden")
            .join(&self.platform)
            .join(self.file_name())
    }

    /// Stable text: one checkpoint per line, so a re-capture diffs by step.
    pub fn to_json(&self) -> String {
        let mut s = String::from("{\n");
        let kv = |k: &str, v: &str| format!("  \"{k}\": {},\n", serde_json::to_string(v).unwrap());
        s += &kv("schema", SCHEMA);
        s += &kv("platform", &self.platform);
        s += &kv("toolkit", &self.toolkit);
        s += &kv("version", &self.version);
        s += &kv("injector", &self.injector);
        s += &kv("offsets", "unicode-scalar");
        s += &format!("  \"settings\": {},\n", obj(&self.settings));
        s += &format!("  \"observe\": {},\n", obj(&self.observe));
        s += "  \"pins\": {\n";
        for (i, p) in self.pins.iter().enumerate() {
            s += &format!("    {}: {{\n", serde_json::to_string(&p.id).unwrap());
            s += &format!(
                "      \"pin\": {},\n      \"fingerprint\": \"{}\",\n      \"checkpoints\": [\n",
                p.pin, p.fingerprint
            );
            for (k, (after, cp)) in p.checkpoints.iter().enumerate() {
                s += &format!(
                    "        {}{}\n",
                    checkpoint_json(after, cp),
                    if k + 1 < p.checkpoints.len() { "," } else { "" }
                );
            }
            s += &format!(
                "      ]\n    }}{}\n",
                if i + 1 < self.pins.len() { "," } else { "" }
            );
        }
        s += "  }\n}\n";
        s
    }
}

/// A golden file as read back.
#[derive(Clone, Debug)]
pub struct Golden {
    pub path: PathBuf,
    pub toolkit: String,
    pub version: String,
    pub pins: Vec<PinResult>,
}

impl Golden {
    pub fn pin(&self, id: &str) -> Option<&PinResult> {
        self.pins.iter().find(|p| p.id == id)
    }
}

pub fn read_golden(path: &std::path::Path) -> Result<Golden, String> {
    let ctx = |e: String| format!("{}: {e}", path.display());
    let v: Value =
        serde_json::from_str(&std::fs::read_to_string(path).map_err(|e| ctx(e.to_string()))?)
            .map_err(|e| ctx(e.to_string()))?;
    if v["schema"] != SCHEMA {
        return Err(ctx(format!("schema is {}, expected {SCHEMA}", v["schema"])));
    }
    let mut pins = Vec::new();
    for (id, p) in v["pins"].as_object().ok_or_else(|| ctx("no pins".into()))? {
        let mut checkpoints = Vec::new();
        for c in p["checkpoints"]
            .as_array()
            .ok_or_else(|| ctx(format!("{id}: no checkpoints")))?
        {
            let after = c["after"].as_str().unwrap_or_default().to_string();
            let mut cp = Vec::new();
            for (k, s) in c.as_object().unwrap() {
                if k != "after" {
                    cp.push((k.clone(), State::from_json(s).map_err(&ctx)?));
                }
            }
            cp.sort_by(|a, b| a.0.cmp(&b.0));
            checkpoints.push((after, cp));
        }
        pins.push(PinResult {
            id: id.clone(),
            pin: p["pin"].as_u64().unwrap_or(0) as u32,
            fingerprint: p["fingerprint"].as_str().unwrap_or_default().to_string(),
            checkpoints,
        });
    }
    Ok(Golden {
        path: path.to_path_buf(),
        toolkit: v["toolkit"].as_str().unwrap_or_default().to_string(),
        version: v["version"].as_str().unwrap_or_default().to_string(),
        pins,
    })
}

/// Every golden of a platform, by file name.
pub fn goldens(platform: &str) -> Result<Vec<Golden>, String> {
    let dir = conformance_dir().join("golden").join(platform);
    let Ok(rd) = std::fs::read_dir(&dir) else {
        return Ok(Vec::new());
    };
    let mut files: Vec<PathBuf> = rd
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    files.sort();
    files.iter().map(|f| read_golden(f)).collect()
}

/// Where a run writes its capture and traces (never checked in: `target/` is ignored).
pub fn out_dir(platform: &str, name: &str) -> PathBuf {
    repo_root().join("target/na").join(platform).join(name)
}

/// The tail of every capture: writes the capture and the per-pin traces under `target/na/`, then
/// with `NA_UPDATE=1` writes the golden; otherwise compares with the checked-in golden of the same
/// toolkit version, if there is one (a capture is deterministic: it must reproduce it exactly).
pub fn finish(cap: &Capture, traces: &[(String, String)]) -> Result<(), String> {
    let dir = out_dir(&cap.platform, cap.file_name().trim_end_matches(".json"));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    for (id, t) in traces {
        std::fs::write(dir.join(format!("{id}.oplog")), t).map_err(|e| e.to_string())?;
    }
    let text = cap.to_json();
    let captured = dir.parent().unwrap().join(cap.file_name());
    std::fs::write(&captured, &text).map_err(|e| e.to_string())?;
    eprintln!(
        "NA1: captured {} ({} pins) to {}",
        cap.file_name(),
        cap.pins.len(),
        captured.display()
    );
    let golden = cap.golden_path();
    if std::env::var("NA_UPDATE").is_ok_and(|v| v == "1") {
        std::fs::create_dir_all(golden.parent().unwrap()).map_err(|e| e.to_string())?;
        std::fs::write(&golden, &text).map_err(|e| e.to_string())?;
        eprintln!("NA1: wrote {}", golden.display());
        return Ok(());
    }
    match std::fs::read_to_string(&golden) {
        Ok(old) if old == text => {
            eprintln!("NA1: reproduces {}", golden.display());
            Ok(())
        }
        Ok(old) => {
            let first = old
                .lines()
                .zip(text.lines())
                .position(|(a, b)| a != b)
                .unwrap_or(old.lines().count().min(text.lines().count()));
            Err(format!(
                "capture differs from {} at line {}:\n  golden:  {}\n  capture: {}\n(re-capture with NA_UPDATE=1 when the change is intended)",
                golden.display(),
                first + 1,
                old.lines().nth(first).unwrap_or("<eof>"),
                text.lines().nth(first).unwrap_or("<eof>")
            ))
        }
        Err(_) => {
            eprintln!(
                "NA1: no golden for {} {} (checked in: {:?}); the capture is in target/na",
                cap.toolkit,
                cap.version,
                goldens(&cap.platform)
                    .map(|g| g.iter().map(|g| g.version.clone()).collect::<Vec<_>>())
            );
            Ok(())
        }
    }
}

/// How a run compares with a golden pin: equal checkpoints and the differing ones.
#[derive(Clone, Debug, Default)]
pub struct PinGap {
    pub total: usize,
    pub equal: usize,
    /// `(index, after, ours, golden)` per differing checkpoint.
    pub diffs: Vec<(usize, String, String, String)>,
}

pub fn compare(ours: &[(String, Checkpoint)], golden: &[(String, Checkpoint)]) -> PinGap {
    let mut g = PinGap {
        total: golden.len().max(ours.len()),
        ..PinGap::default()
    };
    for i in 0..g.total {
        let (a, b) = (ours.get(i), golden.get(i));
        match (a, b) {
            (Some(a), Some(b)) if a.1 == b.1 => g.equal += 1,
            _ => {
                let show = |c: Option<&(String, Checkpoint)>| {
                    c.map(|c| checkpoint_json(&c.0, &c.1))
                        .unwrap_or_else(|| "<none>".into())
                };
                let after = b.or(a).map(|c| c.0.clone()).unwrap_or_default();
                g.diffs.push((i, after, show(a), show(b)));
            }
        }
    }
    g
}
