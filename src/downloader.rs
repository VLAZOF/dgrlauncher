use iced::{stream, Subscription};
use iced::futures::SinkExt;
use reqwest::{self, Client};
use serde_json::Value;
use std::{
    env,
    fs::{self, File},
    hash::Hash,
    io::{BufReader, Write},
    path::Path,
    sync::LazyLock,
};
use zip::ZipArchive;
use rust_i18n::t;
/// Shared HTTP client: one connection pool per process instead of a
/// fresh `Client::new()` per request (fewer TLS handshakes, fewer sockets).
static HTTP: LazyLock<Client> = LazyLock::new(Client::new);
pub enum State {
    GettingDownloadList(String, VersionType),
    Downloading(DownloadList),
    PreparingJavaDownload(Java),
    DownloadingJava {
        downloaded: u64,
        total: u64,
        download: reqwest::Response,
        folder_to_store: String,
        file_to_write: File,
        java: Java,
    },
    ExtractingJava(String, Java),
    DownloadingMissingFiles(DownloadList),
    PreparingUpdate(String),
    DownloadingUpdate {
        downloaded: u64,
        total: u64,
        download: reqwest::Response,
        archive_path: String,
        archive_file: File,
    },
    PreparingNeoForge {
        mc_version: String,
        neoforge_version: String,
    },
    DownloadingNeoForgeInstaller {
        downloaded: u64,
        total: u64,
        download: reqwest::Response,
        installer_path: String,
        installer_file: File,
        mc_version: String,
        neoforge_version: String,
    },
    RunningNeoForgeInstaller {
        mc_version: String,
        neoforge_version: String,
        installer_path: String,
    },
    Idle,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum VersionType {
    Vanilla,
    Fabric { loader: String },
}
#[derive(Debug, Clone, PartialEq)]
pub enum Progress {
    GotDownloadList(i32),
    Downloaded(i32),
    Finished,
    StartedJavaDownload(u8),
    JavaDownloadProgressed(u8, u8),
    JavaDownloadFinished,
    JavaExtracted,
    MissingFilesDownloadProgressed(u16),
    MissingFilesDownloadFinished,
    UpdateStarted(u8),
    UpdateProgressed(u8, u8, u8),
    UpdateFinished,
    /// Managed Java `major` must be downloaded first (resume afterwards).
    NeoForgeNeedsJava(u64),
    /// Indeterminate status text for the NeoForge installer flow.
    NeoForgeStatus(String),
    /// NeoForge installed successfully (triggers version list refresh).
    NeoForgeFinished,
    Errored(String),
}
pub fn start<I: 'static + Hash + Copy + Send + Sync>(
    id: I,
    version: String,
    version_type: VersionType,
) -> Subscription<(I, Progress)> {
    Subscription::run_with((id, version, version_type), |data| {
        let (id, version, version_type) = data.clone();
        stream::channel(100, async move |mut output| {
            let mut state = State::GettingDownloadList(version, version_type);
            loop {
                match state {
                    State::Idle => break,
                    _ => {}
                }
                let ((out_id, progress), next_state) = download(id, state).await;
                let finished = matches!(next_state, State::Idle);
                let _ = output.send((out_id, progress)).await;
                if finished {
                    break;
                }
                state = next_state;
            }
        })
    })
}
/// Launcher-managed Temurin JRE identified by Java major version.
/// Downloaded from the Adoptium API on demand.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Java(pub u64);
/// Stable download URL for the latest GA Temurin JRE of a major version.
/// Redirects to the actual binary; reqwest follows them.
pub fn temurin_jre_url(major: u64) -> String {
    let os = match std::env::consts::OS {
        "windows" => "windows",
        "linux" => "linux",
        _ => "linux",
    };
    let arch = match std::env::consts::ARCH {
        "x86_64" => "x64",
        "aarch64" => "aarch64",
        _ => "x64",
    };
    format!("https://api.adoptium.net/v3/binary/latest/{major}/ga/{os}/{arch}/jre/hotspot/normal/eclipse")
}
pub fn start_java<I: 'static + Hash + Copy + Send + Sync>(
    id: I,
    java: Java,
) -> Subscription<(I, Progress)> {
    Subscription::run_with((id, java), |data| {
        let (id, java) = data.clone();
        stream::channel(100, async move |mut output| {
            let mut state = State::PreparingJavaDownload(java);
            loop {
                match state {
                    State::Idle => break,
                    _ => {}
                }
                let ((out_id, progress), next_state) = download(id, state).await;
                let finished = matches!(next_state, State::Idle);
                let _ = output.send((out_id, progress)).await;
                if finished {
                    break;
                }
                state = next_state;
            }
        })
    })
}
pub fn start_missing_files<I: 'static + Hash + Copy + Send + Sync>(
    id: I,
    files: Vec<Download>,
) -> Subscription<(I, Progress)> {
    Subscription::run_with((id, files), |data| {
        let (id, files) = data.clone();
        stream::channel(100, async move |mut output| {
            let mut state = State::DownloadingMissingFiles(DownloadList {
                download_list: files.into(),
                client: HTTP.clone(),
            });
            loop {
                match state {
                    State::Idle => break,
                    _ => {}
                }
                let ((out_id, progress), next_state) = download(id, state).await;
                let finished = matches!(next_state, State::Idle);
                let _ = output.send((out_id, progress)).await;
                if finished {
                    break;
                }
                state = next_state;
            }
        })
    })
}
pub fn start_update<I: 'static + Hash + Copy + Send + Sync>(
    id: I,
    url: String,
) -> Subscription<(I, Progress)> {
    Subscription::run_with((id, url), |data| {
        let (id, url) = data.clone();
        stream::channel(100, async move |mut output| {
            let mut state = State::PreparingUpdate(url);
            loop {
                match state {
                    State::Idle => break,
                    _ => {}
                }
                let ((out_id, progress), next_state) = download(id, state).await;
                let finished = matches!(next_state, State::Idle);
                let _ = output.send((out_id, progress)).await;
                if finished {
                    break;
                }
                state = next_state;
            }
        })
    })
}
pub fn start_neoforge<I: 'static + Hash + Copy + Send + Sync>(
    id: I,
    mc_version: String,
    neoforge_version: String,
) -> Subscription<(I, Progress)> {
    Subscription::run_with((id, mc_version, neoforge_version), |data| {
        let (id, mc_version, neoforge_version) = data.clone();
        stream::channel(100, async move |mut output| {
            let mut state = State::PreparingNeoForge {
                mc_version,
                neoforge_version,
            };
            loop {
                match state {
                    State::Idle => break,
                    _ => {}
                }
                let ((out_id, progress), next_state) = download(id, state).await;
                let finished = matches!(next_state, State::Idle);
                let _ = output.send((out_id, progress)).await;
                if finished {
                    break;
                }
                state = next_state;
            }
        })
    })
}
/// Stable URL of a NeoForge installer jar on the NeoForged Maven.
pub fn neoforge_installer_url(neoforge_version: &str) -> String {
    format!(
        "https://maven.neoforged.net/releases/net/neoforged/neoforge/{v}/neoforge-{v}-installer.jar",
        v = neoforge_version
    )
}
/// Map a NeoForge version to its Minecraft version.
/// Old scheme: `20.2.59` -> `1.20.2`, `21.1.213` -> `1.21.1`.
/// New scheme: `26.1.0.5-beta` -> `26.1` (zero placeholder),
/// `26.1.1.7` -> `26.1.1`.
pub fn neoforge_mc_of_version(version: &str) -> Option<String> {
    let core = version.split('-').next().unwrap_or(version);
    let parts: Vec<&str> = core.split('.').collect();
    if parts.iter().any(|p| p.parse::<u64>().is_err()) {
        return None;
    }
    match parts.len() {
        4 => {
            if parts[2] == "0" {
                Some(format!("{}.{}", parts[0], parts[1]))
            } else {
                Some(format!("{}.{}.{}", parts[0], parts[1], parts[2]))
            }
        }
        3 => {
            if parts[0] == "1" {
                Some(format!("1.{}.{}", parts[1], parts[2]))
            } else {
                // Old scheme omits the leading `1`: `21.1.213` -> `1.21.1`.
                Some(format!("1.{}.{}", parts[0], parts[1]))
            }
        }
        _ => None,
    }
}
/// Full NeoForge version list from Maven metadata (latest first).
/// Returns `(minecraft_version, neoforge_version)` pairs.
pub async fn get_neoforge_versions() -> Result<Vec<(String, String)>, String> {
    let text = match HTTP
        .get("https://maven.neoforged.net/releases/net/neoforged/neoforge/maven-metadata.xml")
        .send()
        .await
    {
        Ok(ok) => match ok.text().await {
            Ok(ok) => ok,
            Err(e) => return Err(format!("Failed to read NeoForge version list: {e}")),
        },
        Err(e) => return Err(format!("Failed to fetch NeoForge version list: {e}. You can enter the version manually.")),
    };
    let mut versions = Vec::new();
    // Minimal manual parse: maven-metadata is a flat list of <version>X</version>.
    let mut rest = text.as_str();
    while let Some(start) = rest.find("<version>") {
        rest = &rest[start + "<version>".len()..];
        if let Some(end) = rest.find("</version>") {
            versions.push(rest[..end].trim().to_owned());
            rest = &rest[end + "</version>".len()..];
        } else {
            break;
        }
    }
    if versions.is_empty() {
        return Err("NeoForge version list is empty. You can enter the version manually.".to_string());
    }
    let mut pairs = Vec::new();
    for v in versions.iter().rev() {
        if let Some(mc) = neoforge_mc_of_version(v) {
            pairs.push((mc, v.clone()));
        }
    }
    Ok(pairs)
}
/// Fabric loader versions for a Minecraft version (latest first).
pub async fn get_fabric_loader_versions(mc_version: &str) -> Result<Vec<String>, String> {
    let text = match HTTP
        .get(format!(
            "https://meta.fabricmc.net/v2/versions/loader/{mc_version}"
        ))
        .send()
        .await
    {
        Ok(ok) => match ok.text().await {
            Ok(ok) => ok,
            Err(e) => return Err(format!("Failed to read Fabric loader list: {e}")),
        },
        Err(e) => return Err(format!("Failed to fetch Fabric loader list: {e}")),
    };
    let list: Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(e) => return Err(format!("Failed to parse Fabric loader list: {e}")),
    };
    let mut loaders = Vec::new();
    if let Some(arr) = list.as_array() {
        for entry in arr {
            if let Some(v) = entry["loader"]["version"].as_str() {
                loaders.push(v.to_owned());
            }
        }
    }
    if loaders.is_empty() {
        return Err(t!("dl.no_fabric", mc = mc_version).to_string());
    }
    Ok(loaders)
}
#[derive(Clone)]
pub struct DownloadList {
    /// Files still to fetch (front = next). A queue: popping the front
    /// is O(1) and never clones the remainder (the old `Vec` + `clone()`
    /// + `remove(0)` copied every pending entry per file).
    pub download_list: std::collections::VecDeque<Download>,
    pub client: Client,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Download {
    pub path: String,
    pub url: String,
}
/// Stream a URL straight to disk chunk by chunk: memory stays O(chunk)
/// no matter the file size (the old `.bytes()` held the whole file in
/// RAM). Parent folders are created, so nested library paths never fail
/// with "no such directory".
async fn stream_to_file(client: &Client, url: &str, path: &str) -> Result<(), String> {
    let mut response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Download failed: {e}"))?;
    if let Some(parent) = Path::new(path).parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|e| format!("Folder failed: {e}"))?;
    }
    let mut file = File::create(path).map_err(|e| format!("File create failed: {e}"))?;
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| format!("Download read failed: {e}"))?
    {
        file.write_all(&chunk)
            .map_err(|e| format!("File write failed: {e}"))?;
    }
    Ok(())
}
/// Unpack a downloaded `natives.jar` into its sibling folder and remove
/// the jar (the game loads extracted natives, not the archive).
fn extract_natives(jar_path: &str) -> Result<(), String> {
    let natives_file =
        File::open(jar_path).map_err(|e| format!("Natives open failed: {e}"))?;
    let mut archive = ZipArchive::new(BufReader::new(natives_file))
        .map_err(|e| format!("Natives zip broken: {e}"))?;
    let folder = jar_path.replace("/natives.jar", "");
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("Natives entry broken: {e}"))?;
        let outpath = format!("{folder}/{}", entry.mangled_name().to_string_lossy());
        if entry.is_dir() {
            println!("Creating directory: {:?}", outpath);
            std::fs::create_dir_all(&outpath)
                .map_err(|e| format!("Natives folder failed: {e}"))?;
        } else {
            println!("Extracting file: {:?}", outpath);
            let mut outfile =
                File::create(&outpath).map_err(|e| format!("Natives file failed: {e}"))?;
            std::io::copy(&mut entry, &mut outfile)
                .map_err(|e| format!("Natives extract failed: {e}"))?;
        }
    }
    fs::remove_file(jar_path).map_err(|e| format!("Natives cleanup failed: {e}"))?;
    Ok(())
}
async fn download<I: 'static + Hash + Copy + Send + Sync>(
    id: I,
    state: State,
) -> ((I, Progress), State) {
    match state {
        State::GettingDownloadList(version, version_type) => {
            let mc_dir = super::launcher::get_minecraft_dir();
            let version_name = match &version_type {
                VersionType::Vanilla => version.clone(),
                VersionType::Fabric { .. } => format!("{}-fabric", &version),
            };
            let version_folder = format!("{}/versions/{}", &mc_dir, version_name);
            let client = HTTP.clone();
            let vanilla_version_json = match &version_type {
                VersionType::Vanilla => {
                    match downloadversionjson(&version_type, &version, &version_folder, &client)
                        .await
                    {
                        Ok(json) => json,
                        Err(e) => return ((id, Progress::Errored(e)), State::Idle),
                    }
                }
                VersionType::Fabric { .. } => {
                    // The helper saves vanilla json as `{mc}.json` and the
                    // loader profile as `{mc}-fabric.json`, returning the
                    // loader profile: re-read the vanilla one for assets
                    // and the client jar below.
                    if let Err(e) = downloadversionjson(
                        &version_type,
                        &version,
                        &version_folder,
                        &client,
                    )
                    .await
                    {
                        return ((id, Progress::Errored(e)), State::Idle);
                    }
                    let vanilla_path = format!("{}/{}.json", version_folder, version);
                    let content = match fs::read_to_string(&vanilla_path) {
                        Ok(c) => c,
                        Err(e) => {
                            return (
                                (
                                    id,
                                    Progress::Errored(format!(
                                        "Version file unreadable: {e}"
                                    )),
                                ),
                                State::Idle,
                            )
                        }
                    };
                    match serde_json::from_str(&content) {
                        Ok(v) => v,
                        Err(e) => {
                            return (
                                (
                                    id,
                                    Progress::Errored(format!("Version file corrupt: {e}")),
                                ),
                                State::Idle,
                            )
                        }
                    }
                }
            };
            let version_json = super::getjson(format!("{}/{}.json", version_folder, version_name));
            let asset_url = match vanilla_version_json["assetIndex"]["url"].as_str() {
                Some(u) => u.to_owned(),
                None => {
                    return (
                        (
                            id,
                            Progress::Errored(
                                "Version metadata has no asset index.".to_owned(),
                            ),
                        ),
                        State::Idle,
                    )
                }
            };
            let asset_index_download = match client.get(asset_url).send().await {
                Ok(ok) => match ok.bytes().await {
                    Ok(b) => b,
                    Err(e) => {
                        return (
                            (
                                id,
                                Progress::Errored(format!("Asset index read failed: {e}")),
                            ),
                            State::Idle,
                        )
                    }
                },
                Err(e) => {
                    return (
                        (
                            id,
                            Progress::Errored(format!("Asset index download failed: {e}")),
                        ),
                        State::Idle,
                    )
                }
            };
            let assets_name = match vanilla_version_json["assets"].as_str() {
                Some(a) => a.to_owned(),
                None => {
                    return (
                        (
                            id,
                            Progress::Errored(
                                "Version metadata has no assets index.".to_owned(),
                            ),
                        ),
                        State::Idle,
                    )
                }
            };
            let asset_index_path =
                format!("{}/assets/indexes/{}.json", mc_dir, assets_name);
            match fs::create_dir_all(format!("{}/assets/indexes", mc_dir)) {
                Ok(ok) => ok,
                Err(e) => return ((id, Progress::Errored(e.to_string())), State::Idle),
            }
            let mut asset_index_file = match File::create(&asset_index_path) {
                Ok(ok) => ok,
                Err(e) => return ((id, Progress::Errored(e.to_string())), State::Idle),
            };
            match asset_index_file.write_all(&asset_index_download) {
                Ok(ok) => ok,
                Err(e) => return ((id, Progress::Errored(e.to_string())), State::Idle),
            }
            let asset_index_json = super::getjson(asset_index_path);
            let mut download_list = vec![];
            // The per-version client jar copy is required for vanilla and
            // Fabric (Knot locates the game through it). NeoForge installs
            // via its own installer flow and never reaches this code.
            let client_jar_url = match vanilla_version_json["downloads"]["client"]["url"]
                .as_str()
            {
                Some(u) => u.to_owned(),
                None => {
                    return (
                        (
                            id,
                            Progress::Errored(
                                "Version metadata has no client jar.".to_owned(),
                            ),
                        ),
                        State::Idle,
                    )
                }
            };
            download_list.push(Download {
                path: format!("{}/{}.jar", version_folder, version_name),
                url: client_jar_url,
            });
            match get_assets(&mc_dir, asset_index_json) {
                Ok(ok) => download_list.extend_from_slice(&ok),
                Err(e) => return ((id, Progress::Errored(e.to_string())), State::Idle),
            }
            let Some(vanilla_libs) = vanilla_version_json["libraries"].as_array() else {
                return (
                    (
                        id,
                        Progress::Errored("Version metadata has no libraries.".to_owned()),
                    ),
                    State::Idle,
                );
            };
            let libraries = match get_libraries(&mc_dir, vanilla_libs, &version_folder) {
                Ok(ok) => ok,
                Err(e) => return ((id, Progress::Errored(e.to_string())), State::Idle),
            };
            download_list.extend_from_slice(&libraries);
            if matches!(version_type, VersionType::Fabric { .. }) {
                let Some(loader_libs) = version_json["libraries"].as_array() else {
                    return (
                        (
                            id,
                            Progress::Errored(
                                "Loader profile has no libraries.".to_owned(),
                            ),
                        ),
                        State::Idle,
                    );
                };
                let libraries = match get_libraries(&mc_dir, loader_libs, &version_folder)
                {
                    Ok(ok) => ok,
                    Err(e) => return ((id, Progress::Errored(e.to_string())), State::Idle),
                };
                download_list.extend_from_slice(&libraries);
            }
            // Only what is missing hits the network; the queue owns the
            // remainder without per-file cloning.
            let queue: std::collections::VecDeque<Download> = download_list
                .into_iter()
                .filter(|i| !Path::new(&i.path).exists())
                .collect();
            (
                (id, Progress::GotDownloadList(queue.len() as i32)),
                State::Downloading(DownloadList {
                    download_list: queue,
                    client,
                }),
            )
        }
        State::Downloading(mut download_list) => {
            let Some(current) = download_list.download_list.pop_front() else {
                println!("finished");
                return ((id, Progress::Finished), State::Idle);
            };
            println!("Downloading {}", current.path);
            if let Err(e) =
                stream_to_file(&download_list.client, &current.url, &current.path).await
            {
                return ((id, Progress::Errored(e)), State::Idle);
            }
            if current.path.contains("natives.jar") && let Err(e) = extract_natives(&current.path)
            {
                return ((id, Progress::Errored(e)), State::Idle);
            }
            println!("starting next download.");
            (
                (
                    id,
                    Progress::Downloaded(download_list.download_list.len() as i32),
                ),
                State::Downloading(download_list),
            )
        }
        State::PreparingNeoForge {
            mc_version,
            neoforge_version,
        } => {
            let mc_dir = super::launcher::get_minecraft_dir();
            // Java required to run the installer: exact major from the
            // vanilla json when present, table fallback otherwise.
            let vanilla_path =
                format!("{mc_dir}/versions/{mc_version}/{mc_version}.json");
            let java_major = fs::read_to_string(&vanilla_path)
                .ok()
                .and_then(|c| serde_json::from_str::<Value>(&c).ok())
                .and_then(|v| v["javaVersion"]["majorVersion"].as_u64())
                .unwrap_or_else(|| super::launcher::fallback_java_major(&mc_version));
            let java_folder = format!(
                "{mc_dir}/dgrlauncher_java/{}",
                super::launcher::managed_java_folder(java_major)
            );
            if !Path::new(&java_folder).exists() {
                return ((id, Progress::NeoForgeNeedsJava(java_major)), State::Idle);
            }
            // The installer refuses a directory without launcher_profiles.json.
            let profiles = format!("{mc_dir}/launcher_profiles.json");
            if !Path::new(&profiles).exists() {
                if let Err(e) = fs::write(&profiles, "{\"profiles\":{}}") {
                    return ((id, Progress::Errored(e.to_string())), State::Idle);
                }
            }
            let downloads_dir = format!("{mc_dir}/downloads");
            if let Err(e) = fs::create_dir_all(&downloads_dir) {
                return ((id, Progress::Errored(e.to_string())), State::Idle);
            }
            let installer_path = format!(
                "{downloads_dir}/neoforge-{neoforge_version}-installer.jar"
            );
            // Reuse a previously downloaded installer.
            if Path::new(&installer_path).exists() {
                return (
                    (
                        id,
                        Progress::NeoForgeStatus(t!("dl.neoforge_running").to_string()),
                    ),
                    State::RunningNeoForgeInstaller {
                        mc_version,
                        neoforge_version,
                        installer_path,
                    },
                );
            }
            let download = HTTP.get(neoforge_installer_url(&neoforge_version)).send().await;
            match download {
                Ok(d) => {
                    let total = d.content_length().unwrap_or(0);
                    let file = match File::create(&installer_path) {
                        Ok(f) => f,
                        Err(e) => return ((id, Progress::Errored(e.to_string())), State::Idle),
                    };
                    (
                        (
                            id,
                            Progress::NeoForgeStatus(t!("dl.neoforge_downloading").to_string()),
                        ),
                        State::DownloadingNeoForgeInstaller {
                            downloaded: 0,
                            total,
                            download: d,
                            installer_path,
                            installer_file: file,
                            mc_version,
                            neoforge_version,
                        },
                    )
                }
                Err(e) => ((id, Progress::Errored(e.to_string())), State::Idle),
            }
        }
        State::DownloadingNeoForgeInstaller {
            downloaded,
            total,
            mut download,
            installer_path,
            mut installer_file,
            mc_version,
            neoforge_version,
        } => match download.chunk().await {
            Ok(Some(chunk)) => {
                let downloaded = downloaded + chunk.len() as u64;
                if let Err(e) = std::io::Write::write_all(&mut installer_file, &chunk) {
                    return ((id, Progress::Errored(e.to_string())), State::Idle);
                }
                let text = if total > 0 {
                    t!(
                        "dl.neoforge_progress",
                        done = downloaded / 1048576,
                        total = total / 1048576
                    )
                    .to_string()
                } else {
                    t!(
                        "dl.neoforge_progress_unknown",
                        done = downloaded / 1048576
                    )
                    .to_string()
                };
                (
                    (id, Progress::NeoForgeStatus(text)),
                    State::DownloadingNeoForgeInstaller {
                        downloaded,
                        total,
                        download,
                        installer_path,
                        installer_file,
                        mc_version,
                        neoforge_version,
                    },
                )
            }
            Ok(None) => (
                (
                    id,
                    Progress::NeoForgeStatus(t!("dl.neoforge_running").to_string()),
                ),
                State::RunningNeoForgeInstaller {
                    mc_version,
                    neoforge_version,
                    installer_path,
                },
            ),
            Err(e) => ((id, Progress::Errored(e.to_string())), State::Idle),
        },
        State::RunningNeoForgeInstaller {
            mc_version,
            neoforge_version,
            installer_path,
        } => {
            let mc_dir = super::launcher::get_minecraft_dir();
            // Managed java for this MC version (ensured in PreparingNeoForge).
            let vanilla_path =
                format!("{mc_dir}/versions/{mc_version}/{mc_version}.json");
            let java_major = fs::read_to_string(&vanilla_path)
                .ok()
                .and_then(|c| serde_json::from_str::<Value>(&c).ok())
                .and_then(|v| v["javaVersion"]["majorVersion"].as_u64())
                .unwrap_or_else(|| super::launcher::fallback_java_major(&mc_version));
            let java_bin = if std::env::consts::OS == "windows" {
                format!(
                    "{mc_dir}/dgrlauncher_java/{}/bin/javaw.exe",
                    super::launcher::managed_java_folder(java_major)
                )
            } else {
                format!(
                    "{mc_dir}/dgrlauncher_java/{}/bin/java",
                    super::launcher::managed_java_folder(java_major)
                )
            };
            let output = tokio::process::Command::new(&java_bin)
                .arg("-jar")
                .arg(&installer_path)
                .arg("--install-client")
                .arg(&mc_dir)
                .output()
                .await;
            match output {
                Ok(out) if out.status.success() => {
                    let version_id = format!("neoforge-{neoforge_version}");
                    let produced =
                        format!("{mc_dir}/versions/{version_id}/{version_id}.json");
                    if Path::new(&produced).exists() {
                        ((id, Progress::NeoForgeFinished), State::Idle)
                    } else {
                        (
                            (
                                id,
                                Progress::Errored(format!(
                                    "Installer finished but {produced} is missing."
                                )),
                            ),
                            State::Idle,
                        )
                    }
                }
                Ok(out) => {
                    let tail = String::from_utf8_lossy(&out.stderr);
                    let tail: String =
                        tail.lines().rev().take(5).collect::<Vec<_>>().join(" | ");
                    (
                        (
                            id,
                            Progress::Errored(format!(
                                "NeoForge installer failed (exit {}): {}",
                                out.status,
                                tail.chars().take(300).collect::<String>()
                            )),
                        ),
                        State::Idle,
                    )
                }
                Err(e) => ((id, Progress::Errored(e.to_string())), State::Idle),
            }
        }
        State::Idle => iced::futures::future::pending().await,
        State::PreparingJavaDownload(java) => {
            let os = std::env::consts::OS;
            let java_url = temurin_jre_url(java.0);
            let mc_dir = super::launcher::get_minecraft_dir();
            let folder_to_store_download = format!("{}/dgrlauncher_java", mc_dir);
            match fs::create_dir_all(&folder_to_store_download) {
                Ok(ok) => ok,
                Err(e) => return ((id, Progress::Errored(e.to_string())), State::Idle),
            }
            let download = HTTP.get(java_url).send().await;
            let file_name = match os {
                "linux" => "compressed.tar.gz",
                "windows" => "compressed.zip",
                _ => {
                    return (
                        (id, Progress::Errored("System not supported.".to_owned())),
                        State::Idle,
                    )
                }
            };
            let file_to_write =
                match File::create(format!("{}/{}", folder_to_store_download, file_name)) {
                    Ok(ok) => ok,
                    Err(e) => return ((id, Progress::Errored(e.to_string())), State::Idle),
                };
            match download {
                Ok(d) => {
                    let size = d.content_length().unwrap_or(0);
                    (
                        (id, Progress::StartedJavaDownload((size / 1048576) as u8)),
                        State::DownloadingJava {
                            downloaded: 0,
                            total: size,
                            download: d,
                            folder_to_store: folder_to_store_download,
                            file_to_write,
                            java,
                        },
                    )
                }
                Err(e) => ((id, Progress::Errored(e.to_string())), State::Idle),
            }
        }
        State::DownloadingJava {
            downloaded,
            total,
            mut download,
            folder_to_store,
            mut file_to_write,
            java,
        } => match download.chunk().await {
            Ok(Some(chunk)) => {
                let downloaded = downloaded + chunk.len() as u64;
                let percentage = ((downloaded as f32 / total as f32) * 100.0) as u8;
                let mb_downloaded = (downloaded / 1048576) as u8;
                match file_to_write.write_all(&chunk) {
                    Ok(ok) => ok,
                    Err(e) => return ((id, Progress::Errored(e.to_string())), State::Idle),
                }
                (
                    (
                        id,
                        Progress::JavaDownloadProgressed(mb_downloaded, percentage),
                    ),
                    State::DownloadingJava {
                        downloaded,
                        total,
                        download,
                        folder_to_store,
                        file_to_write,
                        java,
                    },
                )
            }
            Ok(None) => (
                (id, Progress::JavaDownloadFinished),
                State::ExtractingJava(folder_to_store, java),
            ),
            Err(e) => ((id, Progress::Errored(e.to_string())), State::Idle),
        },
        State::ExtractingJava(folder, java) => {
            let os = std::env::consts::OS;
            let file_name = match os {
                "linux" => "compressed.tar.gz",
                "windows" => "compressed.zip",
                _ => return ((id, Progress::Errored("System not supported.".to_owned())), State::Idle),
            };
            let compressed_java = match File::open(format!("{}/{}", folder, file_name)) {
                Ok(ok) => ok,
                Err(e) => return ((id, Progress::Errored(e.to_string())), State::Idle),
            };
            let java_folder_name = format!("java{}", java.0);
            let mut f_folder_name = String::new();
            // Extraction is fallible end to end: any failure aborts with
            // an error instead of panicking mid-unpack.
            let extracted = (|| -> Result<(), String> {
                match os {
                    "windows" => {
                        let mut archive = ZipArchive::new(BufReader::new(compressed_java))
                            .map_err(|e| format!("Java archive broken: {e}"))?;
                        let mut got_first = false;
                        for i in 0..archive.len() {
                            let mut file = archive
                                .by_index(i)
                                .map_err(|e| format!("Java entry broken: {e}"))?;
                            if !got_first {
                                f_folder_name = file.name().to_string();
                                got_first = true;
                            }
                            let outpath = format!(
                                "{}/{}",
                                &folder,
                                file.mangled_name().to_string_lossy()
                            );
                            if file.is_dir() {
                                std::fs::create_dir_all(&outpath)
                                    .map_err(|e| format!("Java folder failed: {e}"))?;
                            } else {
                                let mut outfile = File::create(&outpath)
                                    .map_err(|e| format!("Java file failed: {e}"))?;
                                std::io::copy(&mut file, &mut outfile)
                                    .map_err(|e| format!("Java extract failed: {e}"))?;
                            }
                        }
                        Ok(())
                    }
                    "linux" => {
                        let gz_decoder =
                            flate2::read::GzDecoder::new(BufReader::new(compressed_java));
                        let mut archive = tar::Archive::new(gz_decoder);
                        let archive_iterator = archive
                            .entries()
                            .map_err(|e| format!("Java archive broken: {e}"))?;
                        let mut got_first = false;
                        for i in archive_iterator {
                            let mut i =
                                i.map_err(|e| format!("Java entry broken: {e}"))?;
                            if !got_first {
                                f_folder_name = i
                                    .header()
                                    .path()
                                    .map_err(|e| format!("Java entry broken: {e}"))?
                                    .file_name()
                                    .ok_or_else(|| "Java archive has no top folder.".to_string())?
                                    .to_string_lossy()
                                    .into_owned();
                                got_first = true;
                            }
                            i.unpack_in(&folder)
                                .map_err(|e| format!("Java extract failed: {e}"))?;
                        }
                        Ok(())
                    }
                    _ => Err("System not supported.".to_owned()),
                }
            })();
            if let Err(e) = extracted {
                return ((id, Progress::Errored(e)), State::Idle);
            }
            if f_folder_name.is_empty() {
                return (
                    (id, Progress::Errored("Java archive is empty.".to_owned())),
                    State::Idle,
                );
            }
            if let Err(e) = fs::rename(
                format!("{}/{}", folder, f_folder_name),
                format!("{}/{}", folder, java_folder_name),
            ) {
                return ((id, Progress::Errored(e.to_string())), State::Idle);
            }
            if let Err(e) = fs::remove_file(format!("{}/{}", folder, file_name)) {
                return ((id, Progress::Errored(e.to_string())), State::Idle);
            }
            ((id, Progress::JavaExtracted), State::Idle)
        }
        State::DownloadingMissingFiles(mut download_list) => {
            let Some(current) = download_list.download_list.pop_front() else {
                println!("finished");
                return ((id, Progress::MissingFilesDownloadFinished), State::Idle);
            };
            println!("Downloading {}", current.path);
            if let Err(e) =
                stream_to_file(&download_list.client, &current.url, &current.path).await
            {
                return ((id, Progress::Errored(e)), State::Idle);
            }
            if current.path.contains("natives.jar") && let Err(e) = extract_natives(&current.path)
            {
                return ((id, Progress::Errored(e)), State::Idle);
            }
            println!("starting next download.");
            (
                (
                    id,
                    Progress::MissingFilesDownloadProgressed(
                        download_list.download_list.len() as u16,
                    ),
                ),
                State::DownloadingMissingFiles(download_list),
            )
        }
        State::PreparingUpdate(url) => {
            let exec_path = match env::current_exe() {
                Ok(p) => p,
                Err(e) => return ((id, Progress::Errored(e.to_string())), State::Idle),
            };
            let archive_suffix = if url.to_lowercase().ends_with(".zip") {
                "update_tmp.zip"
            } else {
                "update_tmp.tar.gz"
            };
            let archive_path = exec_path.with_file_name(archive_suffix);
            let archive_file = match File::create(&archive_path) {
                Ok(f) => f,
                Err(e) => return ((id, Progress::Errored(e.to_string())), State::Idle),
            };
            let download = HTTP.get(&url).send().await;
            match download {
                Ok(d) => {
                    let size = d.content_length().unwrap_or(0);
                    (
                        (id, Progress::UpdateStarted((size / 1048576) as u8)),
                        State::DownloadingUpdate {
                            downloaded: 0,
                            total: size,
                            download: d,
                            archive_path: archive_path.to_string_lossy().to_string(),
                            archive_file,
                        },
                    )
                }
                Err(e) => ((id, Progress::Errored(e.to_string())), State::Idle),
            }
        }
        State::DownloadingUpdate {
            downloaded,
            total,
            mut download,
            archive_path,
            mut archive_file,
        } => match download.chunk().await {
            Ok(Some(chunk)) => {
                let downloaded = downloaded + chunk.len() as u64;
                let percentage = if total > 0 {
                    ((downloaded as f32 / total as f32) * 100.) as u8
                } else {
                    0
                };
                let mb_downloaded = (downloaded / 1048576) as u8;
                if let Err(e) = archive_file.write_all(&chunk) {
                    return ((id, Progress::Errored(e.to_string())), State::Idle);
                }
                (
                    (
                        id,
                        Progress::UpdateProgressed(
                            mb_downloaded,
                            percentage,
                            (total / 1048576) as u8,
                        ),
                    ),
                    State::DownloadingUpdate {
                        downloaded,
                        total,
                        download,
                        archive_path,
                        archive_file,
                    },
                )
            }
            Ok(None) => {
                drop(archive_file);
                if let Err(e) = install_update_from_archive(&archive_path) {
                    let _ = fs::remove_file(&archive_path);
                    return ((id, Progress::Errored(e)), State::Idle);
                }
                let _ = fs::remove_file(&archive_path);
                ((id, Progress::UpdateFinished), State::Idle)
            }
            Err(e) => ((id, Progress::Errored(e.to_string())), State::Idle),
        },
    }
}
fn install_update_from_archive(archive_path: &str) -> Result<(), String> {
    let exec_path = env::current_exe().map_err(|e| e.to_string())?;
    let new_path = exec_path.with_extension("new");
    if archive_path.to_lowercase().ends_with(".zip") {
        let file = File::open(archive_path).map_err(|e| e.to_string())?;
        let mut archive = ZipArchive::new(BufReader::new(file)).map_err(|e| e.to_string())?;
        let mut preferred: Option<usize> = None;
        let mut legacy: Option<usize> = None;
        let mut fallback: Option<usize> = None;
        for i in 0..archive.len() {
            let entry = archive.by_index(i).map_err(|e| e.to_string())?;
            if !entry.is_file() {
                continue;
            }
            let name = entry.name().to_lowercase();
            if name.ends_with("dgrlauncher.exe") {
                preferred = Some(i);
                break;
            } else if name.ends_with("minelander.exe") || name == "minelander" {
                if legacy.is_none() {
                    legacy = Some(i);
                }
            } else if name.ends_with(".exe") && fallback.is_none() {
                fallback = Some(i);
            }
        }
        let index = preferred
            .or(legacy)
            .or(fallback)
            .ok_or_else(|| "No executable found in update archive.".to_string())?;
        let mut entry = archive.by_index(index).map_err(|e| e.to_string())?;
        let mut out = File::create(&new_path).map_err(|e| e.to_string())?;
        std::io::copy(&mut entry, &mut out).map_err(|e| e.to_string())?;
    } else {
        let file = File::open(archive_path).map_err(|e| e.to_string())?;
        let gz = flate2::read::GzDecoder::new(BufReader::new(file));
        let mut archive = tar::Archive::new(gz);
        let entries = archive.entries().map_err(|e| e.to_string())?;
        let mut legacy_blob: Option<Vec<u8>> = None;
        let mut found = false;
        for entry in entries {
            let mut entry = entry.map_err(|e| e.to_string())?;
            let path = entry.header().path().map_err(|e| e.to_string())?;
            let is_file = entry.header().entry_type().is_file();
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            let is_preferred = name == "dgrlauncher" || name == "dgrlauncher.exe";
            let is_legacy = name == "minelander" || name == "minelander.exe";
            if !is_file || (!is_preferred && !is_legacy) {
                continue;
            }
            if is_preferred {
                let mut out = File::create(&new_path).map_err(|e| e.to_string())?;
                std::io::copy(&mut entry, &mut out).map_err(|e| e.to_string())?;
                found = true;
                break;
            } else if legacy_blob.is_none() {
                let mut buf = Vec::new();
                std::io::copy(&mut entry, &mut buf).map_err(|e| e.to_string())?;
                legacy_blob = Some(buf);
            }
        }
        if !found {
            if let Some(blob) = legacy_blob {
                fs::write(&new_path, blob).map_err(|e| e.to_string())?;
                found = true;
            }
        }
        if !found {
            return Err("No executable found in update archive.".to_string());
        }
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perm = fs::metadata(&new_path)
                .map_err(|e| e.to_string())?
                .permissions();
            perm.set_mode(0o755);
            fs::set_permissions(&new_path, perm).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
pub async fn downloadversionjson(
    version_type: &VersionType,
    version: &String,
    foldertosave: &String,
    client: &Client,
) -> Result<Value, String> {
    async fn manifest_url(client: &Client, version: &str) -> Result<String, String> {
        let text = client
            .get("https://launchermeta.mojang.com/mc/game/version_manifest_v2.json")
            .send()
            .await
            .map_err(|e| format!("Version list download failed: {e}"))?
            .text()
            .await
            .map_err(|e| format!("Version list read failed: {e}"))?;
        let p: Value = serde_json::from_str(&text)
            .map_err(|e| format!("Version list parse failed: {e}"))?;
        let versions = p["versions"]
            .as_array()
            .ok_or_else(|| "Version list has no versions.".to_string())?;
        for i in versions {
            if i["id"].as_str().unwrap_or("") == version
                && let Some(url) = i["url"].as_str()
            {
                return Ok(url.to_owned());
            }
        }
        Err(format!("Unknown Minecraft version: {version}"))
    }
    async fn fetch_bytes(client: &Client, url: &str, what: &str) -> Result<Vec<u8>, String> {
        client
            .get(url)
            .send()
            .await
            .map_err(|e| format!("{what} download failed: {e}"))?
            .bytes()
            .await
            .map_err(|e| format!("{what} read failed: {e}"))
            .map(|b| b.to_vec())
    }
    fn save_and_parse(folder: &str, name: &str, bytes: &[u8]) -> Result<Value, String> {
        fs::create_dir_all(folder).map_err(|e| format!("Version folder failed: {e}"))?;
        let location = format!("{folder}/{name}.json");
        // Atomic: the launcher reads this file right after.
        let tmp = format!("{location}.tmp");
        fs::write(&tmp, bytes).map_err(|e| format!("Version file write failed: {e}"))?;
        fs::rename(&tmp, &location).map_err(|e| format!("Version file write failed: {e}"))?;
        let content =
            fs::read_to_string(&location).map_err(|e| format!("Version file unreadable: {e}"))?;
        serde_json::from_str(&content).map_err(|e| format!("Version file corrupt: {e}"))
    }
    println!("Downloading json...");
    let url = manifest_url(client, version).await?;
    let versionjson = fetch_bytes(client, &url, "Version json").await?;
    // Small version jsons: one-shot fetch is fine (bulk files stream).
    let vanilla: Value = save_and_parse(foldertosave, version, &versionjson)?;
    match version_type {
        VersionType::Vanilla => Ok(vanilla),
        VersionType::Fabric { loader } => {
            let verjson = fetch_bytes(
                client,
                &format!(
                    "https://meta.fabricmc.net/v2/versions/loader/{}/{}/profile/json",
                    version, loader
                ),
                "Fabric profile",
            )
            .await?;
            save_and_parse(foldertosave, &format!("{version}-fabric"), &verjson)
        }
    }
}
pub async fn get_downloadable_version_list(
    showallversions: bool,
) -> Result<Vec<Vec<String>>, String> {
    let client = HTTP.clone();
    let vanillaversionlistjson = match client
        .get("https://launchermeta.mojang.com/mc/game/version_manifest_v2.json")
        .send()
        .await
    {
        Ok(ok) => match ok.text().await {
            Ok(ok) => ok,
            Err(e) => return Err(format!("failed to get download list: {}", e)),
        },
        Err(e) => return Err(format!("failed to get download list: {}", e)),
    };
    let content = serde_json::from_str(&vanillaversionlistjson);
    let p: Value = match content {
        Ok(ok) => ok,
        Err(e) => return Err(format!("failed to read list as json: {}", e)),
    };
    let mut vanillaversionlist: Vec<String> = vec![];
    if let Some(versions) = p["versions"].as_array() {
        if showallversions {
            for i in versions {
                if let Some(id) = i["id"].as_str() {
                    vanillaversionlist.push(id.to_owned())
                }
            }
        } else {
            for i in versions {
                if i["type"] == "release" {
                    if let Some(id) = i["id"].as_str() {
                        vanillaversionlist.push(id.to_owned())
                    }
                }
            }
        }
    }
    let fabricversionlistjson = match client
        .get("https://meta.fabricmc.net/v2/versions/game")
        .send()
        .await
    {
        Ok(ok) => match ok.text().await {
            Ok(ok) => ok,
            Err(e) => return Err(format!("failed to get fabric download list: {}", e)),
        },
        Err(e) => return Err(format!("failed to get fabric download list: {}", e)),
    };
    let content = serde_json::from_str(&fabricversionlistjson);
    let p: Value = match content {
        Ok(ok) => ok,
        Err(e) => return Err(format!("failed to read fabric list as json: {}", e)),
    };
    let mut fabricversionlist: Vec<String> = vec![];
    if let Some(versions) = p.as_array() {
        if showallversions {
            for i in versions {
                if let Some(v) = i["version"].as_str() {
                    fabricversionlist.push(v.to_owned())
                }
            }
        } else {
            for i in versions {
                if i["stable"] == true {
                    if let Some(v) = i["version"].as_str() {
                        fabricversionlist.push(v.to_owned())
                    }
                }
            }
        }
    }
    Ok(vec![vanillaversionlist, fabricversionlist])
}
pub fn get_libraries(
    mc_dir: &String,
    libraries: &Vec<Value>,
    foldertosave: &String,
) -> Result<Vec<Download>, Box<dyn std::error::Error>> {
    let lib_dir = format!("{}/libraries/", mc_dir);
    let os = std::env::consts::OS;
    enum LibraryType {
        Natives,
        Normal,
        Old,
    }
    let mut library_download_list = vec![];
    for library in libraries {
        if library["rules"][0]["os"]["name"] == os || library["rules"][0]["os"]["name"].is_null() {
            let Some(libraryname) = library["name"].as_str() else {
                return Err("Library without a name.".into());
            };
            let mut lpieces: Vec<&str> = libraryname.split(':').collect();
            if lpieces.is_empty() {
                return Err(format!("Bad library coordinates: {libraryname}").into());
            }
            let firstpiece = lpieces.remove(0).replace('.', "/");
            // `artifact:version` at minimum for every branch below.
            if lpieces.len() < 2 {
                return Err(format!("Bad library coordinates: {libraryname}").into());
            }
            let libtype = if libraryname.contains(&format!("natives-{}", os)) {
                LibraryType::Natives
            } else if library["natives"][os].is_null() {
                LibraryType::Normal
            } else {
                LibraryType::Old
            };
            match libtype {
                LibraryType::Natives => {
                    let Some(last_piece) = lpieces.pop() else {
                        return Err(format!("Bad library coordinates: {libraryname}").into());
                    };
                    if lpieces.len() < 2 {
                        return Err(format!("Bad library coordinates: {libraryname}").into());
                    }
                    let lib = format!(
                        "{}/{}/{}-{}-{}.jar",
                        &firstpiece,
                        &lpieces.join("/"),
                        &lpieces[&lpieces.len() - 2],
                        &lpieces[&lpieces.len() - 1],
                        last_piece
                    );
                    let libpath =
                        super::launcher::artifact_path(library, &lib_dir, &lib);
                    let parent = Path::new(&libpath).parent().ok_or_else(|| {
                        std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            format!("Bad library path: {libpath}"),
                        )
                    })?;
                    fs::create_dir_all(parent).map_err(|e| {
                        std::io::Error::other(format!("Library folder failed: {e}"))
                    })?;
                    let unmodifiedurl = if !library["downloads"]["artifact"]["url"].is_null() {
                        library["downloads"]["artifact"]["url"].as_str().unwrap_or("")
                    } else if !library["url"].is_null() {
                        library["url"].as_str().unwrap_or("")
                    } else {
                        ""
                    };
                    let url = get_library_url(unmodifiedurl, lib);
                    library_download_list.push(Download { path: libpath, url })
                }
                LibraryType::Normal => {
                    let lib = format!(
                        "{}/{}/{}-{}.jar",
                        &firstpiece,
                        &lpieces.join("/"),
                        &lpieces[&lpieces.len() - 2],
                        &lpieces[&lpieces.len() - 1]
                    );
                    let libpath =
                        super::launcher::artifact_path(library, &lib_dir, &lib);
                    let parent = Path::new(&libpath).parent().ok_or_else(|| {
                        std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            format!("Bad library path: {libpath}"),
                        )
                    })?;
                    fs::create_dir_all(parent).map_err(|e| {
                        std::io::Error::other(format!("Library folder failed: {e}"))
                    })?;
                    let unmodifiedurl = if !library["downloads"]["artifact"]["url"].is_null() {
                        library["downloads"]["artifact"]["url"].as_str().unwrap_or("")
                    } else if !library["url"].is_null() {
                        library["url"].as_str().unwrap_or("")
                    } else {
                        ""
                    };
                    let url = get_library_url(unmodifiedurl, lib);
                    library_download_list.push(Download { path: libpath, url })
                }
                LibraryType::Old => {
                    let lib = format!(
                        "{}/{}/{}-{}-natives-{}.jar",
                        &firstpiece,
                        &lpieces.join("/"),
                        &lpieces[&lpieces.len() - 2],
                        &lpieces[&lpieces.len() - 1],
                        os
                    );
                    let libpath =
                        super::launcher::artifact_path(library, &lib_dir, &lib);
                    let parent = Path::new(&libpath).parent().ok_or_else(|| {
                        std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            format!("Bad library path: {libpath}"),
                        )
                    })?;
                    fs::create_dir_all(parent).map_err(|e| {
                        std::io::Error::other(format!("Library folder failed: {e}"))
                    })?;
                    let unmodifiedurl = if !library["downloads"]["artifact"]["url"].is_null() {
                        library["downloads"]["artifact"]["url"].as_str().unwrap_or("")
                    } else if !library["url"].is_null() {
                        library["url"].as_str().unwrap_or("")
                    } else if !library["downloads"]["classifiers"][format!("natives-{}", os)]["url"]
                        .is_null()
                        || library["downloads"]["classifiers"][format!("natives-{}-64", os)]["url"]
                            .is_string()
                    {
                        let url = if !library["downloads"]["classifiers"][format!("natives-{}", os)]
                            ["url"]
                            .is_null()
                        {
                            library["downloads"]["classifiers"][format!("natives-{}", os)]["url"]
                                .as_str()
                                .unwrap_or("")
                        } else {
                            library["downloads"]["classifiers"][format!("natives-{}-64", os)]["url"]
                                .as_str()
                                .unwrap_or("")
                        };
                        url
                    } else {
                        ""
                    };
                    let url = get_library_url(unmodifiedurl, lib);
                    library_download_list.push(Download { path: libpath, url })
                }
            }
        }
        if !library["downloads"]["classifiers"][format!("natives-{}", os)].is_null() {
            let url = library["downloads"]["classifiers"][format!("natives-{}", os)]["url"]
                .as_str()
                .unwrap_or("")
                .to_string();
            fs::create_dir_all(format!("{}/natives", foldertosave))?;
            let path = format!("{}/natives/natives.jar", foldertosave);
            library_download_list.push(Download { path, url });
        }
    }
    Ok(library_download_list)
}
fn get_library_url(unmodifiedurl: &str, lib: String) -> String {
    if unmodifiedurl.ends_with('/') {
        format!("{}{}", unmodifiedurl, lib)
    } else if unmodifiedurl.is_empty() {
        format!("https://libraries.minecraft.net/{}", lib)
    } else {
        unmodifiedurl.to_string()
    }
}
#[cfg(test)]
mod tests {
    use super::neoforge_mc_of_version;
    #[test]
    fn maps_neoforge_versions_to_mc() {
        assert_eq!(
            neoforge_mc_of_version("21.1.213"),
            Some("1.21.1".to_string())
        );
        assert_eq!(
            neoforge_mc_of_version("20.2.59"),
            Some("1.20.2".to_string())
        );
        assert_eq!(
            neoforge_mc_of_version("26.1.0.5-beta"),
            Some("26.1".to_string())
        );
        assert_eq!(
            neoforge_mc_of_version("26.1.1.7"),
            Some("26.1.1".to_string())
        );
        assert_eq!(neoforge_mc_of_version("garbage"), None);
    }
}
pub fn get_assets(mc_dir: &String, asset_index_json: Value) -> Result<Vec<Download>, String> {
    let save_to_resources = !asset_index_json["map_to_resources"].is_null();
    let mut download_list = Vec::new();
    if let Some(assets) = asset_index_json["objects"].as_object() {
        let assets_directory = format!("{}/assets/objects/", &mc_dir);
        let old_assets_directory = format!("{}/resources", &mc_dir);
        for (key, value) in assets.iter() {
            if let Some(hash) = value["hash"].as_str() {
                match save_to_resources {
                    true => {
                        match fs::create_dir_all(&old_assets_directory) {
                            Ok(ok) => ok,
                            Err(e) => return Err(e.to_string()),
                        };
                        let asset_path = format!("{}/{}", old_assets_directory, key);
                        let asset_url = format!(
                            "https://resources.download.minecraft.net/{}/{}",
                            &hash[0..2],
                            hash
                        );
                        download_list.push(Download {
                            path: asset_path,
                            url: asset_url,
                        });
                    }
                    false => {
                        match fs::create_dir_all(format!("{}/{}", assets_directory, &hash[0..2])) {
                            Ok(ok) => ok,
                            Err(e) => return Err(e.to_string()),
                        };
                        let asset_path = format!("{}/{}/{}", &assets_directory, &hash[0..2], &hash);
                        let asset_url = format!(
                            "https://resources.download.minecraft.net/{}/{}",
                            &hash[0..2],
                            hash
                        );
                        download_list.push(Download {
                            path: asset_path,
                            url: asset_url,
                        });
                    }
                }
            }
        }
    }
    Ok(download_list)
}
