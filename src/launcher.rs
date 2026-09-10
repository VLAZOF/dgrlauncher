use iced::{stream, Subscription};
use iced::futures::SinkExt;
use serde_json::Value;
use shared_child::SharedChild;
use std::{
    collections::HashMap,
    env,
    fs::{self, File},
    hash::{Hash, Hasher},
    io::{BufRead, BufReader, Read, Write},
    path::Path,
    process::{Command, Stdio},
    sync::{
        mpsc::{self, Receiver},
        Arc,
    },
    thread::{self, JoinHandle},
};
use uuid::Uuid;
pub enum State {
    Checking(Option<GameSettings>),
    Launching(GameSettings),
    GettingLogs((Receiver<String>, JoinHandle<()>)),
    Idle,
}
#[derive(Debug, Clone)]
pub enum Progress {
    Checked(Option<Missing>),
    Started(Arc<SharedChild>),
    GotLog(String),
    Finished,
    Errored(String),
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Missing {
    /// Temurin JRE of this major version must be downloaded.
    Java(u64),
    VersionFiles(Vec<super::downloader::Download>),
    VanillaJson(String, String),
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum JavaType {
    System,
    Custom,
    Automatic,
}
/// Folder name for a launcher-managed Temurin JRE, e.g. `java25`.
pub fn managed_java_folder(major: u64) -> String {
    format!("java{major}")
}
/// Absolute library path, preferring the exact `downloads.artifact.path`
/// from the version json. The manual layout computation is only a fallback:
/// it is wrong for 4-part coordinates (e.g. `mergetool:2.0.0:api`).
pub(crate) fn artifact_path(
    library: &Value,
    lib_dir: &str,
    computed_relative: &str,
) -> String {
    if let Some(p) = library["downloads"]["artifact"]["path"].as_str() {
        format!("{lib_dir}{p}")
    } else {
        format!("{lib_dir}{computed_relative}")
    }
}
/// Join classpath entries, dropping duplicates (loader and vanilla jsons
/// overlap; duplicate jars crash NeoForge's union filesystem).
pub(crate) fn dedupe_classpath(classpath: &str, separator: char) -> String {
    use std::collections::HashSet;
    let mut seen = HashSet::new();
    classpath
        .split(separator)
        .filter(|s| !s.is_empty())
        .filter(|s| seen.insert(s.to_string()))
        .collect::<Vec<_>>()
        .join(&separator.to_string())
}
/// Required Java major for a version. Mirrors the official launcher:
/// `javaVersion.majorVersion` from the version json (following `inheritsFrom`
/// for modded versions). Old versions without the field fall back to a table.
pub fn required_java_major(version_json: &Value, game_version: &str) -> u64 {
    if let Some(major) = version_json["javaVersion"]["majorVersion"].as_u64() {
        return major;
    }
    if let Some(major) = version_json["javaVersion"]["Version"].as_u64() {
        return major;
    }
    fallback_java_major(game_version)
}
pub(crate) fn fallback_java_major(game_version: &str) -> u64 {
    // Strip loader suffixes like "1.21.1-fabric".
    let base = game_version.split('-').next().unwrap_or(game_version);
    let mut parts = base.split('.');
    let major: u64 = parts.next().and_then(|p| p.parse().ok()).unwrap_or(1);
    let minor: u64 = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    let patch: u64 = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    if major != 1 {
        // New version scheme, unknown requirement: latest known LTS.
        return 25;
    }
    match minor {
        0..=16 => 8,
        17 => 17,
        18..=19 => 17,
        20 if patch < 5 => 17,
        _ => 21,
    }
}
pub fn start<I: 'static + Hash + Copy + Send + Sync>(
    id: I,
    game_settings: Option<&GameSettings>,
) -> Subscription<(I, Progress)> {
    Subscription::run_with((id, game_settings.cloned()), |data| {
        let (id, game_settings) = data.clone();
        stream::channel(100, async move |mut output| {
            let mut state = State::Checking(game_settings);
            loop {
                match state {
                    State::Idle => break,
                    _ => {}
                }
                let ((out_id, progress), next_state) = launcher(id, state).await;
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
#[derive(Debug, Clone)]
pub struct GameSettings {
    pub account: super::auth::MinecraftAccount,
    pub game_version: String,
    pub jvm: String,
    pub jvmargs: Vec<String>,
    pub ram: f64,
    pub game_directory: String,
    pub java_type: JavaType,
    pub game_wrapper_commands: Vec<String>,
    pub enviroment_variables: HashMap<String, String>,
}
impl PartialEq for GameSettings {
    fn eq(&self, other: &Self) -> bool {
        self.account == other.account
            && self.game_version == other.game_version
            && self.jvm == other.jvm
            && self.jvmargs == other.jvmargs
            && self.ram.to_bits() == other.ram.to_bits()
            && self.game_directory == other.game_directory
            && self.java_type == other.java_type
            && self.game_wrapper_commands == other.game_wrapper_commands
            && self.enviroment_variables == other.enviroment_variables
    }
}
impl Eq for GameSettings {}
impl Hash for GameSettings {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.account.hash(state);
        self.game_version.hash(state);
        self.jvm.hash(state);
        self.jvmargs.hash(state);
        self.ram.to_bits().hash(state);
        self.game_directory.hash(state);
        self.java_type.hash(state);
        self.game_wrapper_commands.hash(state);
        let mut env: Vec<(&String, &String)> =
            self.enviroment_variables.iter().collect();
        env.sort_by(|a, b| a.0.cmp(b.0));
        env.hash(state);
    }
}
async fn launcher<I: Copy>(id: I, state: State) -> ((I, Progress), State) {
    match state {
        State::Checking(game_settings) => {
            let game_settings = game_settings.unwrap();
            let minecraft_dir = get_minecraft_dir();
            let version_dir = format!("{}/versions/{}", minecraft_dir, game_settings.game_version);
            let jsonpathstring = format!(
                "{}/versions/{}/{}.json",
                &minecraft_dir, game_settings.game_version, game_settings.game_version
            );
            let jsonpath = Path::new(&jsonpathstring);
            let mut json_file = match File::open(jsonpath) {
                Ok(file) => file,
                Err(e) => {
                    return (
                        (
                            id,
                            Progress::Errored(format!("Error {e}. Try reinstalling the version")),
                        ),
                        State::Idle,
                    )
                }
            };
            let mut json_file_content = String::new();
            json_file.read_to_string(&mut json_file_content).unwrap();
            let content = serde_json::from_str(&json_file_content);
            let mut p: Value = content.unwrap();
            if let Some(vanilla_ver) = p["inheritsFrom"].as_str() {
                let json_path = format!(
                    "{}/versions/{}/{}.json",
                    minecraft_dir, game_settings.game_version, vanilla_ver
                );
                if !Path::new(&json_path).exists() {
                    if Path::new(&format!(
                        "{}/versions/{}/{}.json",
                        minecraft_dir, vanilla_ver, vanilla_ver
                    ))
                    .exists()
                    {
                        let mut needed_json = File::open(format!(
                            "{}/versions/{}/{}.json",
                            minecraft_dir, vanilla_ver, vanilla_ver
                        ))
                        .unwrap();
                        let mut needed_json_content = Vec::new();
                        needed_json.read_to_end(&mut needed_json_content).unwrap();
                        File::create(&json_path)
                            .unwrap()
                            .write_all(&needed_json_content)
                            .unwrap();
                    } else {
                        println!("Vanilla Json needs to be downloaded.");
                        return (
                            (
                                id,
                                Progress::Checked(Some(Missing::VanillaJson(
                                    vanilla_ver.to_string(),
                                    format!(
                                        "{}/versions/{}",
                                        minecraft_dir, game_settings.game_version
                                    ),
                                ))),
                            ),
                            State::Idle,
                        );
                    }
                }
            }
            // Natives are usable only if already extracted. A missing (or
            // empty) folder, e.g. after a NeoForge installer run, means the
            // natives jars must still be downloaded.
            let natives_ready = match fs::read_dir(format!("{}/natives", version_dir)) {
                Ok(entries) => entries.count() > 0,
                Err(_) => false,
            };
            let mut missing_files_list = Vec::new();
            let modded = !p["inheritsFrom"].is_null();
            // Only BootstrapLauncher-based loaders (NeoForge) run without a
            // per-version game jar. Fabric's Knot locates the game through it,
            // vanilla needs it as the game itself.
            let skip_version_jar = modded
                && p["mainClass"]
                    .as_str()
                    .is_some_and(|m| m.contains("bootstraplauncher"));
            if modded {
                match super::downloader::get_libraries(
                    &minecraft_dir,
                    p["libraries"].as_array().unwrap(),
                    &version_dir,
                ) {
                    Ok(ok) => {
                        for i in ok {
                            if !Path::new(&i.path).exists() {
                                if i.path.contains("natives.jar") && natives_ready {
                                    continue;
                                }
                                missing_files_list.push(i);
                            }
                        }
                    }
                    Err(e) => println!("Failed to get libraries, ignoring. -> {e}"),
                }
                let mut vanilla_json_content = String::new();
                let mut vanilla_json_file = match File::open(format!(
                    "{}/versions/{}/{}.json",
                    minecraft_dir,
                    game_settings.game_version,
                    p["inheritsFrom"].as_str().unwrap()
                )) {
                    Ok(ok) => ok,
                    Err(_) => panic!("no!!!"),
                };
                vanilla_json_file
                    .read_to_string(&mut vanilla_json_content)
                    .unwrap();
                let content = serde_json::from_str(&vanilla_json_content);
                p = content.unwrap();
            }
            // Modded BootstrapLauncher versions (NeoForge) run from
            // libraries; the per-version jar is required otherwise.
            if !skip_version_jar {
                let version_jar_path = format!(
                    "{}/versions/{}/{}.jar",
                    minecraft_dir, game_settings.game_version, game_settings.game_version
                );
                if !Path::new(&version_jar_path).exists()
                    || (Path::new(&version_jar_path).exists()
                        && super::is_file_empty(&version_jar_path))
                {
                    if let Some(url) = p["downloads"]["client"]["url"].as_str() {
                        missing_files_list.push(super::downloader::Download {
                            path: version_jar_path,
                            url: url.to_string(),
                        })
                    }
                }
            }
            match super::downloader::get_libraries(
                &minecraft_dir,
                p["libraries"].as_array().unwrap(),
                &version_dir,
            ) {
                Ok(ok) => {
                    for i in ok {
                        if !Path::new(&i.path).exists() {
                            if i.path.contains("natives.jar") && natives_ready {
                                continue;
                            }
                            missing_files_list.push(i);
                        }
                    }
                }
                Err(e) => println!("Failed to get libraries, ignoring. -> {e}"),
            }
            let asset_index_path = format!(
                "{}/assets/indexes/{}.json",
                minecraft_dir,
                p["assets"].as_str().unwrap()
            );
            if !Path::new(&asset_index_path).exists() {
                match reqwest::get(p["assetIndex"]["url"].as_str().unwrap()).await {
                    Ok(ok) => {
                        let bytes = ok.bytes().await.unwrap();
                        match fs::create_dir_all(format!("{}/assets/indexes", minecraft_dir)) {
                            Ok(ok) => ok,
                            Err(e) => {
                                println!("Failed to create asset index directory, ignoring. -> {e}")
                            }
                        }
                        match File::create(&asset_index_path) {
                            Ok(mut ok) => match ok.write_all(&bytes) {
                                Ok(ok) => ok,
                                Err(e) => {
                                    println!("Failed to write to asset index, ignoring. -> {e}")
                                }
                            },
                            Err(e) => println!("Failed to create asset index, ignoring. -> {e}"),
                        }
                    }
                    Err(e) => println!("Failed to download asset index, ignoring. -> {e}"),
                };
            }
            if Path::new(&format!(
                "{}/assets/indexes/{}.json",
                minecraft_dir,
                p["assets"].as_str().unwrap()
            ))
            .exists()
            {
                let asset_p = super::getjson(asset_index_path);
                match super::downloader::get_assets(&minecraft_dir, asset_p) {
                    Ok(ok) => {
                        for i in ok {
                            if !Path::new(&i.path).exists() {
                                missing_files_list.push(i)
                            }
                        }
                    }
                    Err(e) => println!("Failed to get assets, ignoring. -> {e}"),
                }
            }
            if !missing_files_list.is_empty() {
                return (
                    (
                        id,
                        Progress::Checked(Some(Missing::VersionFiles(missing_files_list))),
                    ),
                    State::Idle,
                );
            }
            match game_settings.java_type {
                JavaType::Automatic => {
                    let required = modded_aware_required_major(&p, &minecraft_dir, &game_settings.game_version);
                    let folder = format!(
                        "{}/dgrlauncher_java/{}",
                        minecraft_dir,
                        managed_java_folder(required)
                    );
                    if !Path::new(&folder).exists() {
                        return ((id, Progress::Checked(Some(Missing::Java(required)))), State::Idle);
                    }
                }
                _ => {}
            }
            (
                (id, Progress::Checked(None)),
                State::Launching(game_settings),
            )
        }
        State::Launching(game_settings) => {
            let minecraft_directory = get_minecraft_dir();
            // game_directory is always the full isolated path
            // ({minecraft_dir}/dgrlauncher_instances/<version>),
            // computed by the caller. Shared files (versions, libraries,
            // assets, java) stay in `.minecraft`.
            let game_dir = game_settings.game_directory.clone();
            fs::create_dir_all(&game_dir).expect("Failed to create instance folder!");
            env::set_current_dir(&game_dir).expect("Failed to open instance folder!");
            let assets_dir = format!("{}/assets", &minecraft_directory);
            let jsonpathstring = format!(
                "{}/versions/{}/{}.json",
                &minecraft_directory, game_settings.game_version, game_settings.game_version
            );
            let jsonpath = Path::new(&jsonpathstring);
            let mut json_file = match File::open(jsonpath) {
                Ok(ok) => ok,
                Err(e) => return ((id, Progress::Errored(e.to_string())), State::Idle),
            };
            let mut json_file_content = String::new();
            json_file.read_to_string(&mut json_file_content).unwrap();
            let content = serde_json::from_str(&json_file_content);
            let p: Value = content.unwrap();
            let main_class = &p["mainClass"].as_str().unwrap();
            let asset_index = p["assets"].as_str().unwrap_or("").to_string();
            let native_directory = format!(
                "{}/versions/{}/natives",
                &minecraft_directory, game_settings.game_version
            );
            let mut library_list = lib_manager(&p);
            let mut version_jvm_args = get_game_jvm_args(&p, &native_directory);
            let mut version_game_args = vec![];
            let uuid = if game_settings.account.uuid.is_empty() {
                generate_uuid(&game_settings.account.username)
            } else {
                game_settings.account.uuid
            };
            let gamedata = vec![
                game_settings.account.username,
                game_settings.game_version.clone(),
                game_dir.to_string(),
                assets_dir,
                asset_index,
                uuid,
                game_settings.account.token,
                String::from("{}"),
                String::from("legacy"),
                String::from("Release"),
                String::from("Modified"),
                library_list.clone(),
            ];
            let is_modded = if game_settings.game_version.to_lowercase().contains("fabric")
                || game_settings.game_version.to_lowercase().contains("forge")
                || !p["inheritsFrom"].is_null()
            {
                let (modded_jvm_args, modded_game_args, vanilla_version_library_list) =
                    modded(&p, &game_settings.game_version, gamedata.clone());
                version_jvm_args.extend(modded_jvm_args);
                library_list.push_str(&vanilla_version_library_list);
                version_game_args = modded_game_args;
                true
            } else {
                false
            };
            let (java_path, java_args) = match game_settings.java_type{
                JavaType::System => ("java".to_owned(), get_vec_from("-XX:+UnlockExperimentalVMOptions -XX:+UnlockDiagnosticVMOptions -XX:+AlwaysActAsServerClassMachine -XX:+AlwaysPreTouch -XX:+DisableExplicitGC -XX:+UseNUMA -XX:NmethodSweepActivity=1 -XX:ReservedCodeCacheSize=400M -XX:NonNMethodCodeHeapSize=12M -XX:ProfiledCodeHeapSize=194M -XX:NonProfiledCodeHeapSize=194M -XX:-DontCompileHugeMethods -XX:MaxNodeLimit=240000 -XX:NodeLimitFudgeFactor=8000 -XX:+UseVectorCmov -XX:+PerfDisableSharedMem -XX:+UseFastUnorderedTimeStamps -XX:AllocatePrefetchStyle=3")),
                JavaType::Custom => (game_settings.jvm, game_settings.jvmargs),
                JavaType::Automatic => automatic_java(p.clone(), &game_settings.game_version, is_modded),
            };
            library_list.push_str(&format!(
                "{}/versions/{}/{}.jar",
                &minecraft_directory, game_settings.game_version, game_settings.game_version
            ));
            // Loader and vanilla jsons overlap (11 libs for NeoForge);
            // duplicates crash modded union filesystems, so drop them.
            let separator = match std::env::consts::OS {
                "linux" => ':',
                "windows" => ';',
                _ => panic!(),
            };
            library_list = dedupe_classpath(&library_list, separator);
            // For modded versions modded() already merged the loader's own
            // args with the vanilla ones; re-adding p's args here would
            // duplicate bootstrap flags (e.g. --launchTarget).
            if !is_modded {
                if let Some(arguments) = p["arguments"]["game"].as_array() {
                    let mut str_arguments = vec![];
                    for i in arguments {
                        if i.is_string() {
                            str_arguments.push(i.as_str().unwrap_or("").to_owned())
                        } else if i["value"].is_string() {
                            str_arguments.push(i["value"].as_str().unwrap().to_owned())
                        }
                    }
                    version_game_args.extend_from_slice(&get_game_args(str_arguments, &gamedata));
                } else if let Some(arguments) = p["minecraftArguments"].as_str() {
                    let oldargs: Vec<String> = arguments
                        .to_string()
                        .split_whitespace()
                        .map(String::from)
                        .collect();
                    version_game_args.extend_from_slice(&get_game_args(oldargs, &gamedata))
                }
            }
            let mut wrapper_commands = game_settings.game_wrapper_commands;
            let has_wrapper_commands;
            let mut game_command = if !wrapper_commands.is_empty() {
                has_wrapper_commands = true;
                Command::new(wrapper_commands.remove(0))
            } else {
                has_wrapper_commands = false;
                Command::new(&java_path)
            };
            if has_wrapper_commands {
                game_command.args(wrapper_commands).arg(&java_path);
            }
            game_command
                .arg(format!("-Xmx{}M", game_settings.ram * 1024.))
                .args(java_args.clone())
                .args(version_jvm_args.clone())
                .arg("-cp")
                .arg(library_list.clone())
                .arg(main_class)
                .args(version_game_args.clone());
            game_command.envs(game_settings.enviroment_variables);
            if cfg!(debug_assertions) {
                println!("{:?}", game_command)
            }
            if command_exists(game_command.get_program().to_str().unwrap()) {
                let game_process_receiver = run_and_log_game(game_command);
                if let Ok(game_pr_rec) = game_process_receiver.await {
                    (
                        (id, Progress::Started(game_pr_rec.1)),
                        State::GettingLogs(game_pr_rec.0),
                    )
                } else {
                    (
                        (
                            id,
                            Progress::Errored("Failed to start game process.".to_owned()),
                        ),
                        State::Idle,
                    )
                }
            } else {
                (
                    (
                        id,
                        Progress::Errored("Java or wrapper doesn't exist".to_owned()),
                    ),
                    State::Idle,
                )
            }
        }
        State::GettingLogs(receiver) => {
            if let Ok(log_line) = receiver.0.recv() {
                (
                    (id, Progress::GotLog(log_line)),
                    State::GettingLogs(receiver),
                )
            } else {
                receiver.1.join().expect("Failed to join child thread");
                ((id, Progress::Finished), State::Idle)
            }
        }
        State::Idle => iced::futures::future::pending().await,
    }
}
async fn run_and_log_game(
    mut game_command: Command,
) -> std::io::Result<((Receiver<String>, JoinHandle<()>), Arc<SharedChild>)> {
    let (sender, receiver) = mpsc::channel();
    let shared_child =
        SharedChild::spawn(game_command.stdout(Stdio::piped()).stderr(Stdio::piped()))
            .expect("failed to start game process.");
    let child_arc = Arc::new(shared_child);
    let child_clone = child_arc.clone();
    let child_thread = thread::spawn(move || {
        if let Some(stdout) = child_clone.take_stdout() {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                match line {
                    // The receiver is gone when the UI subscription is
                    // replaced (e.g. on exit): keep draining without panic.
                    Ok(line) => {
                        let _ = sender.send(line);
                    }
                    Err(err) => eprintln!("Error reading child output: {}", err),
                }
            }
        }
        if let Some(stderr) = child_clone.take_stderr() {
            let reader = BufReader::new(stderr);
            for line in reader.lines() {
                match line {
                    Ok(line) => {
                        let _ = sender.send(line);
                    }
                    Err(err) => eprintln!("Error reading child output: {}", err),
                }
            }
        }
        let status = child_clone
            .wait()
            .expect("Failed to wait for child process");
        println!("Child process exited with: {}", status);
    });
    Ok(((receiver, child_thread), child_arc))
}
pub fn get_minecraft_dir() -> String {
    match std::env::consts::OS {
        "linux" => format!("{}/.minecraft", std::env::var("HOME").unwrap()),
        "windows" => format!(
            "{}/AppData/Roaming/.minecraft",
            std::env::var("USERPROFILE").unwrap().replace('\\', "/")
        ),
        _ => panic!("System not supported."),
    }
}
pub async fn getinstalledversions() -> Vec<String> {
    let versions_dir = format!("{}/versions", get_minecraft_dir());
    if !Path::new(&versions_dir).exists() {
        fs::create_dir_all(&versions_dir).unwrap();
    }
    let entries = fs::read_dir(versions_dir).unwrap();
    let mut versions = entries
        .filter_map(|entry| {
            let path = entry.unwrap().path();
            if path.is_dir() {
                Some(path.file_name().unwrap().to_string_lossy().to_string())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    versions.sort_unstable_by(|a, b| get_version_order(b, a));
    versions
}
fn get_version_order(a: &str, b: &str) -> std::cmp::Ordering {
    let split_a: Vec<&str> = a.split(|c| c == '.' || c == '-').collect();
    let split_b: Vec<&str> = b.split(|c| c == '.' || c == '-').collect();
    if let (Some(major_a), Some(major_b)) = (
        split_a.first().and_then(|v| v.parse::<i32>().ok()),
        split_b.first().and_then(|v| v.parse::<i32>().ok()),
    ) {
        if major_a != major_b {
            return major_a.cmp(&major_b);
        }
    } else {
        return match (
            split_a.first().map(|v| v.parse::<i32>().is_ok()),
            split_b.first().map(|v| v.parse::<i32>().is_ok()),
        ) {
            (Some(true), Some(false)) => std::cmp::Ordering::Greater,
            (Some(false), Some(true)) => std::cmp::Ordering::Less,
            _ => std::cmp::Ordering::Equal,
        };
    }
    if let (Some(minor_a), Some(minor_b)) = (
        split_a.get(1).and_then(|v| v.parse::<i32>().ok()),
        split_b.get(1).and_then(|v| v.parse::<i32>().ok()),
    ) {
        if minor_a != minor_b {
            return minor_a.cmp(&minor_b);
        }
    } else {
        return match (
            split_a.get(1).map(|v| v.parse::<i32>().is_ok()),
            split_b.get(1).map(|v| v.parse::<i32>().is_ok()),
        ) {
            (Some(true), Some(false)) => std::cmp::Ordering::Greater,
            (Some(false), Some(true)) => std::cmp::Ordering::Less,
            _ => std::cmp::Ordering::Equal,
        };
    }
    if let (Some(release_a), Some(release_b)) = (
        split_a.get(2).and_then(|v| v.parse::<i32>().ok()),
        split_b.get(2).and_then(|v| v.parse::<i32>().ok()),
    ) {
        if release_a != release_b {
            return release_a.cmp(&release_b);
        }
    } else {
        return match (
            split_a.get(2).map(|v| v.parse::<i32>().is_ok()),
            split_b.get(2).map(|v| v.parse::<i32>().is_ok()),
        ) {
            (Some(true), Some(false)) => std::cmp::Ordering::Greater,
            (Some(false), Some(true)) => std::cmp::Ordering::Less,
            _ => std::cmp::Ordering::Equal,
        };
    }
    split_a
        .get(3)
        .unwrap_or(&"")
        .cmp(split_b.get(3).unwrap_or(&""))
}
fn get_game_args(arguments: Vec<String>, gamedata: &[String]) -> Vec<String> {
    let mut version_game_args = vec![];
    for i in arguments {
        match i.as_str() {
            "${auth_player_name}" => version_game_args.push(gamedata[0].clone()),
            "${version_name}" => version_game_args.push(gamedata[1].clone()),
            "${game_directory}" => version_game_args.push(gamedata[2].clone()),
            "${assets_root}" => version_game_args.push(gamedata[3].clone()),
            "${assets_index_name}" => version_game_args.push(gamedata[4].clone()),
            "${auth_uuid}" => version_game_args.push(gamedata[5].clone()),
            "${auth_session}" => version_game_args.push(gamedata[5].clone()),
            "${clientid}" => version_game_args.push(gamedata[5].clone()),
            "${auth_xuid}" => version_game_args.push(gamedata[5].clone()),
            "${auth_access_token}" => version_game_args.push(gamedata[6].clone()),
            "${user_properties}" => version_game_args.push(gamedata[7].clone()),
            "${user_type}" => version_game_args.push(gamedata[8].clone()),
            "${version_type}" => version_game_args.push(gamedata[9].clone()),
            "${classpath}" => version_game_args.push(gamedata[10].clone()),
            "${game_assets}" => {
                version_game_args.push(format!("{}/resources", get_minecraft_dir()))
            }
            "--demo" => {}
            _ => version_game_args.push(i.to_owned()),
        }
    }
    version_game_args
}
fn get_game_jvm_args(p: &Value, nativedir: &str) -> Vec<String> {
    let lib_dir = format!("{}/libraries", get_minecraft_dir());
    let separator = match std::env::consts::OS {
        "linux" => ":",
        "windows" => ";",
        _ => panic!(),
    };
    let mut version_jvm_args = vec![];
    if let Some(arguments) = p["arguments"]["jvm"].as_array() {
        for i in arguments {
            if i.is_string() {
                let mut value = i.as_str().unwrap().to_string();
                if value.contains("${natives_directory}") {
                    value = value.replace("${natives_directory}", nativedir);
                }
                if value.contains("${library_directory}") {
                    value = value.replace("${library_directory}", &lib_dir);
                }
                if value.contains("${classpath_separator}") {
                    value = value.replace("${classpath_separator}", separator);
                }
                if value.contains("${version_name}") {
                    let game_ver = p["id"].as_str().unwrap();
                    value = value.replace("${version_name}", game_ver)
                }
                if !value.contains("${classpath}") && !value.contains("-cp") {
                    version_jvm_args.push(value.to_string())
                }
            }
        }
    } else {
        version_jvm_args.push(format!("-Djava.library.path={}", &nativedir))
    }
    version_jvm_args
}
/// Vanilla version json for java lookup: for modded versions (loader json
/// without `javaVersion`) the inherited vanilla json is used.
fn version_json_for_java(p: &Value, mc_dir: &str, game_version: &str) -> Value {
    if let Some(vanilla) = p["inheritsFrom"].as_str() {
        let path = format!("{mc_dir}/versions/{game_version}/{vanilla}.json");
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(v) = serde_json::from_str(&content) {
                return v;
            }
        }
    }
    p.clone()
}
fn modded_aware_required_major(p: &Value, mc_dir: &str, game_version: &str) -> u64 {
    let v = version_json_for_java(p, mc_dir, game_version);
    // For modded ids (e.g. "neoforge-21.1.213") the fallback table must use
    // the vanilla id, not the loader id.
    let fallback_id = p["inheritsFrom"].as_str().unwrap_or(game_version);
    required_java_major(&v, fallback_id)
}
/// Plain-string game args from the loader json itself (no placeholders).
/// NeoForge bootstrap flags live here; fabric profiles have none.
fn loader_own_game_args(p: &Value) -> Vec<String> {
    let mut args = Vec::new();
    if let Some(list) = p["arguments"]["game"].as_array() {
        for i in list {
            if let Some(s) = i.as_str() {
                args.push(s.to_owned());
            } else if let Some(s) = i["value"].as_str() {
                args.push(s.to_owned());
            }
        }
    } else if let Some(old) = p["minecraftArguments"].as_str() {
        args.extend(old.split_whitespace().map(String::from));
    }
    args
}
fn automatic_java(p: Value, game_version: &String, ismodded: bool) -> (String, Vec<String>) {
    let mc_dir = get_minecraft_dir();
    let lookup_json;
    let lookup_ref = if ismodded {
        lookup_json = version_json_for_java(&p, &mc_dir, game_version);
        &lookup_json
    } else {
        &p
    };
    // Custom folder names must never reach the fallback table: for modded
    // versions the vanilla id (inheritsFrom) is the fallback reference.
    let fallback_id = if ismodded {
        p["inheritsFrom"]
            .as_str()
            .unwrap_or(game_version)
            .to_owned()
    } else {
        game_version.clone()
    };
    let required = required_java_major(lookup_ref, &fallback_id);
    let binary = if std::env::consts::OS == "windows" {
        "javaw.exe"
    } else {
        "java"
    };
    let path = format!(
        "{}/dgrlauncher_java/{}/bin/{}",
        mc_dir,
        managed_java_folder(required),
        binary
    );
    let modern_args = "-XX:+UnlockExperimentalVMOptions -XX:+UnlockDiagnosticVMOptions -XX:+AlwaysActAsServerClassMachine -XX:+AlwaysPreTouch -XX:+DisableExplicitGC -XX:+UseNUMA -XX:NmethodSweepActivity=1 -XX:ReservedCodeCacheSize=400M -XX:NonNMethodCodeHeapSize=12M -XX:ProfiledCodeHeapSize=194M -XX:NonProfiledCodeHeapSize=194M -XX:-DontCompileHugeMethods -XX:MaxNodeLimit=240000 -XX:NodeLimitFudgeFactor=8000 -XX:+UseVectorCmov -XX:+PerfDisableSharedMem -XX:+UseFastUnorderedTimeStamps -XX:AllocatePrefetchStyle=3";
    let java8args = "-XX:+UnlockExperimentalVMOptions -XX:+UnlockDiagnosticVMOptions -XX:+AlwaysActAsServerClassMachine -XX:+ParallelRefProcEnabled -XX:+DisableExplicitGC -XX:+AlwaysPreTouch -XX:+AggressiveOpts -XX:MaxInlineLevel=15 -XX:MaxVectorSize=32 -XX:+UseNUMA -XX:+UseDynamicNumberOfGCThreads -XX:NmethodSweepActivity=1 -XX:ReservedCodeCacheSize=350M -XX:-DontCompileHugeMethods -XX:MaxNodeLimit=240000 -XX:NodeLimitFudgeFactor=8000 -Dgraal.CompilerConfiguration=community";
    let args = if required == 8 { java8args } else { modern_args };
    (
        path,
        args.split(' ').map(|s| s.to_owned()).collect(),
    )
}
fn lib_manager(p: &Value) -> String {
    let os = std::env::consts::OS;
    let mc_dir = get_minecraft_dir();
    let mut library_list = String::new();
    if let Some(libraries) = p["libraries"].as_array() {
        let lib_dir = format!("{}/libraries/", &mc_dir);
        let separator = match os {
            "linux" => ':',
            "windows" => ';',
            _ => panic!(),
        };
        enum LibraryType {
            Natives,
            Normal,
            Old,
        }
        for library in libraries {
            if library["rules"][0]["os"]["name"] == os
                || library["rules"][0]["os"]["name"].is_null()
            {
                let libraryname = library["name"].as_str().unwrap();
                let mut lpieces: Vec<&str> = libraryname.split(':').collect();
                let firstpiece = lpieces[0].replace('.', "/");
                lpieces.remove(0);
                let lib_type = if libraryname.contains(&format!("natives-{}", os)) {
                    LibraryType::Natives
                } else if library["natives"][os].is_null() {
                    LibraryType::Normal
                } else {
                    LibraryType::Old
                };
                match lib_type {
                    LibraryType::Natives => {
                        let last_piece = lpieces.pop().unwrap();
                        let computed = format!(
                            "{}/{}/{}-{}-{}.jar",
                            &firstpiece,
                            &lpieces.join("/"),
                            &lpieces[&lpieces.len() - 2],
                            &lpieces[&lpieces.len() - 1],
                            last_piece
                        );
                        let libpath = artifact_path(library, &lib_dir, &computed);
                        library_list.push_str(&libpath);
                        library_list.push(separator);
                    }
                    LibraryType::Normal => {
                        let computed = format!(
                            "{}/{}/{}-{}.jar",
                            &firstpiece,
                            &lpieces.join("/"),
                            &lpieces[&lpieces.len() - 2],
                            &lpieces[&lpieces.len() - 1]
                        );
                        let libpath = artifact_path(library, &lib_dir, &computed);
                        library_list.push_str(&libpath);
                        library_list.push(separator);
                    }
                    LibraryType::Old => {
                        if libraryname == "tv.twitch:twitch-platform:6.5" {
                            continue;
                        }
                        let computed = format!(
                            "{}/{}/{}-{}-natives-{}.jar",
                            &firstpiece,
                            &lpieces.join("/"),
                            &lpieces[&lpieces.len() - 2],
                            &lpieces[&lpieces.len() - 1],
                            os
                        );
                        let libpath = artifact_path(library, &lib_dir, &computed);
                        library_list.push_str(&libpath);
                        library_list.push(separator);
                    }
                }
            }
        }
    }
    library_list
}
fn modded(
    p: &Value,
    game_version: &String,
    mut gamedata: Vec<String>,
) -> (Vec<String>, Vec<String>, String) {
    let mc_dir = get_minecraft_dir();
    // Loader's own game args first (e.g. NeoForge bootstrap flags
    // --launchTarget/--fml.*; fabric profiles carry none, so this is a no-op
    // there), vanilla placeholders appended after.
    let mut modded_game_args = loader_own_game_args(p);
    let vanillaversion = p["inheritsFrom"].as_str().unwrap();
    let vanillajsonpathstring = format!(
        "{}/versions/{}/{}.json",
        &mc_dir, game_version, vanillaversion
    );
    let mut vanillajson = File::open(vanillajsonpathstring).unwrap();
    let mut vjsoncontent = String::new();
    vanillajson.read_to_string(&mut vjsoncontent).unwrap();
    let vjson: Value = serde_json::from_str(&vjsoncontent).unwrap();
    let new_asset_index = vjson["assets"].as_str().unwrap().to_string();
    gamedata[4] = new_asset_index;
    if let Some(arguments) = vjson["arguments"]["game"].as_array() {
        let mut base_arguments = Vec::new();
        for i in arguments {
            if i.is_string() {
                base_arguments.push(i.as_str().unwrap().to_string())
            } else if i["value"].is_string() {
                base_arguments.push(i["value"].as_str().unwrap().to_string())
            }
        }
        modded_game_args.extend(get_game_args(base_arguments, &gamedata))
    } else if let Some(arguments) = vjson["minecraftArguments"].as_str() {
        if p["minecraftArguments"].is_null() {
            let oldargs: Vec<String> = arguments
                .to_string()
                .split_whitespace()
                .map(String::from)
                .collect();
            modded_game_args.extend_from_slice(&get_game_args(oldargs, &gamedata))
        }
    }
    let vanilla_version_jvm_args = get_game_jvm_args(
        &vjson,
        &format!("{}/versions/{}/natives", &mc_dir, game_version),
    );
    let vanilla_library_list = &lib_manager(&vjson);
    (
        vanilla_version_jvm_args,
        modded_game_args,
        vanilla_library_list.to_string(),
    )
}
fn command_exists(command_name: &str) -> bool {
    if let Ok(paths) = env::var("PATH") {
        let path_list: Vec<_> = env::split_paths(&paths).collect();
        for path in path_list {
            let command_path = path.join(command_name);
            if let Ok(metadata) = fs::metadata(&command_path) {
                if metadata.is_file() {
                    return true;
                }
            }
        }
    }
    false
}
fn generate_uuid(username: &str) -> String {
    let hash = md5::compute(username.as_bytes());
    let uuid = Uuid::from_slice(hash.as_slice()).unwrap();
    uuid.to_string()
}
fn get_vec_from(str: &str) -> Vec<String> {
    str.split(' ').map(|s| s.to_owned()).collect()
}
