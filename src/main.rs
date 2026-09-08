#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use self::widget::Element;
use iced::{
    alignment, clipboard,
    event::listen_with,
    widget::{button, column, container, row, svg, tooltip, Button},
    window::{self},
    Alignment, Length, Subscription, Task,
};
use launcher::get_minecraft_dir;
use serde::{Deserialize, Serialize};
use serde_json::{Number, Value};
use shared_child::SharedChild;
use std::io::Read;
use std::{collections::HashMap, env::set_current_dir};
use std::{
    env,
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};
use std::{fs::File, sync::Arc};
use widget::Renderer;
mod downloader;
mod launcher;
mod theme;
use theme::Theme;
mod auth;
mod screens;
mod system_java;
mod update_manager;
fn load_window_icon() -> Option<window::icon::Icon> {
    let bytes = include_bytes!("icons/dgrlauncher.png");
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (width, height) = (img.width(), img.height());
    window::icon::from_rgba(img.into_raw(), width, height).ok()
}
fn main() -> iced::Result {
    if !Path::new(&get_minecraft_dir()).exists() {
        match fs::create_dir_all(get_minecraft_dir()) {
            Ok(_) => println!("Minecraft directory was created."),
            Err(e) => println!("Failed to create Minecraft directory: {e}"),
        };
    }
    let old_exec = env::current_exe().unwrap().with_extension("old");
    if Path::new(&old_exec).exists() {
        match fs::remove_file(old_exec) {
            Ok(ok) => ok,
            Err(e) => println!("Failed to delete old executable: {e}"),
        }
    }
    iced::application(boot, update, view)
        .title(|state: &DgrLauncher| state.title())
        .subscription(subscription)
        .theme(|_state: &DgrLauncher| Theme)
        .window(window::Settings {
            size: iced::Size {
                width: 900.,
                height: 535.,
            },
            resizable: false,
            icon: load_window_icon(),
            exit_on_close_request: false,
            ..window::Settings::default()
        })
        .run()
}
#[derive(Default)]
struct DgrLauncher {
    screen: Screen,
    launcher: Launcher,
    downloaders: Vec<Downloader>,
    logs: Vec<String>,
    current_account: Account,
    current_account_mc_data: auth::MinecraftAccount,
    current_version: String,
    current_version_info: String,
    game_state_text: String,
    game_state_text_2: String,
    game_ram: f64,
    current_java_name: String,
    current_java: Java,
    game_wrapper_commands: String,
    game_enviroment_variables: String,
    show_all_versions_in_download_list: bool,
    all_versions: Vec<String>,
    java_name_list: Vec<String>,
    vanilla_versions_download_list: Vec<String>,
    install_mc_version: String,
    install_loader: LoaderChoice,
    fabric_loader_list: Vec<String>,
    fabric_loader_selected: String,
    neoforge_all: Vec<(String, String)>,
    neoforge_versions_for_mc: Vec<String>,
    neoforge_selected: String,
    neoforge_manual: String,
    neoforge_status: String,
    pending_neoforge_install: Option<(String, String)>,
    download_text: String,
    files_download_number: i32,
    needs_to_update_download_list: bool,
    detected_javas: Vec<system_java::SystemJava>,
    java_scan_status: String,
    custom_java_path: String,
    custom_java_flags: String,
    restrict_launch: bool,
    java_download_size: u8,
    game_proccess: GameProcess,
    update_available: bool,
    last_version: String,
    update_url: String,
    update_text: String,
    accounts: Vec<Account>,
    auth_code: auth::AuthCode,
    auth_token: auth::AuthToken,
    auth_xbox_data: auth::XboxLiveData,
    auth_status: String,
    local_account_to_add_name: String,
    is_first_launcher_use: bool
}
#[derive(Default, Serialize, Deserialize, Clone)]
struct Account {
    microsoft: bool,
    username: String,
    refresh_token: String,
}
/// Mod loader choice in the version installer. Exactly one can be active.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LoaderChoice {
    #[default]
    Vanilla,
    Fabric,
    NeoForge,
}
#[derive(Default)]
enum GameProcess {
    Running(Arc<SharedChild>),
    #[default]
    Null,
}
#[derive(PartialEq, Debug, Clone, Default)]
pub enum Screen {
    #[default]
    Main,
    Settings,
    Installation,
    CustomJava,
    Logs,
    ModifyCommand,
    InfoAndUpdates,
    Accounts,
    MicrosoftAccount,
    LocalAccount,
    GettingStarted,
    GettingStarted2
}
#[derive(Debug, Clone)]
enum Message {
    LoadVersionList(Vec<String>),
    Launch,
    CloseGame,
    ManageGameInfo((usize, launcher::Progress)),
    CurrentAccountChanged(String),
    VersionChanged(String),
    JavaChanged(String),
    GameRamChanged(f64),
    GameWrapperCommandsChanged(String),
    GameEnviromentVariablesChanged(String),
    ShowAllVersionsInDownloadListChanged(bool),
    GotDownloadList(Result<Vec<Vec<String>>, String>),
    VanillaVersionToDownloadChanged(String),
    InstallLoaderChanged(LoaderChoice),
    FabricLoaderChanged(String),
    GotFabricLoaders(Result<Vec<String>, String>),
    GotNeoForgeList(Result<Vec<(String, String)>, String>),
    ReloadNeoForgeList,
    NeoForgeVersionChanged(String),
    NeoForgeManualChanged(String),
    InstallPressed,
    ManageDownload((usize, downloader::Progress)),
    VanillaJson(Value),
    OpenGameFolder,
    OpenGameInstanceFolder,
    ChangeScreen(Screen),
    ScanSystemJavas,
    GotSystemJavas(Vec<system_java::SystemJava>),
    CustomJavaPathChanged(String),
    CustomJavaFlagsChanged(String),
    DetectedJavaSelected(String),
    SaveCustomJava,
    CheckedUpdates(Result<(String, String), String>),
    RecheckUpdates,
    Update,
    OpenURL(String),
    CopyToClipboard(String),
    CopyLogs,
    GotAuthCode(auth::AuthCode),
    ManageAuth((usize, auth::WaitProgress)),
    GotXboxToken(auth::XboxLiveData),
    GotMinecraftAuthData(auth::MinecraftAccount),
    RefreshLogin(Option<auth::MinecraftAccount>),
    LocalAccountNameChanged(String),
    AddedLocalAccount,
    RemoveAccount(String),
    Exit,
}
impl DgrLauncher {
    /// Current NeoForge install target from the installer menu, if complete.
    pub fn neoforge_install_target(&self) -> Option<(String, String)> {
        if self.install_mc_version.is_empty() {
            return None;
        }
        let nf = if !self.neoforge_manual.trim().is_empty() {
            self.neoforge_manual.trim().to_owned()
        } else {
            self.neoforge_selected.clone()
        };
        if nf.is_empty() {
            return None;
        }
        Some((self.install_mc_version.clone(), nf))
    }
    pub fn launch(&mut self) {
        if updateusersettingsfile(self.current_account.clone(), self.current_version.clone())
            .is_err()
        {
            println!("Failed to save user settings!")
        };
        let wrapper_commands_vec: Vec<String> = if !self.game_wrapper_commands.is_empty() {
            self.game_wrapper_commands
                .split(' ')
                .map(|s| s.to_owned())
                .collect()
        } else {
            Vec::new()
        };
        let enviroment_variables_hash_map = if !self.game_enviroment_variables.is_empty() {
            let mut hashmap = HashMap::new();
            let splitted_env_vars = self.game_enviroment_variables.split(' ');
            for i in splitted_env_vars {
                if i.contains('=') {
                    let splitted_i: Vec<String> = i.split('=').map(|i| i.to_owned()).collect();
                    hashmap.insert(splitted_i[0].clone(), splitted_i[1].clone());
                }
            }
            hashmap
        } else {
            HashMap::new()
        };
        let java_type = match self.current_java_name.as_str() {
            "System Java" => launcher::JavaType::System,
            "Custom" => launcher::JavaType::Custom,
            _ => launcher::JavaType::Automatic,
        };
        let game_settings = launcher::GameSettings {
            account: self.current_account_mc_data.clone(),
            game_version: self.current_version.clone(),
            jvm: self.current_java.path.clone(),
            jvmargs: self
                .current_java
                .flags
                .split(' ')
                .map(|s| s.to_owned())
                .collect(),
            ram: self.game_ram,
            game_wrapper_commands: wrapper_commands_vec,
            game_directory: game_instance_dir_for_version(&self.current_version),
            java_type,
            enviroment_variables: enviroment_variables_hash_map,
        };
        self.launcher.start(game_settings);
        self.logs.clear();
        self.current_account_mc_data.token = String::new();
    }
}
fn boot() -> (DgrLauncher, Task<Message>) {
        backward_compatibility_measures();
        let is_first_launcher_use = checksettingsfile();
        let mut file = File::open(get_config_file_path()).unwrap();
        let mut fcontent = String::new();
        file.read_to_string(&mut fcontent).unwrap();
        let content = serde_json::from_str(&fcontent);
        let p: Value = content.unwrap();
        let mut currentjava = Java {
            name: String::new(),
            path: String::new(),
            flags: String::new(),
        };
        currentjava.name = sanitize_java_name(p["current_java_name"].as_str().unwrap());
        // Migrate old configs ("Java 8 (DgrLauncher)", removed custom names...).
        if currentjava.name != p["current_java_name"].as_str().unwrap() {
            persist_current_java_name(&currentjava.name);
        }
        if currentjava.name == "Custom" {
            currentjava.path = p["custom_java_path"].as_str().unwrap_or("").to_owned();
            currentjava.flags = p["custom_java_flags"].as_str().unwrap_or("").to_owned();
        }
        let jvmnames = vec![
            "Automatic".to_owned(),
            "System Java".to_owned(),
            "Custom".to_owned(),
        ];
        let mc_dir = launcher::get_minecraft_dir();
        let game_instance_folder_path = format!("{}/dgrlauncher_instances", mc_dir);
        if !Path::new(&game_instance_folder_path).exists() {
            match fs::create_dir_all(&game_instance_folder_path) {
                Ok(_) => println!("Created game instances folder"),
                Err(e) => println!("Failed to create game instances folder: {}", e),
            }
        }
        if !Path::new(&format!("{}/launcher_profiles.json", mc_dir)).exists() {
            match File::create(format!("{}/launcher_profiles.json", mc_dir)) {
                Ok(mut file) => {
                    println!("Created launcher_profiles.json");
                    match file.write_all("{\"profiles\":{}}".as_bytes()) {
                        Ok(_) => println!("Wrote data to launcher_profiles.json"),
                        Err(e) => println!("Failed to write data to launcher_profiles.json: {}", e),
                    }
                }
                Err(d) => println!("Failed to create launcher_profiles.json: {}.", d),
            }
        }
        let mut accounts = vec![];
        if let Some(accounts_vec) = p["accounts"].as_array() {
            for account in accounts_vec {
                let microsoft = account["microsoft"].as_bool().unwrap();
                let username = account["username"].as_str().unwrap().to_string();
                let refresh_token = account["refresh_token"].as_str().unwrap().to_string();
                accounts.push(Account {
                    microsoft,
                    username,
                    refresh_token,
                })
            }
        }
        let mut current_account = Account {
            microsoft: p["current_account"]["microsoft"].as_bool().unwrap(),
            username: p["current_account"]["username"]
                .as_str()
                .unwrap()
                .to_owned(),
            refresh_token: p["current_account"]["refresh_token"]
                .as_str()
                .unwrap()
                .to_owned(),
        };
        // Self-heal: current_account must exist in accounts list.
        // Fixes configs broken by the old RemoveAccount handler.
        if !current_account.username.is_empty()
            && !accounts.iter().any(|a| a.username == current_account.username)
        {
            current_account = accounts.first().cloned().unwrap_or_default();
            persist_current_account(&current_account);
        }
        let initial_screen = match is_first_launcher_use{
            true => Screen::GettingStarted,
            false => Screen::Main,
        };
        let current_version = p["current_version"].as_str().unwrap().to_owned();
        let current_version_info = describe_version(&current_version);
        (
            DgrLauncher {
                screen: initial_screen,
                current_account: current_account,
                current_version,
                current_version_info,
                game_ram: p["game_ram"].as_f64().unwrap(),
                current_java_name: currentjava.name.clone(),
                current_java: currentjava,
                game_wrapper_commands: p["game_wrapper_commands"].as_str().unwrap().to_owned(),
                game_enviroment_variables: p["game_enviroment_variables"]
                    .as_str()
                    .unwrap()
                    .to_owned(),
                show_all_versions_in_download_list: p["show_all_versions"].as_bool().unwrap(),
                java_name_list: jvmnames,
                custom_java_path: p["custom_java_path"].as_str().unwrap_or("").to_owned(),
                custom_java_flags: p["custom_java_flags"].as_str().unwrap_or("").to_owned(),
                needs_to_update_download_list: true,
                accounts,
                is_first_launcher_use,
                ..Default::default()
            },
            Task::batch(vec![
                Task::perform(launcher::getinstalledversions(), Message::LoadVersionList),
                Task::perform(
                    update_manager::check_launcher_updates(),
                    Message::CheckedUpdates,
                ),
            ]),
        )
    }
impl DgrLauncher {
    fn title(&self) -> String {
        format!("DgrLauncher {}", env!("CARGO_PKG_VERSION"))
    }
}
    fn update(state: &mut DgrLauncher, message: Message) -> Task<Message> {
        match message {
            Message::Launch => {
                if state.current_java_name == "Custom" && state.current_java.path.trim().is_empty()
                {
                    state.game_state_text =
                        String::from("Set a custom Java path first (Settings > Custom Java).");
                    return Task::none();
                }
                if !state.restrict_launch
                    && !state.current_account.username.is_empty()
                    && !state.current_version.is_empty()
                {
                    if state.current_account.microsoft
                        && state.current_account_mc_data.token.is_empty()
                    {
                        state.game_state_text = String::from("Fetching account data...");
                        return Task::perform(
                            auth::login_with_refresh_token(
                                state.current_account.refresh_token.clone(),
                            ),
                            Message::RefreshLogin,
                        );
                    } else {
                        state.current_account_mc_data = auth::MinecraftAccount {
                            username: state.current_account.username.clone(),
                            token: "[pro]".to_string(),
                            uuid: String::new(),
                        }
                    }
                    state.launch();
                }
                Task::none()
            }
            Message::ManageGameInfo((_id, progress)) => {
                match progress {
                    launcher::Progress::Checked(missing) => {
                        if let Some(missing) = missing {
                            match missing {
                                launcher::Missing::Java(major) => {
                                    state.launcher.state = LauncherState::Waiting;
                                    state.game_state_text = format!(
                                        "Downloading Java {major}..."
                                    );
                                    state.downloaders.push(Downloader {
                                        state: DownloaderState::Idle,
                                        id: state.downloaders.len(),
                                    });
                                    let index = state.downloaders.len() - 1;
                                    state.downloaders[index]
                                        .start_java(downloader::Java(major))
                                }
                                launcher::Missing::VersionFiles(vec) => {
                                    state.game_state_text =
                                        String::from("Found missing files. Starting download.");
                                    state.launcher.state = LauncherState::Waiting;
                                    state.downloaders.push(Downloader {
                                        state: DownloaderState::Idle,
                                        id: state.downloaders.len(),
                                    });
                                    let index = state.downloaders.len() - 1;
                                    state.downloaders[index].start_missing_files(vec)
                                }
                                launcher::Missing::VanillaJson(ver, folder) => {
                                    state.launcher.state = LauncherState::Waiting;
                                    state.game_state_text =
                                        String::from("Downloading required json");
                                    return Task::perform(
                                        async move {
                                            match downloader::downloadversionjson(
                                                &downloader::VersionType::Vanilla,
                                                &ver,
                                                &folder,
                                                &reqwest::Client::new(),
                                            )
                                            .await
                                            {
                                                Ok(ok) => ok,
                                                Err(_) => Value::Null,
                                            }
                                        },
                                        Message::VanillaJson,
                                    );
                                }
                            }
                        }
                    }
                    launcher::Progress::Started(child) => {
                        // Move the settings along so the subscription keeps
                        // the same recipe and the log stream survives.
                        let settings = match &state.launcher.state {
                            LauncherState::Launching(s) => s.clone(),
                            LauncherState::GettingLogs(s) => s.clone(),
                            _ => return Task::none(),
                        };
                        state.launcher.state = LauncherState::GettingLogs(settings);
                        state.game_proccess = GameProcess::Running(child);
                        state.game_state_text = String::new()
                    }
                    launcher::Progress::GotLog(log) => {
                        state.logs.push(log);
                    }
                    launcher::Progress::Finished => {
                        state.game_state_text = String::new();
                        state.launcher.state = LauncherState::Idle;
                        state.game_proccess = GameProcess::Null;
                    }
                    launcher::Progress::Errored(e) => {
                        state.game_state_text = e;
                        state.launcher.state = LauncherState::Idle;
                    }
                }
                Task::none()
            }
            Message::VersionChanged(new_version) => {
                state.current_version_info = describe_version(&new_version);
                state.current_version = new_version;
                Task::none()
            }
            Message::ChangeScreen(new_screen) => {
                if state.screen == Screen::Settings {
                    updatesettingsfile(
                        state.game_ram,
                        state.current_java_name.clone(),
                        state.game_wrapper_commands.clone(),
                        state.game_enviroment_variables.clone(),
                        state.show_all_versions_in_download_list,
                    )
                    .unwrap();
                }
                state.screen = new_screen.clone();
                match new_screen {
                    Screen::Main => {
                        state.is_first_launcher_use = false;
                        Task::perform(launcher::getinstalledversions(), Message::LoadVersionList)
                    }
                    Screen::Installation => {
                        if !state.vanilla_versions_download_list.is_empty()
                            || state.needs_to_update_download_list
                        {
                            let show_all_versions = state.show_all_versions_in_download_list;
                            return Task::perform(
                                async move {
                                    downloader::get_downloadable_version_list(show_all_versions)
                                        .await
                                },
                                Message::GotDownloadList,
                            );
                        } else {
                            Task::none()
                        }
                    }
                    Screen::MicrosoftAccount => {
                        state.auth_status = String::from("Getting code and link...");
                        Task::perform(
                            async move { auth::request_code().await },
                            Message::GotAuthCode,
                        )
                    }
                    _ => Task::none(),
                }
            }
            Message::OpenGameFolder => {
                open::that(launcher::get_minecraft_dir()).unwrap();
                Task::none()
            }
            Message::OpenGameInstanceFolder => {
                // Each version runs in its own isolated folder:
                // dgrlauncher_instances/<version>.
                let dir = if state.current_version.is_empty() {
                    format!("{}/dgrlauncher_instances", launcher::get_minecraft_dir())
                } else {
                    game_instance_dir_for_version(&state.current_version)
                };
                if let Err(e) = fs::create_dir_all(&dir) {
                    println!("Failed to create game instance folder: {e}");
                }
                if let Err(e) = open::that(&dir) {
                    println!("Failed to open game instance folder: {e}");
                }
                Task::none()
            }
            Message::JavaChanged(selected_jvm_name) => {
                let name = sanitize_java_name(&selected_jvm_name);
                state.current_java_name = name.clone();
                state.current_java = if name == "Custom" {
                    let config = getjson(get_config_file_path());
                    Java {
                        name,
                        path: config["custom_java_path"]
                            .as_str()
                            .unwrap_or("")
                            .to_owned(),
                        flags: config["custom_java_flags"]
                            .as_str()
                            .unwrap_or("")
                            .to_owned(),
                    }
                } else {
                    Java {
                        name,
                        path: String::new(),
                        flags: String::new(),
                    }
                };
                persist_current_java_name(&state.current_java_name);
                Task::none()
            }
            Message::ScanSystemJavas => {
                state.java_scan_status = String::from("Scanning for installed Java...");
                Task::perform(system_java::scan_system_javas(), Message::GotSystemJavas)
            }
            Message::GotSystemJavas(found) => {
                if found.is_empty() {
                    state.java_scan_status =
                        String::from("No Java installations found. Enter the path manually.");
                } else {
                    state.java_scan_status =
                        format!("Found {} Java installation(s).", found.len());
                }
                state.detected_javas = found;
                Task::none()
            }
            Message::CustomJavaPathChanged(path) => {
                state.custom_java_path = path;
                Task::none()
            }
            Message::CustomJavaFlagsChanged(flags) => {
                state.custom_java_flags = flags;
                Task::none()
            }
            Message::DetectedJavaSelected(path) => {
                state.custom_java_path = path;
                Task::none()
            }
            Message::SaveCustomJava => {
                if state.custom_java_path.trim().is_empty() {
                    state.java_scan_status =
                        String::from("Enter a Java path or pick one from the scan results.");
                    return Task::none();
                }
                persist_custom_java(
                    state.custom_java_path.trim(),
                    state.custom_java_flags.trim(),
                );
                state.current_java_name = String::from("Custom");
                state.current_java = Java {
                    name: String::from("Custom"),
                    path: state.custom_java_path.trim().to_owned(),
                    flags: state.custom_java_flags.trim().to_owned(),
                };
                persist_current_java_name(&state.current_java_name);
                state.screen = Screen::Settings;
                Task::none()
            }
            Message::GameRamChanged(new_ram) => {
                state.game_ram = new_ram;
                Task::none()
            }
            Message::GameWrapperCommandsChanged(s) => {
                state.game_wrapper_commands = s;
                Task::none()
            }
            Message::ShowAllVersionsInDownloadListChanged(bool) => {
                state.needs_to_update_download_list = true;
                state.show_all_versions_in_download_list = bool;
                Task::perform(
                    async move { downloader::get_downloadable_version_list(bool).await },
                    Message::GotDownloadList,
                )
            }
            Message::GotDownloadList(result) => {
                match result {
                    Ok(list) => {
                        state.needs_to_update_download_list = false;
                        if !list.is_empty() {
                            state.vanilla_versions_download_list.clear();
                            for i in &list[0] {
                                let ii = i;
                                state.vanilla_versions_download_list.push(ii.to_string());
                            }
                        }
                    }
                    Err(err) => state.download_text = err,
                }
                Task::none()
            }
            Message::VanillaVersionToDownloadChanged(new_version) => {
                state.install_mc_version = new_version.clone();
                match state.install_loader {
                    LoaderChoice::Fabric => {
                        if new_version.is_empty() {
                            return Task::none();
                        }
                        return Task::perform(
                            async move {
                                downloader::get_fabric_loader_versions(&new_version).await
                            },
                            Message::GotFabricLoaders,
                        );
                    }
                    LoaderChoice::NeoForge => {
                        filter_neoforge_for_mc(state);
                        return Task::none();
                    }
                    LoaderChoice::Vanilla => {}
                }
                Task::none()
            }
            Message::InstallLoaderChanged(loader) => {
                state.install_loader = loader;
                state.download_text = String::new();
                match loader {
                    LoaderChoice::Fabric => {
                        if state.install_mc_version.is_empty() || !state.fabric_loader_list.is_empty() {
                            return Task::none();
                        }
                        let mc = state.install_mc_version.clone();
                        return Task::perform(
                            async move {
                                downloader::get_fabric_loader_versions(&mc).await
                            },
                            Message::GotFabricLoaders,
                        );
                    }
                    LoaderChoice::NeoForge => {
                        if state.neoforge_all.is_empty() {
                            state.neoforge_status =
                                String::from("Loading NeoForge versions...");
                            return Task::perform(
                                downloader::get_neoforge_versions(),
                                Message::GotNeoForgeList,
                            );
                        }
                        filter_neoforge_for_mc(state);
                        return Task::none();
                    }
                    LoaderChoice::Vanilla => Task::none(),
                }
            }
            Message::FabricLoaderChanged(loader) => {
                state.fabric_loader_selected = loader;
                Task::none()
            }
            Message::GotFabricLoaders(result) => {
                match result {
                    Ok(list) => {
                        state.fabric_loader_list = list.clone();
                        state.fabric_loader_selected =
                            list.first().cloned().unwrap_or_default();
                    }
                    Err(err) => state.download_text = err,
                }
                Task::none()
            }
            Message::GotNeoForgeList(result) => {
                match result {
                    Ok(pairs) => {
                        state.neoforge_all = pairs;
                        state.neoforge_status = String::new();
                        filter_neoforge_for_mc(state);
                    }
                    Err(err) => {
                        state.neoforge_status = format!(
                            "{err} (NeoForge Maven is unreachable; enter the version manually)"
                        )
                    }
                }
                Task::none()
            }
            Message::ReloadNeoForgeList => {
                state.neoforge_status = String::from("Loading NeoForge versions...");
                Task::perform(
                    downloader::get_neoforge_versions(),
                    Message::GotNeoForgeList,
                )
            }
            Message::NeoForgeVersionChanged(version) => {
                state.neoforge_selected = version;
                state.neoforge_manual = String::new();
                Task::none()
            }
            Message::NeoForgeManualChanged(version) => {
                state.neoforge_manual = version;
                Task::none()
            }
            Message::InstallPressed => {
                if state.install_mc_version.is_empty() {
                    state.download_text = String::from("Select a Minecraft version first.");
                    return Task::none();
                }
                match state.install_loader {
                    LoaderChoice::Vanilla => {
                        let version = state.install_mc_version.clone();
                        state.downloaders
                            .push(Downloader::new(state.downloaders.len()));
                        let index = state.downloaders.len() - 1;
                        state.downloaders[index]
                            .start(version, downloader::VersionType::Vanilla);
                    }
                    LoaderChoice::Fabric => {
                        if state.fabric_loader_selected.is_empty() {
                            state.download_text =
                                String::from("Select a Fabric loader version first.");
                            return Task::none();
                        }
                        let version = state.install_mc_version.clone();
                        let loader = state.fabric_loader_selected.clone();
                        state.downloaders
                            .push(Downloader::new(state.downloaders.len()));
                        let index = state.downloaders.len() - 1;
                        state.downloaders[index].start(
                            version,
                            downloader::VersionType::Fabric { loader },
                        );
                    }
                    LoaderChoice::NeoForge => {
                        let nf = if !state.neoforge_manual.trim().is_empty() {
                            state.neoforge_manual.trim().to_owned()
                        } else {
                            state.neoforge_selected.clone()
                        };
                        if nf.is_empty() {
                            state.neoforge_status = String::from(
                                "Select a NeoForge version or enter it manually.",
                            );
                            return Task::none();
                        }
                        let mc = state.install_mc_version.clone();
                        // Remember the target through the whole chain
                        // (vanilla prefetch -> installer -> optional java).
                        state.pending_neoforge_install = Some((mc.clone(), nf));
                        state.neoforge_status = String::from(
                            "Downloading Minecraft files first...",
                        );
                        // Prefetch vanilla files so the game doesn't have to
                        // download them on first launch; when this flow
                        // finishes, the installer starts (see Finished).
                        state.downloaders
                            .push(Downloader::new(state.downloaders.len()));
                        let index = state.downloaders.len() - 1;
                        state.downloaders[index]
                            .start(mc, downloader::VersionType::Vanilla);
                    }
                }
                Task::none()
            }
            Message::ManageDownload((id, progress)) => {
                match progress {
                    downloader::Progress::GotDownloadList(file_number) => {
                        state.download_text =
                            format!("Downloaded 0 from {} files. (0%)", file_number);
                        state.files_download_number = file_number;
                    }
                    downloader::Progress::Downloaded(remaining_files_number) => {
                        let downloaded_files = state.files_download_number - remaining_files_number;
                        let percentage = (downloaded_files as f32
                            / state.files_download_number as f32
                            * 100.0) as i32;
                        state.download_text = format!(
                            "Downloaded {} from {} files. ({}%)",
                            downloaded_files, state.files_download_number, percentage
                        );
                    }
                    downloader::Progress::Finished => {
                        state.download_text = String::from("Version installed successfully.");
                        for (index, downloader) in state.downloaders.iter().enumerate() {
                            if downloader.id == id {
                                state.downloaders.remove(index);
                                break;
                            }
                        }
                        // Vanilla prefetch for NeoForge done and nothing else
                        // is downloading: continue with the installer.
                        if state.downloaders.is_empty() {
                            if let Some((mc, nf)) = state.pending_neoforge_install.clone() {
                                state.downloaders
                                    .push(Downloader::new(state.downloaders.len()));
                                let index = state.downloaders.len() - 1;
                                state.downloaders[index].start_neoforge(mc, nf);
                            }
                        }
                        if state.is_first_launcher_use{
                            state.is_first_launcher_use = false;
                            state.screen = Screen::Main;
                            return Task::perform(launcher::getinstalledversions(), Message::LoadVersionList)
                        }
                    }
                    downloader::Progress::Errored(error) => {
                        state.download_text = format!("Failed to install: {error}");
                        state.neoforge_status = format!("Failed to install: {error}");
                        state.restrict_launch = false;
                        state.pending_neoforge_install = None;
                        for (index, downloader) in state.downloaders.iter().enumerate() {
                            if downloader.id == id {
                                state.downloaders.remove(index);
                                break;
                            }
                        }
                    }
                    downloader::Progress::StartedJavaDownload(size) => {
                        state.restrict_launch = true;
                        state.game_state_text = format!("Downloading java. 0 / {size} MiB (0%)");
                        state.java_download_size = size;
                    }
                    downloader::Progress::JavaDownloadProgressed(downloaded, percentage) => {
                        state.game_state_text = format!(
                            "Downloading Java. {downloaded} / {} MiB ({percentage}%)",
                            state.java_download_size
                        )
                    }
                    downloader::Progress::JavaDownloadFinished => {
                        state.game_state_text = String::from("Extracting Java")
                    }
                    downloader::Progress::JavaExtracted => {
                        state.game_state_text = String::from("Java was installed successfully.");
                        state.restrict_launch = false;
                        for (index, downloader) in state.downloaders.iter().enumerate() {
                            if downloader.id == id {
                                state.downloaders.remove(index);
                                break;
                            }
                        }
                        // Java downloaded for a pending NeoForge install:
                        // continue with the installer instead of launching.
                        if let Some((mc, nf)) = state.pending_neoforge_install.take() {
                            state.downloaders
                                .push(Downloader::new(state.downloaders.len()));
                            let index = state.downloaders.len() - 1;
                            state.downloaders[index].start_neoforge(mc, nf);
                            return Task::none();
                        }
                        state.launch();
                    }
                    downloader::Progress::NeoForgeNeedsJava(major) => {
                        state.neoforge_status = format!(
                            "Java {major} is required to run the installer. Downloading it first..."
                        );
                        state.restrict_launch = true;
                        state.downloaders.push(Downloader {
                            state: DownloaderState::Idle,
                            id: state.downloaders.len(),
                        });
                        let index = state.downloaders.len() - 1;
                        state.downloaders[index].start_java(downloader::Java(major));
                        // Remembered when JavaExtracted arrives; cleared on error.
                        if state.pending_neoforge_install.is_none() {
                            state.pending_neoforge_install =
                                state.neoforge_install_target();
                        }
                    }
                    downloader::Progress::NeoForgeStatus(text) => {
                        state.neoforge_status = text;
                    }
                    downloader::Progress::NeoForgeFinished => {
                        state.restrict_launch = false;
                        state.pending_neoforge_install = None;
                        state.neoforge_status =
                            String::from("NeoForge installed successfully.");
                        state.download_text =
                            String::from("NeoForge installed successfully.");
                        for (index, downloader) in state.downloaders.iter().enumerate() {
                            if downloader.id == id {
                                state.downloaders.remove(index);
                                break;
                            }
                        }
                        return Task::perform(
                            launcher::getinstalledversions(),
                            Message::LoadVersionList,
                        );
                    }
                    downloader::Progress::MissingFilesDownloadProgressed(missing_files) => {
                        state.restrict_launch = true;
                        state.game_state_text =
                            format!("Downloading missing files. {} left", missing_files);
                    }
                    downloader::Progress::MissingFilesDownloadFinished => {
                        state.restrict_launch = false;
                        for (index, downloader) in state.downloaders.iter().enumerate() {
                            if downloader.id == id {
                                state.downloaders.remove(index);
                                break;
                            }
                        }
                        state.launch();
                    }
                    downloader::Progress::UpdateStarted(total) => {
                        state.update_text = format!("Downloading update. 0 / {total} MiB (0%)")
                    }
                    downloader::Progress::UpdateProgressed(downloaded, percentage, total) => {
                        state.update_text = format!(
                            "Downloading update. {downloaded} / {total} MiB ({percentage}%)"
                        )
                    }
                    downloader::Progress::UpdateFinished => {
                        for (index, downloader) in state.downloaders.iter().enumerate() {
                            if downloader.id == id {
                                state.downloaders.remove(index);
                                break;
                            }
                        }
                        let exec_path = match env::current_exe() {
                            Ok(p) => p,
                            Err(e) => {
                                state.update_text =
                                    format!("Update failed: cannot locate executable: {e}");
                                return Task::none();
                            }
                        };
                        let old_path = exec_path.with_extension("old");
                        let new_path = exec_path.with_extension("new");
                        if let Err(e) = fs::rename(&exec_path, &old_path) {
                            state.update_text = format!("Update failed (backup step): {e}");
                            return Task::none();
                        }
                        if let Err(e) = fs::rename(&new_path, &exec_path) {
                            let _ = fs::rename(&old_path, &exec_path);
                            state.update_text = format!("Update failed (replace step): {e}");
                            return Task::none();
                        }
                        #[cfg(target_os = "linux")]
                        {
                            use std::os::unix::fs::PermissionsExt;
                            if let Ok(meta) = fs::metadata(&exec_path) {
                                let mut perm = meta.permissions();
                                perm.set_mode(0o755);
                                let _ = fs::set_permissions(&exec_path, perm);
                            }
                        }
                        state.update_text = String::from("Update installed successfully.");
                        match std::process::Command::new(&exec_path).spawn() {
                            Ok(_) => std::process::exit(0),
                            Err(e) => {
                                state.update_text =
                                    format!("Updated, but failed to restart: {e}");
                            }
                        }
                    }
                }
                Task::none()
            }
            Message::VanillaJson(result) => {
                if result.is_null() {
                    state.game_state_text =
                        String::from("Json download failed. Check your internet connection.");
                } else {
                    state.game_state_text = String::from("Json downloaded successfully.");
                }
                state.launch();
                Task::none()
            }
            Message::LoadVersionList(ver_list) => {
                state.all_versions = ver_list.clone();
                if ver_list.len() == 1{
                    state.current_version = ver_list[0].clone()
                }
                state.current_version_info = describe_version(&state.current_version);
                Task::none()
            }
            Message::GameEnviromentVariablesChanged(s) => {
                state.game_enviroment_variables = s;
                Task::none()
            }
            Message::Exit => {
                state.launcher.state = LauncherState::Idle;
                state.downloaders.clear();
                iced::exit()
            }
            Message::CloseGame => {
                match &state.game_proccess {
                    GameProcess::Running(process) => match process.kill() {
                        Ok(ok) => ok,
                        Err(e) => panic!("{}", e),
                    },
                    GameProcess::Null => todo!(),
                }
                Task::none()
            }
            Message::OpenURL(url) => {
                match open::that_detached(url) {
                    Ok(ok) => ok,
                    Err(e) => println!("Failed to open URL: {e}"),
                }
                Task::none()
            }
            Message::CheckedUpdates(result) => {
                match result {
                    Ok((url, last_version)) => {
                        state.update_available = true;
                        state.update_url = url;
                        state.last_version = last_version;
                    }
                    Err(e) => state.last_version = e,
                }
                Task::none()
            }
            Message::RecheckUpdates => {
                state.update_available = false;
                state.update_text = String::new();
                state.last_version = String::from("Checking for updates...");
                return Task::perform(
                    update_manager::check_launcher_updates(),
                    Message::CheckedUpdates,
                );
            }
            Message::Update => {
                state.downloaders.push(Downloader {
                    state: DownloaderState::Idle,
                    id: state.downloaders.len(),
                });
                let index = state.downloaders.len() - 1;
                state.downloaders[index].start_update(state.update_url.clone());
                Task::none()
            }
            Message::GotAuthCode(code) => {
                state.auth_status = String::from("Waiting for login...");
                state.auth_code = code;
                Task::none()
            }
            Message::ManageAuth((_id, progress)) => {
                match progress {
                    auth::WaitProgress::GotAuthToken(auth_token) => {
                        state.auth_token = auth_token.clone();
                        state.auth_status = String::from("Logging into Xbox Services...");
                        return Task::perform(
                            async move { auth::login_to_xbox(auth_token.access_token).await },
                            Message::GotXboxToken,
                        );
                    }
                    auth::WaitProgress::Waiting => (),
                    auth::WaitProgress::Error(e) => println!("auth error: {e}"),
                    auth::WaitProgress::Finished => {
                        state.auth_code.code = String::new();
                        state.auth_code.link = String::new();
                    }
                }
                Task::none()
            }
            Message::GotXboxToken(xbox_data) => {
                state.auth_xbox_data = xbox_data.clone();
                state.auth_status = String::from("Logging into Minecraft...");
                Task::perform(
                    async move { auth::login_to_minecraft(xbox_data).await },
                    Message::GotMinecraftAuthData,
                )
            }
            Message::GotMinecraftAuthData(mc_account) => {
                let refresh_token = state.auth_token.refresh_token.clone();
                let account = Account {
                    microsoft: true,
                    username: mc_account.username,
                    refresh_token,
                };
                state.accounts = save_account(account.clone());
                state.current_account = account;
                persist_current_account(&state.current_account);
                state.auth_status = String::from("Account added successfully!");
                if state.screen == Screen::MicrosoftAccount{
                    if state.is_first_launcher_use{
                        if state.all_versions.is_empty(){
                            state.screen = Screen::GettingStarted2;
                        } else{
                            state.screen = Screen::Main;
                            state.is_first_launcher_use = false;
                        }
                    } else{
                        state.screen = Screen::Accounts;
                    }                }
                Task::none()
            }
            Message::CopyToClipboard(content) => clipboard::write(content),
            Message::CopyLogs => clipboard::write(state.logs.join("\n")),
            Message::CurrentAccountChanged(account_name) => {
                for i in &state.accounts {
                    if i.username == account_name {
                        state.current_account = i.clone();
                        persist_current_account(&state.current_account);
                        break;
                    }
                }
                Task::none()
            }
            Message::RefreshLogin(mc_account) => {
                if let Some(mc_account) = mc_account{
                    state.current_account_mc_data = mc_account;
                } else{
                    state.current_account_mc_data.username = state.current_account.username.clone();
                    state.game_state_text_2 = String::from("Game will run in offline mode. Check your internet connection.");
                }
                state.launch();
                Task::none()
            }
            Message::LocalAccountNameChanged(mut username) => {
                if username.chars().count() <= 16{
                    for (i, char) in username.clone().chars().enumerate(){
                        if !char.is_alphanumeric() && char != '_' {
                            username.remove(i);
                        }
                    }
                    state.local_account_to_add_name = username.replace(" ", "");
                }
                Task::none()
            }
            Message::AddedLocalAccount => {
                if state.local_account_to_add_name.chars().count() >= 3
                    && state.local_account_to_add_name.chars().count() <= 16
                {
                    let account = Account {
                        microsoft: false,
                        username: state.local_account_to_add_name.clone(),
                        refresh_token: String::new(),
                    };
                    state.accounts = save_account(account.clone());
                    state.current_account = account;
                    persist_current_account(&state.current_account);
                    if state.is_first_launcher_use{
                        if state.all_versions.is_empty(){
                            state.screen = Screen::GettingStarted2;
                        } else{
                            state.screen = Screen::Main;
                            state.is_first_launcher_use = false;
                        }
                    } else{
                        state.screen = Screen::Accounts;
                    }
                    state.local_account_to_add_name = String::new();
                }
                Task::none()
            }
            Message::RemoveAccount(account_name) => {
                let mut config_file = getjson(get_config_file_path());
                let mut updated_account_list = vec![];
                if let Some(arr) = config_file["accounts"].as_array() {
                    for account in arr {
                        if account["username"].as_str().unwrap() != &account_name {
                            let microsoft = account["microsoft"].as_bool().unwrap();
                            let username = account["username"].as_str().unwrap().to_owned();
                            let refresh_token =
                                account["refresh_token"].as_str().unwrap().to_owned();
                            updated_account_list.push(Account {
                                microsoft,
                                username,
                                refresh_token,
                            })
                        }
                    }
                }
                // If the deleted account was the current one, fall back to
                // the first remaining account (or empty if none left).
                if state.current_account.username == account_name {
                    state.current_account = updated_account_list
                        .first()
                        .cloned()
                        .unwrap_or_default();
                }
                config_file["accounts"] = serde_json::json!(updated_account_list);
                config_file["current_account"] = serde_json::json!(state.current_account);
                let serialized = serde_json::to_string_pretty(&config_file).unwrap();
                let mut file = OpenOptions::new()
                    .write(true)
                    .truncate(true)
                    .open(get_config_file_path())
                    .unwrap();
                file.write_all(serialized.as_bytes()).unwrap();
                state.accounts = updated_account_list;
                Task::none()
            }
        }
    }
    fn view(state: &DgrLauncher) -> Element<'_, Message> {
        let sidebar = container(
            column![
                action(
                    button(svg(svg::Handle::from_memory(
                        include_bytes!("icons/home.svg").as_slice()
                    )))
                    .on_press(Message::ChangeScreen(Screen::Main))
                    .style(theme::transparent_button)
                    .width(Length::Fixed(42.))
                    .height(Length::Fixed(42.)),
                    "Main Screen"
                ),
                action(
                    button(svg(svg::Handle::from_memory(
                        include_bytes!("icons/settings.svg").as_slice()
                    )))
                    .on_press(Message::ChangeScreen(Screen::Settings))
                    .style(theme::transparent_button)
                    .width(Length::Fixed(42.))
                    .height(Length::Fixed(42.)),
                    "Settings"
                ),
                action(
                    button(svg(svg::Handle::from_memory(
                        include_bytes!("icons/download.svg").as_slice()
                    )))
                    .on_press(Message::ChangeScreen(Screen::Installation))
                    .style(theme::transparent_button)
                    .width(Length::Fixed(42.))
                    .height(Length::Fixed(42.)),
                    "Installer"
                ),
                action(
                    button(svg(svg::Handle::from_memory(
                        include_bytes!("icons/account.svg").as_slice()
                    )))
                    .on_press(Message::ChangeScreen(Screen::Accounts))
                    .style(theme::transparent_button)
                    .width(Length::Fixed(42.))
                    .height(Length::Fixed(42.)),
                    "Account (WIP)"
                ),
                action(
                    button(svg(svg::Handle::from_memory(
                        include_bytes!("icons/info.svg").as_slice()
                    )))
                    .on_press(Message::ChangeScreen(Screen::InfoAndUpdates))
                    .style(theme::transparent_button)
                    .width(Length::Fixed(42.))
                    .height(Length::Fixed(42.)),
                    "Info and updates"
                )
            ]
            .spacing(20)
            .align_x(Alignment::Center),
        )
        .style(theme::black_container)
        .align_x(alignment::Horizontal::Center)
        .align_y(alignment::Vertical::Center)
        .width(50)
        .height(Length::Fixed(400.));
        let screen = screens::get_screen_content(state);
        match state.is_first_launcher_use{
            true =>      container(screen.height(Length::Fixed(400.))
        )
            .width(Length::Fill)
            .height(Length::Fill)
            .align_y(alignment::Vertical::Center)
            .padding(15)
            .into(),
            false =>      container(row![sidebar, screen].spacing(65))
            .width(Length::Fill)
            .height(Length::Fill)
            .align_y(alignment::Vertical::Center)
            .padding(15)
            .into(),
        }
    }
    fn subscription(state: &DgrLauncher) -> Subscription<Message> {
        let mut subscriptions = Vec::new();
        for i in &state.downloaders {
            subscriptions.push(i.subscription())
        }
        subscriptions.push(state.launcher.subscription());
        let events = listen_with(|event, _status, _window| match event {
            iced::Event::Window(window::Event::CloseRequested) => Some(Message::Exit),
            _ => None,
        });
        subscriptions.push(events);
        if !state.auth_code.code.is_empty() {
            let auth_sub = auth::start_wait_for_login(0, state.auth_code.device_code.clone())
                .map(Message::ManageAuth);
            subscriptions.push(auth_sub)
        };
        Subscription::batch(subscriptions)
    }
fn action<'a>(
    widget: Button<'a, Message, Theme, Renderer>,
    tp_text: &'a str,
) -> Element<'a, Message> {
    tooltip(widget, tp_text, tooltip::Position::Right)
        .style(theme::black_container)
        .padding(10)
        .into()
}
fn checksettingsfile() -> bool {
    let file_exists = Path::new(&get_config_file_path()).exists();
    let mut conf_json = match file_exists {
        true => getjson(get_config_file_path()),
        false => serde_json::json!({}),
    };
    let mut file = File::create(get_config_file_path()).unwrap();
    if let Value::Object(map) = &mut conf_json {
        if !map.contains_key("custom_java_path") {
            map.insert(
                "custom_java_path".to_owned(),
                serde_json::to_value(String::new()).unwrap(),
            );
        }
        if !map.contains_key("custom_java_flags") {
            map.insert(
                "custom_java_flags".to_owned(),
                serde_json::to_value(String::new()).unwrap(),
            );
        }
        if !map.contains_key("accounts") {
            let accounts: Vec<Account> = vec![];
            map.insert(
                "accounts".to_owned(),
                serde_json::to_value(accounts).unwrap(),
            );
        }
        if !map.contains_key("current_account") {
            map.insert(
                "current_account".to_owned(),
                serde_json::json!(Account {
                    microsoft: false,
                    username: String::new(),
                    refresh_token: String::new()
                }),
            );
        }
        if !map.contains_key("current_version") {
            map.insert(
                "current_version".to_owned(),
                serde_json::to_value(String::new()).unwrap(),
            );
        }
        if !map.contains_key("game_ram") {
            map.insert("game_ram".to_owned(), serde_json::to_value(2.5).unwrap());
        }
        if !map.contains_key("current_java_name") {
            map.insert(
                "current_java_name".to_owned(),
                serde_json::to_value(String::from("Automatic")).unwrap(),
            );
        }
        if !map.contains_key("game_enviroment_variables") {
            map.insert(
                "game_enviroment_variables".to_owned(),
                serde_json::to_value(String::new()).unwrap(),
            );
        }
        if !map.contains_key("game_wrapper_commands") {
            map.insert(
                "game_wrapper_commands".to_owned(),
                serde_json::to_value(String::new()).unwrap(),
            );
        }
        if !map.contains_key("show_all_versions") {
            map.insert(
                "show_all_versions".to_owned(),
                serde_json::to_value(false).unwrap(),
            );
        }
    }
    let serializedjson = serde_json::to_string_pretty(&conf_json).unwrap();
    file.write_all(serializedjson.as_bytes()).unwrap();
    !file_exists
}
fn updateusersettingsfile(current_account: Account, version: String) -> std::io::Result<()> {
    set_current_dir(env::current_exe().unwrap().parent().unwrap()).unwrap();
    let mut file = File::open(get_config_file_path())?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    let mut data: Value = serde_json::from_str(&contents)?;
    data["current_account"] = serde_json::json!(current_account);
    data["current_version"] = serde_json::Value::String(version);
    let serialized = serde_json::to_string_pretty(&data)?;
    let mut file = OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(get_config_file_path())?;
    file.write_all(serialized.as_bytes())?;
    Ok(())
}
fn persist_current_account(current_account: &Account) {
    set_current_dir(env::current_exe().unwrap().parent().unwrap()).unwrap();
    let mut file = match File::open(get_config_file_path()) {
        Ok(f) => f,
        Err(e) => {
            println!("Failed to persist current account: {e}");
            return;
        }
    };
    let mut contents = String::new();
    if file.read_to_string(&mut contents).is_err() {
        println!("Failed to persist current account: cannot read config");
        return;
    }
    let mut data: Value = match serde_json::from_str(&contents) {
        Ok(v) => v,
        Err(e) => {
            println!("Failed to persist current account: {e}");
            return;
        }
    };
    data["current_account"] = serde_json::json!(current_account);
    let serialized = serde_json::to_string_pretty(&data).unwrap();
    let mut file = match OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(get_config_file_path())
    {
        Ok(f) => f,
        Err(e) => {
            println!("Failed to persist current account: {e}");
            return;
        }
    };
    if let Err(e) = file.write_all(serialized.as_bytes()) {
        println!("Failed to persist current account: {e}");
    }
}
/// Human-readable description of an installed version, e.g.
/// `Minecraft 1.21.1 • NeoForge 21.1.250`, so modded entries show
/// which Minecraft version they are for.
fn describe_version(id: &str) -> String {
    if id.is_empty() {
        return String::new();
    }
    let path = format!(
        "{}/versions/{}/{}.json",
        launcher::get_minecraft_dir(),
        id,
        id
    );
    let content = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return String::new(),
    };
    let v: Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return String::new(),
    };
    if let Some(mc) = v["inheritsFrom"].as_str() {
        // Check "neoforge" before "forge": the former contains the latter.
        let loader = if id.contains("neoforge") {
            format!("NeoForge {}", id.trim_start_matches("neoforge-"))
        } else if id.contains("fabric") {
            "Fabric".to_string()
        } else if id.contains("forge") {
            "Forge".to_string()
        } else {
            "Modded".to_string()
        };
        format!("Minecraft {mc} \u{2022} {loader}")
    } else {
        format!("Minecraft {id}")
    }
}
/// Keep the NeoForge version list in sync with the chosen MC version.
/// The cached full list is latest-first, so the first match is preselected.
fn filter_neoforge_for_mc(state: &mut DgrLauncher) {
    state.neoforge_versions_for_mc = state
        .neoforge_all
        .iter()
        .filter(|(mc, _)| *mc == state.install_mc_version)
        .map(|(_, nf)| nf.clone())
        .collect();
    state.neoforge_selected = state
        .neoforge_versions_for_mc
        .first()
        .cloned()
        .unwrap_or_default();
    state.neoforge_manual = String::new();
    if state.neoforge_versions_for_mc.is_empty() && !state.install_mc_version.is_empty() {
        state.neoforge_status = format!(
            "No NeoForge found for {}. Enter the version manually.",
            state.install_mc_version
        );
    } else if !state.neoforge_all.is_empty() {
        state.neoforge_status = String::new();
    }
}
/// Only these Java selections exist now; old configs (e.g. "Java 8
/// (DgrLauncher)" or removed custom names) fall back to Automatic.
fn sanitize_java_name(name: &str) -> String {
    match name {
        "Automatic" | "System Java" | "Custom" => name.to_owned(),
        _ => String::from("Automatic"),
    }
}
fn persist_config_keys(keys: &[(&str, String)]) {
    set_current_dir(env::current_exe().unwrap().parent().unwrap()).unwrap();
    let mut file = match File::open(get_config_file_path()) {
        Ok(f) => f,
        Err(e) => {
            println!("Failed to persist java settings: {e}");
            return;
        }
    };
    let mut contents = String::new();
    if file.read_to_string(&mut contents).is_err() {
        println!("Failed to persist java settings: cannot read config");
        return;
    }
    let mut data: Value = match serde_json::from_str(&contents) {
        Ok(v) => v,
        Err(e) => {
            println!("Failed to persist java settings: {e}");
            return;
        }
    };
    for (key, value) in keys {
        data[*key] = serde_json::Value::String(value.clone());
    }
    let serialized = serde_json::to_string_pretty(&data).unwrap();
    let mut file = match OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(get_config_file_path())
    {
        Ok(f) => f,
        Err(e) => {
            println!("Failed to persist java settings: {e}");
            return;
        }
    };
    if let Err(e) = file.write_all(serialized.as_bytes()) {
        println!("Failed to persist java settings: {e}");
    }
}
fn persist_current_java_name(name: &str) {
    persist_config_keys(&[("current_java_name", name.to_owned())]);
}
fn persist_custom_java(path: &str, flags: &str) {
    persist_config_keys(&[
        ("custom_java_path", path.to_owned()),
        ("custom_java_flags", flags.to_owned()),
    ]);
}
fn save_account(account: Account) -> Vec<Account> {
    set_current_dir(env::current_exe().unwrap().parent().unwrap()).unwrap();
    let mut file = File::open(get_config_file_path()).unwrap();
    let mut contents = String::new();
    file.read_to_string(&mut contents).unwrap();
    let mut data: Value = serde_json::from_str(&contents).unwrap();
    if let Value::Array(arr) = &mut data["accounts"] {
        arr.push(serde_json::json!(account));
        data["accounts"] = serde_json::json!(arr);
    }
    let mut updated_account_list = vec![];
    if let Some(arr) = data["accounts"].as_array() {
        for account in arr {
            let microsoft = account["microsoft"].as_bool().unwrap();
            let username = account["username"].as_str().unwrap().to_owned();
            let refresh_token = account["refresh_token"].as_str().unwrap().to_owned();
            updated_account_list.push(Account {
                microsoft,
                username,
                refresh_token,
            })
        }
    }
    let serialized = serde_json::to_string_pretty(&data).unwrap();
    let mut file = OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(get_config_file_path())
        .unwrap();
    file.write_all(serialized.as_bytes()).unwrap();
    updated_account_list
}
fn updatesettingsfile(
    ram: f64,
    currentjvm: String,
    wrapper_commands: String,
    env_variables: String,
    showallversions: bool,
) -> std::io::Result<()> {
    set_current_dir(env::current_exe().unwrap().parent().unwrap()).unwrap();
    let mut file = File::open(get_config_file_path())?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    let mut data: Value = serde_json::from_str(&contents)?;
    data["game_ram"] = serde_json::Value::Number(Number::from_f64(ram).unwrap());
    data["current_java_name"] = serde_json::Value::String(currentjvm);
    data["game_wrapper_commands"] = serde_json::Value::String(wrapper_commands);
    data["show_all_versions"] = serde_json::Value::Bool(showallversions);
    data["game_enviroment_variables"] = serde_json::Value::String(env_variables);
    let serialized = serde_json::to_string_pretty(&data)?;
    let mut file = OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(get_config_file_path())?;
    file.write_all(serialized.as_bytes())?;
    Ok(())
}
#[derive(Debug)]
struct Launcher {
    state: LauncherState,
}
#[derive(Debug, PartialEq)]
enum LauncherState {
    Idle,
    Waiting,
    Launching(Box<launcher::GameSettings>),
    // Carries the same settings so the launcher subscription recipe
    // (0, Some(settings)) stays identical and iced keeps the running stream
    // (which owns the log receiver) alive instead of restarting it.
    GettingLogs(Box<launcher::GameSettings>),
}
impl Default for Launcher {
    fn default() -> Self {
        Launcher {
            state: LauncherState::Idle,
        }
    }
}
impl Launcher {
    pub fn start(&mut self, game_settings: launcher::GameSettings) {
        self.state = LauncherState::Launching(Box::new(game_settings))
    }
    pub fn subscription(&self) -> Subscription<Message> {
        match &self.state {
            LauncherState::Idle => Subscription::none(),
            LauncherState::Launching(game_settings) => {
                launcher::start(0, Some(game_settings)).map(Message::ManageGameInfo)
            }
            LauncherState::GettingLogs(game_settings) => {
                launcher::start(0, Some(game_settings)).map(Message::ManageGameInfo)
            }
            LauncherState::Waiting => Subscription::none(),
        }
    }
}
struct Downloader {
    state: DownloaderState,
    id: usize,
}
enum DownloaderState {
    Idle,
    Downloading(String, downloader::VersionType),
    JavaDownloading(downloader::Java),
    DownloadingMissingFiles(Vec<downloader::Download>),
    Update(String),
    NeoForgeInstaller { mc_version: String, nf_version: String },
}
impl Default for Downloader {
    fn default() -> Self {
        Downloader {
            state: DownloaderState::Idle,
            id: 0,
        }
    }
}
impl Downloader {
    pub fn new(id: usize) -> Self {
        Downloader {
            state: DownloaderState::Idle,
            id,
        }
    }
    pub fn start(&mut self, version: String, version_type: downloader::VersionType) {
        self.state = DownloaderState::Downloading(version, version_type)
    }
    pub fn start_java(&mut self, java: downloader::Java) {
        self.state = DownloaderState::JavaDownloading(java)
    }
    pub fn start_update(&mut self, url: String) {
        self.state = DownloaderState::Update(url)
    }
    pub fn start_missing_files(&mut self, files: Vec<downloader::Download>) {
        self.state = DownloaderState::DownloadingMissingFiles(files)
    }
    pub fn start_neoforge(&mut self, mc_version: String, nf_version: String) {
        self.state = DownloaderState::NeoForgeInstaller {
            mc_version,
            nf_version,
        }
    }
    pub fn subscription(&self) -> Subscription<Message> {
        match &self.state {
            DownloaderState::Idle => Subscription::none(),
            DownloaderState::Downloading(version, version_type) => {
                downloader::start(self.id, version.to_string(), version_type.clone())
                    .map(Message::ManageDownload)
            }
            DownloaderState::JavaDownloading(java) => {
                downloader::start_java(self.id, *java).map(Message::ManageDownload)
            }
            DownloaderState::DownloadingMissingFiles(files) => {
                downloader::start_missing_files(self.id, files.clone())
                    .map(Message::ManageDownload)
            }
            DownloaderState::Update(url) => {
                downloader::start_update(self.id, url.to_string()).map(Message::ManageDownload)
            }
            DownloaderState::NeoForgeInstaller {
                mc_version,
                nf_version,
            } => downloader::start_neoforge(self.id, mc_version.clone(), nf_version.clone())
                .map(Message::ManageDownload),
        }
    }
}
mod widget {
    use crate::theme::Theme;
    pub type Renderer = iced::Renderer;
    pub type Element<'a, Message> = iced::Element<'a, Message, Theme, Renderer>;
}
#[derive(Default, Serialize, Deserialize)]
struct Java {
    name: String,
    path: String,
    flags: String,
}
fn getjson(jpathstring: String) -> Value {
    let jsonpath = Path::new(&jpathstring);
    let mut file = File::open(jsonpath).unwrap();
    let mut fcontent = String::new();
    file.read_to_string(&mut fcontent).unwrap();
    serde_json::from_str(&fcontent).unwrap()
}
fn get_config_file_path() -> String {
    #[cfg(debug_assertions)]
    return format!(
        "{}/dgrlauncher_settings_debug.json",
        launcher::get_minecraft_dir()
    );
    #[cfg(not(debug_assertions))]
    return format!(
        "{}/dgrlauncher_settings.json",
        launcher::get_minecraft_dir()
    );
}
/// Folder name for the isolated game data of a version.
/// Version names from Mojang are filesystem-safe, but guard against
/// path separators just in case.
fn sanitize_instance_name(version: &str) -> String {
    let sanitized: String = version
        .chars()
        .map(|c| {
            if c == '/' || c == '\\' || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect();
    let trimmed = sanitized.trim();
    if trimmed.is_empty() || trimmed == "." || trimmed == ".." {
        String::from("unknown")
    } else {
        trimmed.to_owned()
    }
}
/// Isolated game directory for a version:
/// `{minecraft_dir}/dgrlauncher_instances/<version>`.
/// Shared files (versions, libraries, assets, java) stay in `.minecraft`.
fn game_instance_dir_for_version(version: &str) -> String {
    format!(
        "{}/dgrlauncher_instances/{}",
        launcher::get_minecraft_dir(),
        sanitize_instance_name(version)
    )
}
fn is_file_empty(file_path: &str) -> bool {
    let mut file = File::open(file_path).unwrap();
    let mut buffer = [0; 1];
    match file.read(&mut buffer).unwrap() {
        0 => true,
        _ => false,
    }
}
fn migrate_path(old: &str, new: &str, label: &str) {
    if !Path::new(new).exists() && Path::new(old).exists() {
        match fs::rename(old, new) {
            Ok(_) => println!("Migrated {label} to new name."),
            Err(e) => println!("Failed to migrate {label}: {e}"),
        }
    }
}
fn backward_compatibility_measures() {
    let mc_dir = get_minecraft_dir();
    let new_game_instances_path = format!("{}/dgrlauncher_instances", mc_dir);
    let new_java_path = format!("{}/dgrlauncher_java", mc_dir);
    let new_settings_path = format!("{}/dgrlauncher_settings.json", mc_dir);
    let new_settings_debug_path = format!("{}/dgrlauncher_settings_debug.json", mc_dir);
    migrate_path(
        &format!("{}/minelander_instances", mc_dir),
        &new_game_instances_path,
        "game instances folder",
    );
    migrate_path(
        &format!("{}/minelander_java", mc_dir),
        &new_java_path,
        "java folder",
    );
    migrate_path(
        &format!("{}/minelander_settings.json", mc_dir),
        &new_settings_path,
        "settings file",
    );
    migrate_path(
        &format!("{}/minelander_settings_debug.json", mc_dir),
        &new_settings_debug_path,
        "debug settings file",
    );
    let ancient_profiles_path = format!("{}/minelander_profiles", mc_dir);
    if Path::new(&ancient_profiles_path).is_dir() {
        migrate_path(
            &ancient_profiles_path,
            &new_game_instances_path,
            "legacy minelander_profiles folder",
        );
    }
}
