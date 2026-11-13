//! Repository tasks: `cargo xtask <task>`.
//!
//!   version                 print the product version
//!   version set <x.y.z>     set it everywhere (Cargo.toml, Cargo.lock, package.json, package-lock.json);
//!                           pre-releases use a suffix: 0.3.0-rc.1
//!   ci                      what CI runs: clippy, tests, svelte-check, frontend build

use std::path::{Path, PathBuf};
use std::process::{Command, exit};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("workspace root").to_path_buf()
}

fn read(p: &Path) -> String {
    std::fs::read_to_string(p).unwrap_or_else(|e| fail(&format!("reading {}: {e}", p.display())))
}

fn fail(msg: &str) -> ! {
    eprintln!("xtask: {msg}");
    exit(1)
}

/// The `version = "…"` line inside `[workspace.package]`.
fn current_version(cargo_toml: &str) -> Option<(usize, usize, String)> {
    let start = cargo_toml.find("[workspace.package]")?;
    let section_end = cargo_toml[start + 1..].find("\n[").map(|i| start + 1 + i).unwrap_or(cargo_toml.len());
    let section = &cargo_toml[start..section_end];
    let line = section.lines().find(|l| l.trim_start().starts_with("version"))?;
    let a = line.find('"')? + 1;
    let b = line[a..].find('"')? + a;
    let line_at = start + section.find(line)?;
    Some((line_at + a, line_at + b, line[a..b].to_string()))
}

/// SemVer `MAJOR.MINOR.PATCH` with an optional `-prerelease` (alphanumerics, dots, hyphens).
fn valid(v: &str) -> bool {
    let (core, pre) = match v.split_once('-') {
        Some((c, p)) => (c, Some(p)),
        None => (v, None),
    };
    let nums: Vec<&str> = core.split('.').collect();
    nums.len() == 3
        && nums.iter().all(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()) && (n.len() == 1 || !n.starts_with('0')))
        && pre.is_none_or(|p| !p.is_empty() && p.split('.').all(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')))
}

fn set_json_version(path: &Path, v: &str, lock: bool) {
    if !path.exists() {
        return;
    }
    let mut doc: serde_json::Value = serde_json::from_str(&read(path)).unwrap_or_else(|e| fail(&format!("{}: {e}", path.display())));
    doc["version"] = v.into();
    if lock {
        // npm keeps the root package under the empty key
        if let Some(p) = doc.get_mut("packages").and_then(|p| p.get_mut("")) {
            p["version"] = v.into();
        }
    }
    let text = serde_json::to_string_pretty(&doc).expect("json") + "\n";
    std::fs::write(path, text).unwrap_or_else(|e| fail(&format!("writing {}: {e}", path.display())));
}

fn run(cmd: &str, args: &[&str]) {
    println!("$ {cmd} {}", args.join(" "));
    let ok = Command::new(cmd).args(args).current_dir(root()).status().map(|s| s.success()).unwrap_or(false);
    if !ok {
        fail(&format!("`{cmd} {}` failed", args.join(" ")));
    }
}

fn version(args: &[String]) {
    let path = root().join("Cargo.toml");
    let toml = read(&path);
    let (a, b, cur) = current_version(&toml).unwrap_or_else(|| fail("no version in [workspace.package] of Cargo.toml"));
    match args {
        [] => println!("{cur}"),
        [set, new] if set == "set" => {
            let new = new.trim().trim_start_matches('v');
            if !valid(new) {
                fail(&format!("{new:?} is not a version like 0.3.0 or 0.3.0-rc.1"));
            }
            let mut out = toml.clone();
            out.replace_range(a..b, new);
            std::fs::write(&path, out).unwrap_or_else(|e| fail(&e.to_string()));
            set_json_version(&root().join("package.json"), new, false);
            set_json_version(&root().join("package-lock.json"), new, true);
            run("cargo", &["update", "--workspace", "--offline", "--quiet"]);
            println!("{cur} → {new}  (commit Cargo.toml, Cargo.lock, package.json, package-lock.json)");
        }
        _ => fail("usage: cargo xtask version [set <x.y.z>]"),
    }
}

fn ci() {
    run("cargo", &["clippy", "--workspace", "--all-targets", "--", "-D", "warnings"]);
    run("cargo", &["test", "--workspace"]);
    run("npx", &["svelte-check", "--tsconfig", "./tsconfig.json", "--fail-on-warnings"]);
    run("npm", &["run", "build"]);
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(|s| s.as_str()) {
        Some("version") => version(&args[1..]),
        Some("ci") => ci(),
        _ => fail("tasks: version [set <x.y.z>], ci"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions() {
        for v in ["0.2.0", "1.10.3", "0.3.0-rc.1", "2.0.0-beta.2"] {
            assert!(valid(v), "{v}");
        }
        for v in ["0.2", "01.2.3", "1.2.3-", "1.2.3-rc..1", "v1.2.3", "1.2.x"] {
            assert!(!valid(v), "{v}");
        }
        let t = "[workspace]\nmembers=[]\n\n[workspace.package]\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[x]\nversion = \"9\"\n";
        let (a, b, v) = current_version(t).unwrap();
        assert_eq!((v.as_str(), &t[a..b]), ("0.1.0", "0.1.0"));
    }
}
