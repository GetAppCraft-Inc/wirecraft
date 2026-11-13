//! Best-effort lookup of the local process that owns a client connection (the table's Client column).
//! Linux: /proc/net/tcp{,6} gives the socket inode, then /proc/<pid>/fd finds its owner.
//! Other platforms and remote devices fall back to a name derived from the User-Agent.

use std::net::SocketAddr;

#[derive(Debug, Clone)]
pub struct ClientProc {
    pub pid: u32,
    pub name: String,
}

/// Find the process on this machine whose socket is `peer` (connected to our listener on `local`).
pub fn lookup(peer: SocketAddr, local: SocketAddr) -> Option<ClientProc> {
    if !peer.ip().is_loopback() && peer.ip() != local.ip() {
        return None;
    }
    #[cfg(target_os = "linux")]
    {
        linux::lookup(peer, local)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = local;
        None
    }
}

/// Friendly app names for common process names.
pub fn pretty(name: &str) -> String {
    let n = name.to_ascii_lowercase();
    let known = [
        ("chrome", "Chrome"),
        ("chromium", "Chromium"),
        ("firefox", "Firefox"),
        ("librewolf", "LibreWolf"),
        ("brave", "Brave"),
        ("msedge", "Edge"),
        ("vivaldi", "Vivaldi"),
        ("opera", "Opera"),
        ("electron", "Electron"),
        ("code", "VS Code"),
        ("slack", "Slack"),
        ("discord", "Discord"),
        ("spotify", "Spotify"),
        ("node", "Node.js"),
        ("python", "Python"),
        ("java", "Java"),
        ("curl", "curl"),
        ("wget", "wget"),
        ("httpie", "HTTPie"),
        ("http", "HTTPie"),
        ("go", "Go"),
    ];
    for (k, v) in known {
        if n == k || n.starts_with(&format!("{k}-")) || n.starts_with(&format!("{k}.")) || (k.len() > 4 && n.starts_with(k)) {
            return v.to_string();
        }
    }
    name.to_string()
}

/// App name from a User-Agent, for when the owning process can't be found (phones, other machines).
pub fn from_user_agent(ua: &str) -> Option<String> {
    let u = ua.to_ascii_lowercase();
    let pick = |s: &str| Some(s.to_string());
    if u.starts_with("curl/") {
        return pick("curl");
    }
    if u.starts_with("wget/") {
        return pick("wget");
    }
    if u.starts_with("httpie/") {
        return pick("HTTPie");
    }
    if u.starts_with("python-requests") || u.starts_with("python-urllib") || u.starts_with("python-httpx") || u.starts_with("aiohttp") {
        return pick("Python");
    }
    if u.starts_with("go-http-client") {
        return pick("Go");
    }
    if u.starts_with("node") || u.starts_with("undici") || u.starts_with("axios") {
        return pick("Node.js");
    }
    if u.starts_with("okhttp") {
        return pick("Android app");
    }
    if u.contains("cfnetwork") {
        return ua.split('/').next().map(|s| s.to_string());
    }
    if u.contains("edg/") {
        return pick("Edge");
    }
    if u.contains("firefox/") {
        return pick("Firefox");
    }
    if u.contains("chrome/") {
        return pick("Chrome");
    }
    if u.contains("safari/") {
        return pick("Safari");
    }
    ua.split(['/', ' ']).next().filter(|s| !s.is_empty()).map(|s| s.to_string())
}

#[cfg(target_os = "linux")]
mod linux {
    use super::ClientProc;
    use std::net::{IpAddr, SocketAddr};

    pub fn lookup(peer: SocketAddr, local: SocketAddr) -> Option<ClientProc> {
        let inode = find_inode(peer, local)?;
        let needle = format!("socket:[{inode}]");
        for entry in std::fs::read_dir("/proc").ok()?.flatten() {
            let pid: u32 = match entry.file_name().to_str().and_then(|s| s.parse().ok()) {
                Some(p) => p,
                None => continue,
            };
            let Ok(fds) = std::fs::read_dir(entry.path().join("fd")) else { continue };
            for fd in fds.flatten() {
                if let Ok(target) = std::fs::read_link(fd.path())
                    && target.as_os_str() == needle.as_str() {
                        return Some(ClientProc { pid, name: process_name(pid) });
                    }
            }
        }
        None
    }

    fn process_name(pid: u32) -> String {
        // prefer the executable's basename; `comm` is truncated to 15 chars and often a thread name
        if let Ok(exe) = std::fs::read_link(format!("/proc/{pid}/exe"))
            && let Some(n) = exe.file_name().and_then(|s| s.to_str()) {
                return n.to_string();
            }
        std::fs::read_to_string(format!("/proc/{pid}/comm")).map(|s| s.trim().to_string()).unwrap_or_else(|_| pid.to_string())
    }

    /// The client's socket appears with local = peer address and remote = our listener.
    fn find_inode(peer: SocketAddr, local: SocketAddr) -> Option<u64> {
        for file in ["/proc/net/tcp", "/proc/net/tcp6"] {
            let Ok(txt) = std::fs::read_to_string(file) else { continue };
            for line in txt.lines().skip(1) {
                let cols: Vec<&str> = line.split_whitespace().collect();
                if cols.len() < 10 {
                    continue;
                }
                let (Some(l), Some(r)) = (parse_addr(cols[1]), parse_addr(cols[2])) else { continue };
                if l.1 == peer.port() && r.1 == local.port() && same_ip(l.0, peer.ip()) {
                    return cols[9].parse().ok();
                }
            }
        }
        None
    }

    fn same_ip(a: IpAddr, b: IpAddr) -> bool {
        let norm = |ip: IpAddr| match ip {
            IpAddr::V6(v6) => v6.to_ipv4_mapped().map(IpAddr::V4).unwrap_or(IpAddr::V6(v6)),
            v4 => v4,
        };
        norm(a) == norm(b)
    }

    fn parse_addr(s: &str) -> Option<(IpAddr, u16)> {
        let (ip, port) = s.split_once(':')?;
        let port = u16::from_str_radix(port, 16).ok()?;
        let ip = if ip.len() == 8 {
            let n = u32::from_str_radix(ip, 16).ok()?;
            IpAddr::from(n.to_le_bytes())
        } else {
            let mut b = [0u8; 16];
            for i in 0..4 {
                let w = u32::from_str_radix(&ip[i * 8..i * 8 + 8], 16).ok()?;
                b[i * 4..i * 4 + 4].copy_from_slice(&w.to_le_bytes());
            }
            IpAddr::from(b)
        };
        Some((ip, port))
    }
}
