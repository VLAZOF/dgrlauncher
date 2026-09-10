//! Modrinth catalog client (browse-only in slice 1).
//!
//! Only the public API is used, no keys needed. Modrinth requires a
//! `User-Agent` header, otherwise it answers 403.

use reqwest::Client;
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

const API: &str = "https://api.modrinth.com/v2";
const UA: &str = concat!("dgrlauncher/", env!("CARGO_PKG_VERSION"));

fn client() -> Result<Client, String> {
    Client::builder()
        .user_agent(UA)
        .build()
        .map_err(|e| format!("HTTP client failed: {e}"))
}

/// Short mod info for the store list.
#[derive(Debug, Clone, Default)]
pub struct ModSummary {
    pub id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub icon_url: String,
    pub downloads: u64,
}

/// Full mod info for the mod page (slice 1: read-only, no install yet).
#[derive(Debug, Clone, Default)]
pub struct ModDetail {
    pub id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub icon_url: String,
    pub downloads: u64,
}

/// One published file of a mod.
#[derive(Debug, Clone, Default)]
pub struct ModVersion {
    pub id: String,
    pub version_number: String,
    pub loaders: Vec<String>,
    pub game_versions: Vec<String>,
    pub file_url: String,
    pub filename: String,
    pub dependencies: Vec<ModDependency>,
}

/// One dependency of a version. `project_id` is `None` when Modrinth
/// cannot map it (then the row is not clickable).
#[derive(Debug, Clone, Default)]
pub struct ModDependency {
    pub project_id: Option<String>,
    pub dependency_type: String,
}

/// Compatibility rule: the version must list our loader (a file tagged
/// `neoforge`+`forge` counts for NeoForge) and, when the instance MC is
/// known, our Minecraft version.
pub fn is_compatible(v: &ModVersion, loader: &str, mc: &str) -> bool {
    if loader.is_empty() || !v.loaders.iter().any(|l| l == loader) {
        return false;
    }
    if !mc.is_empty() && !v.game_versions.iter().any(|g| g == mc) {
        return false;
    }
    true
}

/// Newest compatible version (the API returns newest first).
pub fn latest_compatible<'a>(
    versions: &'a [ModVersion],
    loader: &str,
    mc: &str,
) -> Option<&'a ModVersion> {
    versions.iter().find(|v| is_compatible(v, loader, mc))
}

/// Minimal percent-encoding for query params (reqwest is built
/// without default features, so `.query()` is unavailable).
fn encode(s: &str) -> String {
    let mut o = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            o.push(b as char);
        } else {
            o.push_str(&format!("%{b:02X}"));
        }
    }
    o
}

/// Search mods. Empty query + `downloads` index = top list for landing.
pub async fn search_mods(query: &str) -> Result<Vec<ModSummary>, String> {
    let index = if query.trim().is_empty() {
        "downloads"
    } else {
        "relevance"
    };
    let url = format!(
        "{API}/search?query={}&limit=25&index={index}&facets=%5B%5B%22project_type%3Amod%22%5D%5D",
        encode(query),
    );
    let text = client()?
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Search failed: {e}"))?
        .text()
        .await
        .map_err(|e| format!("Search read failed: {e}"))?;
    let v: Value =
        serde_json::from_str(&text).map_err(|e| format!("Search parse failed: {e}"))?;
    let mut out = Vec::new();
    if let Some(hits) = v["hits"].as_array() {
        for h in hits {
            let id = h["project_id"]
                .as_str()
                .or_else(|| h["slug"].as_str())
                .unwrap_or("")
                .to_owned();
            if id.is_empty() {
                continue;
            }
            out.push(ModSummary {
                id,
                slug: h["slug"].as_str().unwrap_or("").to_owned(),
                title: h["title"].as_str().unwrap_or("?").to_owned(),
                description: h["description"].as_str().unwrap_or("").to_owned(),
                icon_url: h["icon_url"].as_str().unwrap_or("").to_owned(),
                downloads: h["downloads"].as_u64().unwrap_or(0),
            });
        }
    }
    Ok(out)
}

/// Full project info by id or slug.
pub async fn get_project(id: &str) -> Result<ModDetail, String> {
    let text = client()?
        .get(format!("{API}/project/{id}"))
        .send()
        .await
        .map_err(|e| format!("Project request failed: {e}"))?
        .text()
        .await
        .map_err(|e| format!("Project read failed: {e}"))?;
    let v: Value =
        serde_json::from_str(&text).map_err(|e| format!("Project parse failed: {e}"))?;
    if v["id"].as_str().is_none() {
        return Err(format!(
            "Mod not found: {}",
            v["description"].as_str().unwrap_or(id)
        ));
    }
    Ok(ModDetail {
        id: v["id"].as_str().unwrap_or(id).to_owned(),
        slug: v["slug"].as_str().unwrap_or("").to_owned(),
        title: v["title"].as_str().unwrap_or("?").to_owned(),
        description: v["description"].as_str().unwrap_or("").to_owned(),
        icon_url: v["icon_url"].as_str().unwrap_or("").to_owned(),
        downloads: v["downloads"].as_u64().unwrap_or(0),
    })
}

/// All published versions (newest first), unfiltered: the page marks
/// loader compatibility itself, and install (slice 2) picks from here.
pub async fn get_versions(project_id: &str) -> Result<Vec<ModVersion>, String> {
    let text = client()?
        .get(format!("{API}/project/{project_id}/version?limit=60"))
        .send()
        .await
        .map_err(|e| format!("Versions request failed: {e}"))?
        .text()
        .await
        .map_err(|e| format!("Versions read failed: {e}"))?;
    let v: Value =
        serde_json::from_str(&text).map_err(|e| format!("Versions parse failed: {e}"))?;
    let mut out = Vec::new();
    if let Some(arr) = v.as_array() {
        for e in arr {
            let loaders = e["loaders"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default();
            let game_versions = e["game_versions"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default();
            // Prefer the primary file, fall back to the first one.
            let mut file_url = String::new();
            let mut filename = String::new();
            if let Some(files) = e["files"].as_array() {
                let f = files
                    .iter()
                    .find(|f| f["primary"].as_bool().unwrap_or(false))
                    .or_else(|| files.first());
                if let Some(f) = f {
                    file_url = f["url"].as_str().unwrap_or("").to_owned();
                    filename = f["filename"].as_str().unwrap_or("").to_owned();
                }
            }
            if file_url.is_empty() {
                continue;
            }
            let mut dependencies = Vec::new();
            if let Some(deps) = e["dependencies"].as_array() {
                for d in deps {
                    let t = d["dependency_type"].as_str().unwrap_or("");
                    if t != "required" && t != "optional" {
                        continue;
                    }
                    dependencies.push(ModDependency {
                        project_id: d["project_id"].as_str().map(str::to_owned),
                        dependency_type: t.to_owned(),
                    });
                }
            }
            out.push(ModVersion {
                id: e["id"].as_str().unwrap_or("").to_owned(),
                version_number: e["version_number"].as_str().unwrap_or("?").to_owned(),
                loaders,
                game_versions,
                file_url,
                filename,
                dependencies,
            });
        }
    }
    Ok(out)
}

/// Batch project titles for dependency rows: `id -> (slug, title)`.
/// One request for all ids; unknown ids are simply absent.
pub async fn get_project_titles(ids: &[String]) -> HashMap<String, (String, String)> {
    let mut out = HashMap::new();
    if ids.is_empty() {
        return out;
    }
    let joined = ids
        .iter()
        .map(|s| format!("\"{s}\""))
        .collect::<Vec<_>>()
        .join(",");
    let url = format!("{API}/projects?ids={}", encode(&format!("[{joined}]")));
    let text = match client() {
        Ok(c) => match c.get(url).send().await {
            Ok(r) => match r.text().await {
                Ok(t) => t,
                Err(_) => return out,
            },
            Err(_) => return out,
        },
        Err(_) => return out,
    };
    if let Ok(Value::Array(arr)) = serde_json::from_str::<Value>(&text) {
        for p in arr {
            if let Some(id) = p["id"].as_str() {
                out.insert(
                    id.to_owned(),
                    (
                        p["slug"].as_str().unwrap_or("").to_owned(),
                        p["title"].as_str().unwrap_or(id).to_owned(),
                    ),
                );
            }
        }
    }
    out
}

/// Raw icon bytes. `None` on any error -> caller shows the cube fallback
/// and remembers the URL as failed (no refetch loop).
pub async fn fetch_icon(url: &str) -> Option<Vec<u8>> {
    let bytes = client()
        .ok()?
        .get(url)
        .send()
        .await
        .ok()?
        .bytes()
        .await
        .ok()?;
    if bytes.is_empty() {
        return None;
    }
    Some(bytes.to_vec())
}

/// Loader context of an installed instance for the store:
/// `Some((minecraft_version, modrinth_loader))`, `None` for vanilla
/// (the store button stays disabled there).
/// Reads `{minecraft_dir}/versions/{id}/{id}.json` when available.
pub fn instance_loader(version_id: &str) -> Option<(String, String)> {
    // Content-based (rename-proof): folder names are user-chosen.
    let loader = match crate::version_kind(version_id) {
        crate::VersionKind::Fabric => "fabric",
        crate::VersionKind::NeoForge => "neoforge",
        crate::VersionKind::Forge => "forge",
        _ => return None,
    };
    let json_path = format!(
        "{}/versions/{}/{}.json",
        crate::launcher::get_minecraft_dir(),
        version_id,
        version_id
    );
    let mc = std::fs::read_to_string(&json_path)
        .ok()
        .and_then(|c| serde_json::from_str::<Value>(&c).ok())
        .and_then(|v| v["inheritsFrom"].as_str().map(str::to_owned));
    // Without inheritsFrom there is no modded context at all.
    mc.map(|mc| (mc, loader.to_owned()))
}

// ---------------------------------------------------------------------------
// Installed mods bookkeeping (slice 2).
//
// Each instance keeps `{instance}/mods/.dgrlauncher_mods.json`:
// `{ project_id: {slug, title, icon_url, version_id, version_number, filename} }`.
// The jar presence is authoritative: entries whose file is gone are ignored.
// ---------------------------------------------------------------------------

/// One installed mod recorded in the sidecar.
#[derive(Debug, Clone)]
pub struct InstalledMod {
    pub project_id: String,
    pub slug: String,
    pub title: String,
    pub icon_url: String,
    pub version_id: String,
    pub version_number: String,
    pub filename: String,
}

/// Instance mods folder, created on install.
pub fn mods_dir(version_id: &str) -> String {
    format!("{}/mods", crate::game_instance_dir_for_version(version_id))
}

fn sidecar_path(version_id: &str) -> String {
    format!("{}/.dgrlauncher_mods.json", mods_dir(version_id))
}

fn safe_filename(name: &str) -> Option<String> {
    let name = name
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("")
        .trim()
        .to_owned();
    if name.is_empty() || name == "." || name == ".." || !name.ends_with(".jar") {
        return None;
    }
    Some(name)
}

/// `project_id -> entry` for an instance. Never fails: missing or corrupt
/// sidecar, or a jar deleted by hand, simply means "not installed".
pub fn installed_mods(version_id: &str) -> HashMap<String, InstalledMod> {
    let mut map = HashMap::new();
    let content = match std::fs::read_to_string(sidecar_path(version_id)) {
        Ok(c) => c,
        Err(_) => return map,
    };
    let v: Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return map,
    };
    let dir = mods_dir(version_id);
    if let Some(obj) = v.as_object() {
        for (pid, e) in obj {
            let filename = match e["filename"]
                .as_str()
                .and_then(|n| safe_filename(n))
            {
                Some(n) => n,
                None => continue,
            };
            if !Path::new(&format!("{dir}/{filename}")).exists() {
                continue;
            }
            map.insert(
                pid.clone(),
                InstalledMod {
                    project_id: pid.clone(),
                    slug: e["slug"].as_str().unwrap_or("").to_owned(),
                    title: e["title"].as_str().unwrap_or(pid).to_owned(),
                    icon_url: e["icon_url"].as_str().unwrap_or("").to_owned(),
                    version_id: e["version_id"].as_str().unwrap_or("").to_owned(),
                    version_number: e["version_number"].as_str().unwrap_or("?").to_owned(),
                    filename,
                },
            );
        }
    }
    map
}

fn write_sidecar(version_id: &str, map: &HashMap<String, InstalledMod>) -> Result<(), String> {
    let mut obj = serde_json::Map::new();
    for (pid, e) in map {
        obj.insert(
            pid.clone(),
            serde_json::json!({
                "slug": e.slug,
                "title": e.title,
                "icon_url": e.icon_url,
                "version_id": e.version_id,
                "version_number": e.version_number,
                "filename": e.filename,
            }),
        );
    }
    std::fs::create_dir_all(mods_dir(version_id))
        .map_err(|e| format!("Mods folder failed: {e}"))?;
    let text = serde_json::to_string_pretty(&Value::Object(obj))
        .map_err(|e| format!("Sidecar encode failed: {e}"))?;
    std::fs::write(sidecar_path(version_id), text)
        .map_err(|e| format!("Sidecar write failed: {e}"))
}

/// Download a version file into the instance `mods/` and record it.
/// Replaces the previous jar of the same project (no stale duplicates).
pub async fn install_mod(
    version_id: &str,
    project: &ModDetail,
    version: &ModVersion,
) -> Result<InstalledMod, String> {
    let filename = safe_filename(&version.filename)
        .ok_or_else(|| format!("Unsafe filename from Modrinth: {}", version.filename))?;
    let bytes = client()?
        .get(&version.file_url)
        .send()
        .await
        .map_err(|e| format!("Download failed: {e}"))?
        .bytes()
        .await
        .map_err(|e| format!("Download read failed: {e}"))?;
    if bytes.is_empty() {
        return Err(String::from("Downloaded file is empty."));
    }
    let dir = mods_dir(version_id);
    std::fs::create_dir_all(&dir).map_err(|e| format!("Mods folder failed: {e}"))?;
    let mut map = installed_mods(version_id);
    if let Some(old) = map.get(&project.id) {
        if old.filename != filename {
            let _ = std::fs::remove_file(format!("{dir}/{}", old.filename));
        }
    }
    std::fs::write(format!("{dir}/{filename}"), &bytes)
        .map_err(|e| format!("Mod write failed: {e}"))?;
    let entry = InstalledMod {
        project_id: project.id.clone(),
        slug: project.slug.clone(),
        title: project.title.clone(),
        icon_url: project.icon_url.clone(),
        version_id: version.id.clone(),
        version_number: version.version_number.clone(),
        filename,
    };
    map.insert(project.id.clone(), entry.clone());
    write_sidecar(version_id, &map)?;
    Ok(entry)
}

/// Delete the jar and drop the sidecar entry. Missing files are fine.
pub fn delete_mod(version_id: &str, project_id: &str) -> Result<(), String> {
    let mut map = installed_mods(version_id);
    if let Some(e) = map.remove(project_id) {
        let _ = std::fs::remove_file(format!("{}/{}", mods_dir(version_id), e.filename));
    }
    write_sidecar(version_id, &map)
}

/// Installed count for the Main screen info box.
pub fn count_installed(version_id: &str) -> usize {
    installed_mods(version_id).len()
}

// ---------------------------------------------------------------------------
// Hand-dropped jars (slice 5): identify by sha512 through Modrinth so they
// behave like store installs (updates included). Unmatched files stay
// visible with delete-only actions.
// ---------------------------------------------------------------------------

/// Jar files in `mods/` without a sidecar entry, sorted.
pub fn unlinked_mod_files(version_id: &str) -> Vec<String> {
    let dir = mods_dir(version_id);
    let map = installed_mods(version_id);
    let mut known = std::collections::HashSet::new();
    for e in map.values() {
        known.insert(e.filename.clone());
    }
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.ends_with(".jar") && !known.contains(&name) {
                out.push(name);
            }
        }
    }
    out.sort();
    out
}

/// Hash unknown jars and ask Modrinth which projects they are
/// (`POST /v2/version_files`), linking matches into the sidecar.
/// Returns the linked count.
pub async fn link_unlinked_mods(version_id: &str) -> Result<usize, String> {
    let instance = version_id.to_owned();
    let dir = mods_dir(version_id);
    let map = installed_mods(version_id);
    let mut known = std::collections::HashSet::new();
    for e in map.values() {
        known.insert(e.filename.clone());
    }
    let mut files: Vec<(String, String)> = Vec::new();
    let entries =
        std::fs::read_dir(&dir).map_err(|e| format!("mods read failed: {e}"))?;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.ends_with(".jar") || known.contains(&name) {
            continue;
        }
        let data =
            std::fs::read(entry.path()).map_err(|e| format!("read failed: {e}"))?;
        let mut hasher = sha2::Sha512::new();
        use sha2::Digest;
        hasher.update(&data);
        files.push((name, format!("{:x}", hasher.finalize())));
    }
    if files.is_empty() {
        return Ok(0);
    }
    let hashes: Vec<String> = files.iter().map(|(_, h)| h.clone()).collect();
    let text = client()?
        .post(format!("{API}/version_files"))
        .json(&serde_json::json!({ "hashes": hashes, "algorithm": "sha512" }))
        .send()
        .await
        .map_err(|e| format!("Identify request failed: {e}"))?
        .text()
        .await
        .map_err(|e| format!("Identify read failed: {e}"))?;
    let v: Value =
        serde_json::from_str(&text).map_err(|e| format!("Identify parse failed: {e}"))?;
    let obj = match v.as_object() {
        Some(o) => o,
        None => return Ok(0),
    };
    let mut map = installed_mods(&instance);
    let mut linked = 0;
    for (filename, hash) in &files {
        let ver = match obj.get(hash) {
            Some(v) => v,
            None => continue,
        };
        let project_id = ver["project_id"].as_str().unwrap_or("");
        if project_id.is_empty() {
            continue;
        }
        // Project meta for the sidecar (title/icon); version data itself
        // already carries ids, number and the file list.
        let (slug, title, icon_url) = match get_project(project_id).await {
            Ok(p) => (p.slug, p.title, p.icon_url),
            Err(_) => (String::new(), filename.clone(), String::new()),
        };
        map.insert(
            project_id.to_owned(),
            InstalledMod {
                project_id: project_id.to_owned(),
                slug,
                title,
                icon_url,
                version_id: ver["id"].as_str().unwrap_or("").to_owned(),
                version_number: ver["version_number"].as_str().unwrap_or("?").to_owned(),
                filename: filename.clone(),
            },
        );
        linked += 1;
    }
    if linked > 0 {
        write_sidecar(&instance, &map)?;
    }
    Ok(linked)
}
