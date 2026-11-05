//! Odin uses no network but one request (NFR-03): no library in it can open a connection, the code doesn't reach
//! for sockets, and the Mac's curl is called from one file only, `usage.rs`, for Anthropic's usage endpoint alone
//! (ADR 0009). A new dependency that brings an HTTP client, TLS or a network runtime, or a second address, fails it.

const NETWORK_CRATES: [&str; 12] = [
    "reqwest",
    "hyper",
    "ureq",
    "curl",
    "isahc",
    "surf",
    "attohttpc",
    "tungstenite",
    "h2",
    "rustls",
    "native-tls",
    "tokio",
];

#[test]
fn nfr_03_1_no_library_or_code_can_open_a_connection() {
    let lock = include_str!("../../../Cargo.lock");
    // Who depends on what, from the lock file's [[package]] blocks.
    let packages: Vec<(&str, Vec<&str>)> = lock
        .split("[[package]]")
        .filter_map(|b| {
            let name = b.lines().find_map(|l| l.strip_prefix("name = \""))?.trim_end_matches('"');
            let deps = b
                .split("dependencies = [")
                .nth(1)
                .map(|d| {
                    d.split(']')
                        .next()
                        .unwrap_or("")
                        .lines()
                        .filter_map(|l| l.trim().strip_prefix('"'))
                        .map(|l| l.split([' ', '"']).next().unwrap_or(""))
                        .collect()
                })
                .unwrap_or_default();
            Some((name, deps))
        })
        .collect();
    // eframe's web build (wasm) brings tokio through wasm-bindgen-futures; the Mac build has none of it.
    let web_only = |c: &str| {
        let users: Vec<&str> = packages.iter().filter(|(_, d)| d.contains(&c)).map(|(n, _)| *n).collect();
        !users.is_empty() && users.iter().all(|u| *u == "wasm-bindgen-futures")
    };
    let found: Vec<&str> = NETWORK_CRATES
        .iter()
        .copied()
        .filter(|c| packages.iter().any(|(n, _)| n == c))
        .filter(|c| !web_only(c))
        .collect();
    assert!(found.is_empty(), "networking crates in Cargo.lock: {found:?}");
    for dir in ["src", "../core/src"] {
        let mut dirs = vec![std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(dir)];
        while let Some(d) = dirs.pop() {
            for entry in std::fs::read_dir(&d).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    dirs.push(path);
                } else if path.extension().is_some_and(|e| e == "rs") {
                    let src = std::fs::read_to_string(&path).unwrap();
                    assert!(
                        !src.contains("std::net") && !src.contains("TcpStream"),
                        "{} opens a socket",
                        path.display()
                    );
                    let usage = path.file_name().is_some_and(|n| n == "usage.rs");
                    assert!(usage || !src.contains("/usr/bin/curl"), "{} calls curl", path.display());
                    if usage {
                        let urls: Vec<&str> = src.match_indices("https://").map(|(i, _)| &src[i..]).collect();
                        assert!(
                            urls.iter().all(|u| u.starts_with("https://api.anthropic.com/api/oauth/usage")),
                            "usage.rs asks only Anthropic's usage endpoint"
                        );
                    }
                }
            }
        }
    }
}
