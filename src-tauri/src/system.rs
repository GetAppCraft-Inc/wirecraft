//! OS integration: certificate trust stores, the system proxy, ports and network addresses.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OsInfo {
    pub os: &'static str,
    /// e.g. "arch", "debian", "fedora", "opensuse"
    pub family: String,
    pub pretty_name: String,
    pub desktop: String,
    pub hostname: String,
}

pub fn os_info() -> OsInfo {
    let os = if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "linux"
    };
    let release = std::fs::read_to_string("/etc/os-release").unwrap_or_default();
    let field = |k: &str| {
        release
            .lines()
            .find_map(|l| l.strip_prefix(&format!("{k}=")))
            .map(|v| v.trim_matches('"').to_string())
            .unwrap_or_default()
    };
    let ids = format!("{} {}", field("ID"), field("ID_LIKE")).to_ascii_lowercase();
    let family = ["arch", "debian", "ubuntu", "fedora", "rhel", "suse"]
        .iter()
        .find(|f| ids.contains(*f))
        .map(|f| match *f {
            "ubuntu" => "debian",
            "rhel" => "fedora",
            "suse" => "opensuse",
            x => x,
        })
        .unwrap_or("other")
        .to_string();
    OsInfo {
        os,
        family,
        pretty_name: field("PRETTY_NAME"),
        desktop: std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default(),
        hostname: std::fs::read_to_string("/etc/hostname").map(|s| s.trim().to_string()).unwrap_or_default(),
    }
}

fn run(cmd: &str, args: &[&str]) -> Option<(bool, String)> {
    let out = Command::new(cmd).args(args).output().ok()?;
    let mut s = String::from_utf8_lossy(&out.stdout).into_owned();
    s.push_str(&String::from_utf8_lossy(&out.stderr));
    Some((out.status.success(), s))
}

fn has(cmd: &str) -> bool {
    std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).any(|d| d.join(cmd).is_file()))
        .unwrap_or(false)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrustEntry {
    pub id: &'static str,
    pub name: String,
    /// trusted | untrusted | missing | optional | unknown
    pub state: &'static str,
    pub detail: String,
}

const NSS_NICK: &str = "WireCraft Root CA";

fn home() -> PathBuf {
    dirs::home_dir().unwrap_or_default()
}

fn firefox_profiles() -> Vec<PathBuf> {
    let mut out = vec![];
    for base in [home().join(".mozilla/firefox"), home().join("snap/firefox/common/.mozilla/firefox"), home().join(".librewolf")] {
        if let Ok(rd) = std::fs::read_dir(&base) {
            for e in rd.flatten() {
                if e.path().join("cert9.db").exists() {
                    out.push(e.path());
                }
            }
        }
    }
    out
}

fn nss_has(db: &Path) -> bool {
    let d = format!("sql:{}", db.display());
    run("certutil", &["-d", &d, "-L", "-n", NSS_NICK]).map(|(ok, _)| ok).unwrap_or(false)
}

pub fn trust_status(cert_path: &str, common_name: &str) -> Vec<TrustEntry> {
    let mut v = vec![];
    if cfg!(target_os = "linux") {
        let sys = match run("trust", &["list", "--filter=ca-anchors"]) {
            Some((_, out)) if out.contains(common_name) => ("trusted", "Trusted".to_string()),
            Some(_) => ("untrusted", "Not trusted".to_string()),
            None => ("unknown", "p11-kit not found".to_string()),
        };
        v.push(TrustEntry { id: "system", name: "p11-kit (system)".into(), state: sys.0, detail: sys.1 });

        let nssdb = home().join(".pki/nssdb");
        let chrome = if !has("certutil") {
            ("unknown", "Install nss tools (certutil)".to_string())
        } else if nss_has(&nssdb) {
            ("trusted", "Trusted".to_string())
        } else {
            ("untrusted", "Not trusted".to_string())
        };
        v.push(TrustEntry { id: "nss", name: "Chrome / Chromium (NSS db)".into(), state: chrome.0, detail: chrome.1 });

        let profiles = firefox_profiles();
        let ff = if profiles.is_empty() {
            ("missing", "Not installed".to_string())
        } else if !has("certutil") {
            ("unknown", "Install nss tools (certutil)".to_string())
        } else {
            let n = profiles.iter().filter(|p| nss_has(p)).count();
            if n == profiles.len() {
                ("trusted", "Trusted".to_string())
            } else {
                ("untrusted", format!("Not trusted ({n}/{} profiles)", profiles.len()))
            }
        };
        v.push(TrustEntry { id: "firefox", name: "Firefox".into(), state: ff.0, detail: ff.1 });
    } else if cfg!(target_os = "macos") {
        let ok = run("security", &["verify-cert", "-c", cert_path]).map(|(ok, _)| ok).unwrap_or(false);
        v.push(TrustEntry {
            id: "system",
            name: "System keychain".into(),
            state: if ok { "trusted" } else { "untrusted" },
            detail: if ok { "Trusted".into() } else { "Not trusted".into() },
        });
        v.push(TrustEntry { id: "firefox", name: "Firefox".into(), state: "optional", detail: "Separate step".into() });
    } else {
        let ok = run("certutil", &["-user", "-verifystore", "Root", common_name]).map(|(ok, _)| ok).unwrap_or(false);
        v.push(TrustEntry {
            id: "system",
            name: "Windows root store".into(),
            state: if ok { "trusted" } else { "untrusted" },
            detail: if ok { "Trusted".into() } else { "Not trusted".into() },
        });
    }
    v.push(TrustEntry { id: "java", name: "Java (cacerts)".into(), state: "optional", detail: "Optional".into() });
    let node_ok = std::env::var("NODE_EXTRA_CA_CERTS").map(|p| p == cert_path || p.ends_with("wirecraft-root.pem")).unwrap_or(false);
    v.push(TrustEntry {
        id: "node",
        name: "Node.js".into(),
        state: if node_ok { "trusted" } else { "untrusted" },
        detail: if node_ok { "Trusted".into() } else { "Set NODE_EXTRA_CA_CERTS".into() },
    });
    v
}

/// The commands a user can paste to trust the CA themselves, per platform/distro.
pub fn manual_commands(dir: &str) -> serde_json::Value {
    let pem = format!("{dir}/wirecraft-root.pem");
    let crt = format!("{dir}/wirecraft-root.crt");
    serde_json::json!({
        "arch": [format!("sudo trust anchor --store {crt}")],
        "debian": [format!("sudo cp {crt} /usr/local/share/ca-certificates/wirecraft-root.crt && sudo update-ca-certificates")],
        "fedora": [format!("sudo cp {pem} /etc/pki/ca-trust/source/anchors/wirecraft-root.pem && sudo update-ca-trust")],
        "opensuse": [format!("sudo cp {pem} /etc/pki/trust/anchors/wirecraft-root.pem && sudo update-ca-certificates")],
        "macos": [format!("sudo security add-trusted-cert -d -r trustRoot -k /Library/Keychains/System.keychain {pem}")],
        "windows": [format!("certutil -user -addstore Root \"{crt}\"")],
        "firefox": [format!("certutil -A -d sql:$HOME/.mozilla/firefox/<profile> -t \"C,,\" -n \"{NSS_NICK}\" -i {pem}")],
        "nss": [format!("certutil -A -d sql:$HOME/.pki/nssdb -t \"C,,\" -n \"{NSS_NICK}\" -i {pem}")],
        "node": [format!("export NODE_EXTRA_CA_CERTS={pem}")],
    })
}

/// Add the CA to the OS trust store. Asks for admin rights through the platform's prompt.
pub fn install_system(dir: &str) -> Result<String, String> {
    let pem = format!("{dir}/wirecraft-root.pem");
    let crt = format!("{dir}/wirecraft-root.crt");
    let (cmd, args): (&str, Vec<String>) = if cfg!(target_os = "linux") {
        let fam = os_info().family;
        let script = match fam.as_str() {
            "debian" => format!("cp '{crt}' /usr/local/share/ca-certificates/wirecraft-root.crt && update-ca-certificates"),
            "fedora" => format!("cp '{pem}' /etc/pki/ca-trust/source/anchors/wirecraft-root.pem && update-ca-trust"),
            "opensuse" => format!("cp '{pem}' /etc/pki/trust/anchors/wirecraft-root.pem && update-ca-certificates"),
            _ => format!("trust anchor --store '{crt}'"),
        };
        ("pkexec", vec!["sh".into(), "-c".into(), script])
    } else if cfg!(target_os = "macos") {
        let script = format!(
            "do shell script \"security add-trusted-cert -d -r trustRoot -k /Library/Keychains/System.keychain '{pem}'\" with administrator privileges"
        );
        ("osascript", vec!["-e".into(), script])
    } else {
        ("certutil", vec!["-user".into(), "-addstore".into(), "Root".into(), crt])
    };
    let args: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    match run(cmd, &args) {
        Some((true, out)) => Ok(out),
        Some((false, out)) => Err(if out.trim().is_empty() { "Cancelled or not permitted".into() } else { out }),
        None => Err(format!("{cmd} not found")),
    }
}

/// Add the CA to the per-user NSS database (Chrome/Chromium on Linux) and every Firefox profile.
pub fn install_nss(dir: &str, firefox: bool) -> Result<String, String> {
    if !has("certutil") {
        return Err("certutil not found: install your distribution's nss tools package (e.g. nss, libnss3-tools)".into());
    }
    let pem = format!("{dir}/wirecraft-root.pem");
    let dbs: Vec<PathBuf> = if firefox {
        firefox_profiles()
    } else {
        let d = home().join(".pki/nssdb");
        if !d.join("cert9.db").exists() {
            let _ = std::fs::create_dir_all(&d);
            let ds = format!("sql:{}", d.display());
            let _ = run("certutil", &["-N", "-d", &ds, "--empty-password"]);
        }
        vec![d]
    };
    if dbs.is_empty() {
        return Err("No Firefox profiles found".into());
    }
    for db in &dbs {
        let ds = format!("sql:{}", db.display());
        let _ = run("certutil", &["-D", "-d", &ds, "-n", NSS_NICK]);
        match run("certutil", &["-A", "-d", &ds, "-t", "C,,", "-n", NSS_NICK, "-i", &pem]) {
            Some((true, _)) => {}
            Some((false, out)) => return Err(out),
            None => return Err("certutil failed to start".into()),
        }
    }
    Ok(format!("Added to {} database(s)", dbs.len()))
}

// ---------------------------------------------------------------- system proxy

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemProxy {
    pub supported: bool,
    pub backend: String,
    pub enabled: bool,
    pub host: String,
    pub port: u16,
}

fn gsettings(args: &[&str]) -> Option<String> {
    run("gsettings", args).filter(|(ok, _)| *ok).map(|(_, s)| s.trim().trim_matches('\'').to_string())
}

fn kde_config_tool() -> Option<&'static str> {
    ["kwriteconfig6", "kwriteconfig5"].into_iter().find(|c| has(c))
}

fn proxy_backend() -> &'static str {
    let de = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default().to_ascii_lowercase();
    if de.contains("kde") && kde_config_tool().is_some() {
        "kde"
    } else if has("gsettings") {
        "gnome"
    } else {
        "none"
    }
}

pub fn system_proxy_status(port: u16) -> SystemProxy {
    match proxy_backend() {
        "gnome" => {
            let mode = gsettings(&["get", "org.gnome.system.proxy", "mode"]).unwrap_or_default();
            let host = gsettings(&["get", "org.gnome.system.proxy.http", "host"]).unwrap_or_default();
            let p = gsettings(&["get", "org.gnome.system.proxy.http", "port"]).and_then(|s| s.parse().ok()).unwrap_or(0);
            SystemProxy {
                supported: true,
                backend: "GNOME (gsettings)".into(),
                enabled: mode == "manual" && p == port && (host == "127.0.0.1" || host == "localhost"),
                host,
                port: p,
            }
        }
        "kde" => {
            let rc = dirs::config_dir().unwrap_or_default().join("kioslaverc");
            let txt = std::fs::read_to_string(rc).unwrap_or_default();
            let enabled = txt.contains("ProxyType=1") && txt.contains(&format!("127.0.0.1 {port}"));
            SystemProxy { supported: true, backend: "KDE".into(), enabled, host: "127.0.0.1".into(), port }
        }
        _ => SystemProxy { supported: false, backend: "Unsupported".into(), enabled: false, host: String::new(), port: 0 },
    }
}

pub fn set_system_proxy(on: bool, port: u16, bypass: &str) -> Result<SystemProxy, String> {
    let p = port.to_string();
    match proxy_backend() {
        "gnome" => {
            let set = |schema: &str, key: &str, val: &str| {
                run("gsettings", &["set", schema, key, val]).filter(|(ok, _)| *ok).ok_or_else(|| format!("gsettings set {schema} {key} failed"))
            };
            if on {
                for s in ["org.gnome.system.proxy.http", "org.gnome.system.proxy.https"] {
                    set(s, "host", "127.0.0.1")?;
                    set(s, "port", &p)?;
                }
                let list: Vec<String> =
                    bypass.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).map(|s| format!("'{s}'")).collect();
                set("org.gnome.system.proxy", "ignore-hosts", &format!("[{}]", list.join(", ")))?;
                set("org.gnome.system.proxy", "mode", "manual")?;
            } else {
                set("org.gnome.system.proxy", "mode", "none")?;
            }
        }
        "kde" => {
            let tool = kde_config_tool().unwrap();
            let w = |key: &str, val: &str| {
                run(tool, &["--file", "kioslaverc", "--group", "Proxy Settings", "--key", key, val])
                    .filter(|(ok, _)| *ok)
                    .ok_or_else(|| format!("{tool} failed"))
            };
            if on {
                let v = format!("http://127.0.0.1 {p}");
                w("httpProxy", &v)?;
                w("httpsProxy", &v)?;
                w("NoProxyFor", bypass)?;
                w("ProxyType", "1")?;
            } else {
                w("ProxyType", "0")?;
            }
            let _ = run("dbus-send", &["--type=signal", "/KIO/Scheduler", "org.kde.KIO.Scheduler.reparseSlaveConfiguration", "string:"]);
        }
        _ => return Err("Setting the system proxy isn't supported on this desktop yet".into()),
    }
    Ok(system_proxy_status(port))
}

// ---------------------------------------------------------------- ports & addresses

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PortCheck {
    pub port: u16,
    pub free: bool,
    pub owner: Option<String>,
}

pub fn check_port(port: u16, ours: Option<u16>) -> PortCheck {
    if Some(port) == ours {
        return PortCheck { port, free: true, owner: Some("wirecraft".into()) };
    }
    let free = std::net::TcpListener::bind(("0.0.0.0", port)).is_ok();
    let owner = if free {
        None
    } else {
        run("ss", &["-ltnpH", &format!("sport = :{port}")]).and_then(|(_, out)| {
            let i = out.find("users:((\"")? + 9;
            let rest = &out[i..];
            let name = &rest[..rest.find('"')?];
            let pid = rest.find("pid=").map(|j| rest[j + 4..].chars().take_while(|c| c.is_ascii_digit()).collect::<String>());
            Some(match pid {
                Some(p) if !p.is_empty() => format!("{name} (pid {p})"),
                _ => name.to_string(),
            })
        })
    };
    PortCheck { port, free, owner }
}

/// This machine's LAN address (the one with the default route). Opening a UDP socket sends no packets.
pub fn lan_ip() -> Option<String> {
    let s = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect("192.0.2.1:80").ok()?;
    s.local_addr().ok().map(|a| a.ip().to_string())
}
