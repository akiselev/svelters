//! Pack/site consistency (plans/research/CLI.md §4.3): every page the docs site renders from the
//! real IR, and every member anchor on it (own rows and inherited links), must resolve through the
//! pack built from the same IR to the same page and anchor. Needs `node` and the IR
//! (`pnpm docs:ir`; built here with python3 when missing).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;
use svelters_cli::pack::{self, Pack, Source};
use svelters_cli::resolve::{self, MemberKind, MemberQuery};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn ensure_ir(root: &Path) -> PathBuf {
    let dir = root.join("docs/site/.generated/ir");
    if !dir.join("gtk.json").is_file() || !dir.join("qt.json").is_file() {
        let st = Command::new("python3")
            .arg(root.join("tools/docs/build_ir.py"))
            .status()
            .expect("python3 runs");
        assert!(st.success(), "tools/docs/build_ir.py failed");
    }
    dir
}

#[test]
fn every_site_page_and_anchor_resolves_in_the_pack() {
    let root = root();
    let ir_dir = ensure_ir(&root);
    let out = Command::new("node")
        .arg(root.join("docs/site/scripts/site-addresses.mjs"))
        .args(["--ir".as_ref(), ir_dir.as_os_str()])
        .output()
        .expect("node runs (the consistency test needs Node, like pnpm docs:test)");
    assert!(
        out.status.success(),
        "site-addresses.mjs failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let site: Value = serde_json::from_slice(&out.stdout).unwrap();
    let site = site.as_object().unwrap();

    let mut ir_tks: Vec<String> = std::fs::read_dir(&ir_dir)
        .unwrap()
        .flatten()
        .filter_map(|e| {
            e.path()
                .file_stem()
                .filter(|_| e.path().extension().is_some_and(|x| x == "json"))
                .map(|s| s.to_string_lossy().into_owned())
        })
        .collect();
    ir_tks.sort();
    let site_tks: Vec<String> = site.keys().cloned().collect();
    assert_eq!(
        ir_tks, site_tks,
        "the site renders a different set of toolkits than the IR has"
    );

    let mut checked = (0usize, 0usize, 0usize);
    let mut failures: Vec<String> = Vec::new();
    for (tk, pages) in site {
        let bytes = std::fs::read(ir_dir.join(format!("{tk}.json"))).unwrap();
        let pk = Pack::from_ir(&bytes, Source::Ir(tk.clone())).unwrap();
        let pages = pages.as_array().unwrap();
        assert_eq!(
            pk.len(),
            pages.len(),
            "{tk}: pack has {} entities, site {} pages",
            pk.len(),
            pages.len()
        );
        let pack_routes: BTreeSet<&str> = pk
            .header
            .entities
            .iter()
            .map(|d| d.route.as_str())
            .collect();
        let site_routes: BTreeSet<&str> =
            pages.iter().map(|p| p["route"].as_str().unwrap()).collect();
        assert_eq!(pack_routes, site_routes, "{tk}: routes differ");

        for page in pages {
            let id = page["id"].as_str().unwrap();
            let route = page["route"].as_str().unwrap();
            let Some(i) = pk.index_of(id) else {
                failures.push(format!("{tk}: {id} not in pack"));
                continue;
            };
            if pk.header.entities[i].route != route {
                failures.push(format!(
                    "{tk}: {id} routes to {} in the pack, {route} on the site",
                    pk.header.entities[i].route
                ));
            }
            // every tag and the native name resolve to this entity
            for tag in page["tags"].as_array().unwrap() {
                let tag = tag.as_str().unwrap();
                if resolve::find_entities(&pk, tag) != vec![i] {
                    failures.push(format!("{tk}: tag {tag} does not resolve to {id} alone"));
                }
            }
            checked.0 += 1;
            for (list, inherited) in [(&page["own"], false), (&page["inherited"], true)] {
                for m in list.as_array().unwrap() {
                    let kind = MemberKind::parse(m["kind"].as_str().unwrap()).unwrap();
                    let name = m["name"].as_str().unwrap();
                    let anchor = m["anchor"].as_str().unwrap();
                    let want_route = if inherited {
                        m["route"].as_str().unwrap()
                    } else {
                        route
                    };
                    let hits = resolve::find_members(
                        &pk,
                        i,
                        &MemberQuery {
                            kind: Some(kind),
                            name: name.into(),
                        },
                    );
                    let ok = hits.iter().any(|h| {
                        pk.header.entities[h.owner].route == want_route
                            && h.anchors().contains(&anchor)
                    });
                    if !ok {
                        let got: Vec<String> = hits
                            .iter()
                            .map(|h| {
                                format!("{}#{:?}", pk.header.entities[h.owner].route, h.anchors())
                            })
                            .collect();
                        failures.push(format!(
                            "{tk}: {id} {}:{name} → want {want_route}#{anchor}, got {got:?}",
                            kind.prefix()
                        ));
                    }
                    if inherited {
                        checked.2 += 1
                    } else {
                        checked.1 += 1
                    }
                }
            }
        }
        // the pack round-trips: built twice, identical bytes
        assert_eq!(
            pack::build(&bytes).unwrap(),
            pack::build(&bytes).unwrap(),
            "{tk}: pack build is not deterministic"
        );
    }
    eprintln!(
        "checked {} pages, {} own member anchors, {} inherited links",
        checked.0, checked.1, checked.2
    );
    assert!(
        failures.is_empty(),
        "{} mismatches, first 30:\n{}",
        failures.len(),
        failures
            .iter()
            .take(30)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
    assert!(
        checked.1 > 1000 && checked.2 > 1000,
        "suspiciously few anchors checked: {checked:?}"
    );
}
