use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Java installation found on the PC.
#[derive(Clone, Debug)]
pub struct SystemJava {
    /// Full path to the `java` binary.
    pub path: String,
    /// Full version string, e.g. `25.0.1` or `1.8.0_442`.
    pub version: String,
    /// Major version, e.g. `25` or `8`.
    pub major: u64,
}

/// Scan common locations for installed Java binaries and probe their versions.
/// Locations: `PATH`, `JAVA_HOME`, `/usr/lib/jvm` (Linux), `Program Files`
/// (Windows), and the vanilla launcher runtimes folder.
pub async fn scan_system_javas() -> Vec<SystemJava> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    let exe = if std::env::consts::OS == "windows" {
        "java.exe"
    } else {
        "java"
    };
    // 1. `java` from PATH.
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_var) {
            let p = dir.join(exe);
            if p.is_file() {
                candidates.push(p);
            }
        }
    }
    // 2. JAVA_HOME.
    if let Ok(home) = std::env::var("JAVA_HOME") {
        if !home.trim().is_empty() {
            candidates.push(Path::new(&home).join("bin").join(exe));
        }
    }
    // 3. OS-specific well-known roots.
    match std::env::consts::OS {
        "linux" => {
            for root in ["/usr/lib/jvm", "/usr/lib64/jvm", "/opt"] {
                push_bin_java(&mut candidates, root, exe, 2);
            }
        }
        "windows" => {
            for var in ["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"] {
                if let Ok(base) = std::env::var(var) {
                    for vendor in [
                        "Java",
                        "Eclipse Adoptium",
                        "Eclipse Foundation",
                        "Zulu",
                        "Microsoft",
                    ] {
                        push_bin_java(&mut candidates, &format!("{base}\\{vendor}"), exe, 2);
                    }
                }
            }
        }
        _ => {}
    }
    // 4. Vanilla launcher runtimes (e.g. `<mc>/runtime/java-runtime-delta/windows-x64/...`).
    let mc_runtime = format!("{}/runtime", super::launcher::get_minecraft_dir());
    collect_runtime_javas(&mut candidates, Path::new(&mc_runtime), exe);
    // Deduplicate by canonical path.
    let mut seen = HashSet::new();
    let mut unique: Vec<PathBuf> = Vec::new();
    for c in candidates {
        if !c.is_file() {
            continue;
        }
        let key = std::fs::canonicalize(&c)
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|_| c.to_string_lossy().into_owned());
        if seen.insert(key) {
            unique.push(c);
        }
    }
    // Probe versions (sequentially; usually a handful of candidates).
    let mut found = Vec::new();
    for path in unique {
        if let Some(java) = probe_java(&path).await {
            found.push(java);
        }
    }
    found.sort_by(|a, b| b.major.cmp(&a.major).then(a.path.cmp(&b.path)));
    found
}
/// Look for `bin/java[.exe]` one or two levels below `root`.
fn push_bin_java(candidates: &mut Vec<PathBuf>, root: &str, exe: &str, depth: u8) {
    let root_path = Path::new(root);
    // `<root>/bin/java` (e.g. a JAVA_HOME-like dir passed directly).
    candidates.push(root_path.join("bin").join(exe));
    if depth == 0 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(root_path) else {
        return;
    };
    for entry in entries.flatten() {
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        // `<root>/<vendor>/bin/java`.
        candidates.push(dir.join("bin").join(exe));
        if depth > 1 {
            // `<root>/<vendor>/<version>/bin/java`
            // (e.g. `Eclipse Adoptium/jdk-25.0.1.8-hotspot/bin/java.exe`).
            if let Ok(nested) = std::fs::read_dir(&dir) {
                for n in nested.flatten() {
                    if n.path().is_dir() {
                        candidates.push(n.path().join("bin").join(exe));
                    }
                }
            }
        }
    }
}
/// Recursively collect `bin/java` under the vanilla runtime folder (max depth 6).
fn collect_runtime_javas(candidates: &mut Vec<PathBuf>, dir: &Path, exe: &str) {
    fn walk(candidates: &mut Vec<PathBuf>, dir: &Path, exe: &str, depth: u8) {
        if depth == 0 {
            return;
        }
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                if p.join("bin").join(exe).is_file() {
                    candidates.push(p.join("bin").join(exe));
                } else {
                    walk(candidates, &p, exe, depth - 1);
                }
            }
        }
    }
    walk(candidates, dir, exe, 6);
}
/// Run `<path> -version` and parse the major version.
async fn probe_java(path: &Path) -> Option<SystemJava> {
    let output = tokio::time::timeout(
        Duration::from_secs(5),
        tokio::process::Command::new(path)
            .arg("-version")
            .output(),
    )
    .await
    .ok()?
    .ok()?;
    // `java -version` prints to stderr.
    let mut combined = output.stderr;
    combined.extend_from_slice(&output.stdout);
    let text = String::from_utf8_lossy(&combined);
    let first_line = text.lines().next().unwrap_or("");
    // e.g. `openjdk version "25.0.1" 2025-09-16` or `java version "1.8.0_442"`.
    let quoted = first_line.split('"').nth(1)?;
    let major = parse_java_major(quoted)?;
    Some(SystemJava {
        path: path.to_string_lossy().into_owned(),
        version: quoted.to_owned(),
        major,
    })
}
fn parse_java_major(version: &str) -> Option<u64> {
    let mut parts = version.split(|c| c == '.' || c == '_' || c == '-');
    let first: u64 = parts.next()?.parse().ok()?;
    if first == 1 {
        // Old scheme: "1.8.0_442" -> 8.
        parts.next()?.parse().ok()
    } else {
        Some(first)
    }
}
#[cfg(test)]
mod tests {
    use super::parse_java_major;
    #[test]
    fn parses_java_versions() {
        assert_eq!(parse_java_major("25.0.1"), Some(25));
        assert_eq!(parse_java_major("21.0.12.1"), Some(21));
        assert_eq!(parse_java_major("17.0.9"), Some(17));
        assert_eq!(parse_java_major("1.8.0_442"), Some(8));
        assert_eq!(parse_java_major("11.0.22"), Some(11));
        assert_eq!(parse_java_major("garbage"), None);
    }
}
