#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use self::widget::Element;
use iced::{
    alignment, clipboard,
    event::listen_with,
    widget::{button, column, container, row, space, svg, tooltip, Button},
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
mod modrinth;
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
            min_size: Some(iced::Size {
                width: 700.,
                height: 400.,
            }),
            resizable: true,
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
    /// Instance id with an armed delete confirmation (first click done,
    /// second click deletes). Cleared when the selection changes.
    delete_confirm: Option<String>,
    /// Mod store state (slice 1: browse + search + read-only page).
    modstore_query: String,
    modstore_results: Vec<modrinth::ModSummary>,
    modstore_status: String,
    modstore_detail: Option<modrinth::ModDetail>,
    modstore_versions: Vec<modrinth::ModVersion>,
    /// Downloaded icons by URL; `None` = failed, shows cube, no refetch.
    modstore_icons: HashMap<String, Option<iced::widget::image::Handle>>,
    /// Store tab (catalog vs installed) and page download flag.
    modstore_tab: ModStoreTab,
    modstore_downloading: bool,
    /// Version row under the cursor (hover reveals its Download button).
    modstore_hovered_version: Option<String>,
    /// Hand-dropped jars without a sidecar entry + linking flag.
    modstore_unlinked: Vec<String>,
    modstore_linking: bool,
    /// `project_id -> newest compatible version` newer than installed.
    modstore_updates: HashMap<String, modrinth::ModVersion>,
    /// Inline update confirm (`project_id`) in the installed list.
    modstore_update_confirm: Option<String>,
    /// Auto/manual update check bookkeeping.
    modstore_pending_checks: u32,
    modstore_check_manual: bool,
    /// Mod page history for Back (dependency hopping).
    modstore_history: Vec<String>,
    /// Dependency section state on the mod page.
    modstore_deps_expanded: bool,
    modstore_dep_titles: HashMap<String, (String, String)>,
    /// Which project the loaded `modstore_versions` belong to. The list
    /// survives navigation, so the update-entry refresh must never use a
    /// stale list of ANOTHER mod (it would offer A's version to B).
    modstore_versions_project: Option<String>,
    /// Installed mods of the current instance (`project_id -> entry`).
    modstore_installed: HashMap<String, modrinth::InstalledMod>,
    /// Armed mod delete confirmation (`project_id`), same two-click pattern
    /// as instance delete.
    modstore_delete_confirm: Option<String>,
    /// Saved catalog scroll offset (restored on Back from the mod page).
    modstore_scroll: f32,
    /// Live-search debounce generation (stale delayed tasks are ignored).
    modstore_search_seq: u32,
    /// Instance settings screen state.
    instance_name_edit: String,
    instance_ram_text: String,
    instance_settings_status: String,
    /// Loader switch state (fabric loaders or NeoForge versions for the base
    /// MC, selected/manual input, status). `pending_loader_switch` moves the
    /// selection to the new NeoForge instance id when it finishes.
    il_loader_list: Vec<String>,
    il_selected: String,
    il_manual: String,
    il_status: String,
    pending_loader_switch: Option<String>,
    /// Optional custom instance name typed on the Installation screen.
    install_name: String,
    /// Global RAM manual input mirror + validation hint (Settings).
    settings_ram_text: String,
    settings_status: String,
    /// `(default version id, custom name)` renames applied when each
    /// install finishes (validated at press, executed at finish).
    pending_install_names: Vec<(String, String)>,
    /// Status notice generation: terminal notices auto-clear after 5s.
    notice_seq: u32,
}
#[derive(Default, Serialize, Deserialize, Clone)]
struct Account {
    microsoft: bool,
    username: String,
    refresh_token: String,
}
/// Per-instance overrides, stored in
/// `{instance}/dgrlauncher_instance.json`. `None` = use the global setting.
#[derive(Default, Serialize, Deserialize, Clone, Debug)]
struct InstanceConfig {
    /// "cube" (default) | "star" | "heart" | "letter".
    icon: Option<String>,
    /// GiB override, `None` = global `game_ram`.
    ram: Option<f64>,
    /// "Automatic" | "System Java" | "Custom", `None` = global.
    java: Option<String>,
}
fn instance_config_path(version: &str) -> String {
    format!(
        "{}/dgrlauncher_instance.json",
        game_instance_dir_for_version(version)
    )
}
fn read_instance_config(version: &str) -> InstanceConfig {
    std::fs::read_to_string(instance_config_path(version))
        .ok()
        .and_then(|c| serde_json::from_str(&c).ok())
        .unwrap_or_default()
}
fn write_instance_config(version: &str, cfg: &InstanceConfig) -> Result<(), String> {
    let dir = game_instance_dir_for_version(version);
    std::fs::create_dir_all(&dir).map_err(|e| format!("Instance folder failed: {e}"))?;
    let text =
        serde_json::to_string_pretty(cfg).map_err(|e| format!("Encode failed: {e}"))?;
    std::fs::write(instance_config_path(version), text)
        .map_err(|e| format!("Write failed: {e}"))
}
/// Base Minecraft version of an installed instance: `inheritsFrom` from
/// its json, or the id itself for vanilla.
fn instance_base_mc(version_id: &str) -> String {
    let json_path = format!(
        "{}/versions/{}/{}.json",
        launcher::get_minecraft_dir(),
        version_id,
        version_id
    );
    std::fs::read_to_string(&json_path)
        .ok()
        .and_then(|c| serde_json::from_str::<Value>(&c).ok())
        .and_then(|v| v["inheritsFrom"].as_str().map(str::to_owned))
        .unwrap_or_else(|| version_id.to_owned())
}
/// Icon preset ids used by the instance settings ("" = default cube).
fn instance_icon_presets() -> Vec<String> {
    vec![
        String::new(),
        String::from("star"),
        String::from("heart"),
        String::from("letter"),
    ]
}

/// Mod store tab: catalog from Modrinth vs locally installed mods.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ModStoreTab {
    #[default]
    Store,
    Installed,
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
    Accounts,
    MicrosoftAccount,
    LocalAccount,
    InstanceSettings,
    ModStore,
    ModPage,
}
#[derive(Debug, Clone)]
enum Message {
    LoadVersionList(Vec<String>),
    Launch,
    CloseGame,
    ManageGameInfo((usize, launcher::Progress)),
    CurrentAccountChanged(String),
    VersionChanged(String),
    DeleteInstancePressed(String),
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
    OpenModStore,
    ModStoreQueryChanged(String),
    ModStoreSearch,
    GotModSearch(Result<Vec<modrinth::ModSummary>, String>),
    OpenModPage(String),
    GotModDetail(Result<(modrinth::ModDetail, Vec<modrinth::ModVersion>), String>),
    ModIconLoaded(String, Option<Vec<u8>>),
    ModStoreTabChanged(ModStoreTab),
    ModVersionHovered(String),
    ModVersionUnhovered(String),
    ModInstallVersion(String),
    ModInstallFinished(Result<modrinth::InstalledMod, String>),
    ModDeletePressed(String),
    ModStoreScrolled(f32),
    ModStoreSearchDebounced(u32, String),
    ModCheckUpdates,
    ModUpdateChecked(String, Option<modrinth::ModVersion>),
    ModUpdatePressed(String),
    ModUpdateApply(String),
    ModPageBack,
    ModDepsToggled,
    GotModDepTitles(HashMap<String, (String, String)>),
    ModLinkFinished(Result<usize, String>),
    ModLocalFileDelete(String),
    OpenInstanceSettings,
    InstanceNameChanged(String),
    InstanceRenamePressed,
    InstanceIconChanged(String),
    InstanceRamSlider(f64),
    InstanceRamText(String),
    InstanceRamReset,
    InstanceJavaChanged(String),
    GotInstanceFabricLoaders(Result<Vec<String>, String>),
    GotInstanceNeoForgeList(Result<Vec<(String, String)>, String>),
    InstallNameChanged(String),
    SettingsRamText(String),
    ClearNotices(u32),
    InstanceLoaderChanged(String),
    InstanceLoaderManualChanged(String),
    InstanceLoaderReload,
    InstanceLoaderApply,
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
        // Per-instance overrides (RAM, Java), falling back to globals.
        let cfg = read_instance_config(&self.current_version);
        let ram = cfg.ram.unwrap_or(self.game_ram);
        let eff_java_name = cfg
            .java
            .clone()
            .unwrap_or_else(|| self.current_java_name.clone());
        let java_type = match eff_java_name.as_str() {
            "System Java" => launcher::JavaType::System,
            "Custom" => launcher::JavaType::Custom,
            _ => launcher::JavaType::Automatic,
        };
        // Custom binary/flags live globally (Settings > Custom Java).
        let (jvm_path, jvm_flags) = if eff_java_name == "Custom" {
            if self.current_java_name == "Custom" {
                (
                    self.current_java.path.clone(),
                    self.current_java.flags.clone(),
                )
            } else {
                let config = getjson(get_config_file_path());
                (
                    config["custom_java_path"]
                        .as_str()
                        .unwrap_or("")
                        .to_owned(),
                    config["custom_java_flags"]
                        .as_str()
                        .unwrap_or("")
                        .to_owned(),
                )
            }
        } else {
            (String::new(), String::new())
        };
        let game_settings = launcher::GameSettings {
            account: self.current_account_mc_data.clone(),
            game_version: self.current_version.clone(),
            jvm: jvm_path,
            jvmargs: jvm_flags.split(' ').map(|s| s.to_owned()).collect(),
            ram,
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
        checksettingsfile();
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
        let current_version = p["current_version"].as_str().unwrap().to_owned();
        let current_version_info = describe_version(&current_version);
        (
            DgrLauncher {
                screen: Screen::Main,
                current_account: current_account,
                current_version,
                current_version_info,
                game_ram: p["game_ram"].as_f64().unwrap(),
                settings_ram_text: format!(
                    "{:.2}",
                    p["game_ram"].as_f64().unwrap()
                ),
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
    /// Reusable Modrinth page fetch: project + its versions.
    fn fetch_mod_page(id: String) -> Task<Message> {
        Task::perform(
            async move {
                let detail = modrinth::get_project(&id).await?;
                let versions = modrinth::get_versions(&detail.id).await?;
                Ok((detail, versions))
            },
            Message::GotModDetail,
        )
    }
    /// One update check per installed mod: the newest compatible version
    /// when it differs from the installed one. Fetch errors simply report
    /// "no update", never failing the batch.
    fn spawn_update_checks(
        installed: &HashMap<String, modrinth::InstalledMod>,
        loader: String,
        mc: String,
    ) -> Vec<Task<Message>> {
        let mut tasks = Vec::new();
        for e in installed.values() {
            let pid = e.project_id.clone();
            let installed_id = e.version_id.clone();
            let loader = loader.clone();
            let mc = mc.clone();
            tasks.push(Task::perform(
                async move {
                    let latest = modrinth::get_versions(&pid)
                        .await
                        .ok()
                        .and_then(|vs| {
                            modrinth::latest_compatible(&vs, &loader, &mc).cloned()
                        })
                        .filter(|v| v.id != installed_id);
                    (pid, latest)
                },
                |(pid, latest)| Message::ModUpdateChecked(pid, latest),
            ));
        }
        tasks
    }
    /// Auto-clear terminal status notices after 5s. Callers bump
    /// `notice_seq` first and batch this with their return task; a stale
    /// timer never clears a newer notice.
    fn clear_notices_later(seq: u32) -> Task<Message> {
        Task::perform(
            async move {
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                seq
            },
            Message::ClearNotices,
        )
    }
    /// Refresh the update entry after an install. Only the version list of
    /// the SAME project may be used: a stale page list of another mod must
    /// never leak into this project's entry (it would offer A's version
    /// to B with an icon that never disappears).
    fn refreshed_update_entry(
        versions_project: Option<&str>,
        versions: &[modrinth::ModVersion],
        loader: &str,
        mc: &str,
        entry: &modrinth::InstalledMod,
    ) -> Option<modrinth::ModVersion> {
        if versions_project != Some(entry.project_id.as_str()) {
            return None;
        }
        modrinth::latest_compatible(versions, loader, mc)
            .filter(|v| v.id != entry.version_id)
            .cloned()
    }
    /// Scroll the catalog back to the saved offset.
    fn restore_store_scroll(y: f32) -> Task<Message> {
        iced_runtime::task::widget(
            iced_core::widget::operation::scrollable::scroll_to(
                iced::widget::Id::new("modstore-list"),
                iced_core::widget::operation::scrollable::AbsoluteOffset {
                    x: None,
                    y: Some(y),
                },
            ),
        )
    }
    fn update(state: &mut DgrLauncher, message: Message) -> Task<Message> {
        match message {
            Message::Launch => {
                // Effective Java (per-instance override or global): a Custom
                // pick without a binary path cannot launch.
                let eff_java = read_instance_config(&state.current_version)
                    .java
                    .unwrap_or_else(|| state.current_java_name.clone());
                if eff_java == "Custom" {
                    let path = if state.current_java_name == "Custom" {
                        state.current_java.path.clone()
                    } else {
                        getjson(get_config_file_path())["custom_java_path"]
                            .as_str()
                            .unwrap_or("")
                            .to_owned()
                    };
                    if path.trim().is_empty() {
                        state.game_state_text = String::from(
                            "Set a custom Java path first (Settings > Custom Java).",
                        );
                        return Task::none();
                    }
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
                // Selecting another instance disarms delete confirmation.
                state.delete_confirm = None;
                Task::none()
            }
            Message::DeleteInstancePressed(id) => {
                if state.delete_confirm.as_deref() == Some(&id) {
                    // Second click: confirmed, delete version + instance dirs.
                    if !id.is_empty()
                        && !id.contains("..")
                        && !id.contains('/')
                        && !id.contains('\\')
                    {
                        let version_dir = format!(
                            "{}/versions/{}",
                            launcher::get_minecraft_dir(),
                            id
                        );
                        if let Err(e) = fs::remove_dir_all(&version_dir) {
                            println!("Failed to delete version dir: {e}");
                        }
                        let instance_dir = game_instance_dir_for_version(&id);
                        if let Err(e) = fs::remove_dir_all(&instance_dir) {
                            println!("Failed to delete instance dir: {e}");
                        }
                    }
                    state.delete_confirm = None;
                    if state.current_version == id {
                        state.current_version = String::new();
                        state.current_version_info = String::new();
                    }
                    return Task::perform(
                        launcher::getinstalledversions(),
                        Message::LoadVersionList,
                    );
                }
                // First click: arm confirmation, second click deletes.
                state.delete_confirm = Some(id);
                Task::none()
            }
            Message::OpenModStore => {
                // Store is only for modded instances (vanilla button disabled).
                if state.current_version.is_empty()
                    || modrinth::instance_loader(&state.current_version).is_none()
                {
                    return Task::none();
                }
                state.screen = Screen::ModStore;
                state.modstore_query = String::new();
                state.modstore_results = Vec::new();
                state.modstore_detail = None;
                state.modstore_versions = Vec::new();
                state.modstore_versions_project = None;
                state.modstore_tab = ModStoreTab::Store;
                state.modstore_downloading = false;
                state.modstore_delete_confirm = None;
                state.modstore_hovered_version = None;
                state.modstore_updates = HashMap::new();
                state.modstore_update_confirm = None;
                state.modstore_pending_checks = 0;
                state.modstore_check_manual = false;
                state.modstore_history = Vec::new();
                state.modstore_deps_expanded = false;
                state.modstore_unlinked = Vec::new();
                state.modstore_linking = false;
                state.modstore_scroll = 0.0;
                state.modstore_installed =
                    modrinth::installed_mods(&state.current_version);
                state.modstore_status = String::from("Loading popular mods...");
                // Auto update check for installed mods (manual button too).
                let (mc, loader) = modrinth::instance_loader(&state.current_version)
                    .unwrap_or_default();
                let mut tasks = vec![Task::perform(
                    modrinth::search_mods(""),
                    Message::GotModSearch,
                )];
                let checks =
                    spawn_update_checks(&state.modstore_installed, loader, mc);
                state.modstore_pending_checks = checks.len() as u32;
                tasks.extend(checks);
                return Task::batch(tasks);
            }
            Message::ModStoreQueryChanged(q) => {
                state.modstore_query = q.clone();
                state.modstore_search_seq = state.modstore_search_seq.wrapping_add(1);
                // Installed tab filters locally as you type; the catalog
                // searches live with a debounce (Enter still works instantly).
                if state.modstore_tab != ModStoreTab::Store {
                    return Task::none();
                }
                let seq = state.modstore_search_seq;
                return Task::perform(
                    async move {
                        tokio::time::sleep(std::time::Duration::from_millis(450)).await;
                        (seq, q)
                    },
                    |(seq, query)| Message::ModStoreSearchDebounced(seq, query),
                );
            }
            Message::ModStoreSearchDebounced(seq, query) => {
                // Stale delayed task (user kept typing) or left the tab.
                if seq != state.modstore_search_seq
                    || state.modstore_tab != ModStoreTab::Store
                    || query != state.modstore_query
                {
                    return Task::none();
                }
                state.modstore_results = Vec::new();
                state.modstore_status = String::from("Searching...");
                return Task::perform(
                    async move { modrinth::search_mods(&query).await },
                    Message::GotModSearch,
                );
            }
            Message::ModStoreSearch => {
                // API search always targets the catalog tab (installed
                // filters locally as you type, no request needed).
                state.modstore_tab = ModStoreTab::Store;
                // Invalidate pending debounced tasks.
                state.modstore_search_seq = state.modstore_search_seq.wrapping_add(1);
                let query = state.modstore_query.clone();
                state.modstore_results = Vec::new();
                state.modstore_status = String::from("Searching...");
                return Task::perform(
                    async move { modrinth::search_mods(&query).await },
                    Message::GotModSearch,
                );
            }
            Message::GotModSearch(result) => {
                match result {
                    Ok(list) => {
                        if list.is_empty() {
                            state.modstore_status =
                                String::from("Nothing found. Try another query.");
                        } else {
                            state.modstore_status = String::new();
                        }
                        state.modstore_results = list;
                    }
                    Err(err) => {
                        state.modstore_results = Vec::new();
                        state.modstore_status = err;
                        state.notice_seq = state.notice_seq.wrapping_add(1);
                        return clear_notices_later(state.notice_seq);
                    }
                }
                // Fetch missing icons in the background (cube fallback meanwhile).
                let mut tasks = Vec::new();
                for m in &state.modstore_results {
                    if m.icon_url.is_empty() || state.modstore_icons.contains_key(&m.icon_url)
                    {
                        continue;
                    }
                    let url = m.icon_url.clone();
                    let url2 = url.clone();
                    tasks.push(Task::perform(
                        async move { modrinth::fetch_icon(&url2).await },
                        move |bytes| Message::ModIconLoaded(url, bytes),
                    ));
                }
                Task::batch(tasks)
            }
            Message::OpenModPage(id) => {
                // Remember the return page when hopping through dependencies.
                if state.screen == Screen::ModPage {
                    if let Some(d) = &state.modstore_detail {
                        let back_to = d.id.clone();
                        if state.modstore_history.last() != Some(&back_to) {
                            state.modstore_history.push(back_to);
                            if state.modstore_history.len() > 20 {
                                state.modstore_history.remove(0);
                            }
                        }
                    }
                }
                state.screen = Screen::ModPage;
                state.modstore_detail = None;
                state.modstore_versions = Vec::new();
                state.modstore_versions_project = None;
                state.modstore_downloading = false;
                state.modstore_delete_confirm = None;
                state.modstore_hovered_version = None;
                state.modstore_deps_expanded = false;
                state.modstore_status = String::from("Loading mod page...");
                return fetch_mod_page(id);
            }
            Message::GotModDetail(result) => {
                match result {
                    Ok((detail, versions)) => {
                        state.modstore_status = String::new();
                        state.modstore_detail = Some(detail.clone());
                        state.modstore_versions = versions;
                        state.modstore_versions_project = Some(detail.id.clone());
                        let mut tasks = Vec::new();
                        if !detail.icon_url.is_empty()
                            && !state.modstore_icons.contains_key(&detail.icon_url)
                        {
                            let url = detail.icon_url.clone();
                            let url2 = url.clone();
                            tasks.push(Task::perform(
                                async move { modrinth::fetch_icon(&url2).await },
                                move |bytes| Message::ModIconLoaded(url, bytes),
                            ));
                        }
                        // Resolve dependency titles for the newest compatible
                        // version (cached across pages).
                        let (mc, loader) = modrinth::instance_loader(&state.current_version)
                            .unwrap_or_default();
                        if let Some(latest) = modrinth::latest_compatible(
                            &state.modstore_versions,
                            &loader,
                            &mc,
                        ) {
                            let mut seen = std::collections::HashSet::new();
                            let missing: Vec<String> = latest
                                .dependencies
                                .iter()
                                .filter_map(|d| d.project_id.clone())
                                .filter(|pid| {
                                    !state.modstore_dep_titles.contains_key(pid)
                                        && seen.insert(pid.clone())
                                })
                                .collect();
                            if !missing.is_empty() {
                                tasks.push(Task::perform(
                                    async move {
                                        modrinth::get_project_titles(&missing).await
                                    },
                                    Message::GotModDepTitles,
                                ));
                            }
                        }
                        return Task::batch(tasks);
                    }
                    Err(err) => {
                        state.modstore_status = err;
                        state.notice_seq = state.notice_seq.wrapping_add(1);
                        return clear_notices_later(state.notice_seq);
                    }
                }
            }
            Message::ModIconLoaded(url, bytes) => {
                let handle = bytes.map(iced::widget::image::Handle::from_bytes);
                state.modstore_icons.insert(url, handle);
                Task::none()
            }
            Message::ModStoreTabChanged(tab) => {
                state.modstore_tab = tab;
                state.modstore_delete_confirm = None;
                if tab == ModStoreTab::Installed {
                    state.modstore_installed =
                        modrinth::installed_mods(&state.current_version);
                    state.modstore_unlinked =
                        modrinth::unlinked_mod_files(&state.current_version);
                    let mut tasks = Vec::new();
                    // Identify hand-dropped jars by hash (auto-link).
                    if !state.modstore_unlinked.is_empty() && !state.modstore_linking
                    {
                        state.modstore_linking = true;
                        state.modstore_status =
                            String::from("Identifying hand-added files...");
                        let instance = state.current_version.clone();
                        tasks.push(Task::perform(
                            async move { modrinth::link_unlinked_mods(&instance).await },
                            Message::ModLinkFinished,
                        ));
                    }
                    // Fetch missing icons for installed rows (cube meanwhile).
                    for e in state.modstore_installed.values() {
                        if e.icon_url.is_empty()
                            || state.modstore_icons.contains_key(&e.icon_url)
                        {
                            continue;
                        }
                        let url = e.icon_url.clone();
                        let url2 = url.clone();
                        tasks.push(Task::perform(
                            async move { modrinth::fetch_icon(&url2).await },
                            move |bytes| Message::ModIconLoaded(url, bytes),
                        ));
                    }
                    return Task::batch(tasks);
                }
                Task::none()
            }
            Message::ModLinkFinished(result) => {
                state.modstore_linking = false;
                let mut armed = false;
                match result {
                    Ok(n) => {
                        state.modstore_installed =
                            modrinth::installed_mods(&state.current_version);
                        state.modstore_unlinked =
                            modrinth::unlinked_mod_files(&state.current_version);
                        if n > 0 {
                            state.modstore_status = format!(
                                "Linked {n} hand-added mod(s) ✓"
                            );
                            armed = true;
                        } else if state.modstore_status
                            == "Identifying hand-added files..."
                        {
                            state.modstore_status = String::new();
                        }
                    }
                    Err(err) => {
                        state.modstore_status = err;
                        armed = true;
                    }
                }
                if armed {
                    state.notice_seq = state.notice_seq.wrapping_add(1);
                    return clear_notices_later(state.notice_seq);
                }
                Task::none()
            }
            Message::ModLocalFileDelete(name) => {
                let key = format!("file:{name}");
                if state.modstore_delete_confirm.as_deref() == Some(&key) {
                    // Second click: delete the jar itself.
                    let safe_ok = !name.contains('/')
                        && !name.contains('\\')
                        && name.ends_with(".jar");
                    if safe_ok {
                        let path = format!(
                            "{}/{}",
                            modrinth::mods_dir(&state.current_version),
                            name
                        );
                        if let Err(e) = std::fs::remove_file(&path) {
                            state.modstore_status = format!("Delete failed: {e}");
                        } else {
                            state.modstore_status = String::from("File deleted.");
                        }
                    }
                    state.modstore_delete_confirm = None;
                    state.modstore_unlinked =
                        modrinth::unlinked_mod_files(&state.current_version);
                    state.notice_seq = state.notice_seq.wrapping_add(1);
                    return clear_notices_later(state.notice_seq);
                }
                state.modstore_delete_confirm = Some(key);
                Task::none()
            }
            Message::ModVersionHovered(id) => {
                state.modstore_hovered_version = Some(id);
                Task::none()
            }
            Message::ModVersionUnhovered(id) => {
                if state.modstore_hovered_version.as_deref() == Some(&id) {
                    state.modstore_hovered_version = None;
                }
                Task::none()
            }
            Message::ModInstallVersion(version_id) => {
                let detail = match &state.modstore_detail {
                    Some(d) => d.clone(),
                    None => return Task::none(),
                };
                let version = match state
                    .modstore_versions
                    .iter()
                    .find(|v| v.id == version_id)
                {
                    Some(v) => v.clone(),
                    None => return Task::none(),
                };
                if state.modstore_downloading || state.current_version.is_empty() {
                    return Task::none();
                }
                let instance = state.current_version.clone();
                state.modstore_downloading = true;
                state.modstore_status = format!("Downloading {}...", version.filename);
                return Task::perform(
                    async move { modrinth::install_mod(&instance, &detail, &version).await },
                    Message::ModInstallFinished,
                );
            }
            Message::ModInstallFinished(result) => {
                state.modstore_downloading = false;
                state.notice_seq = state.notice_seq.wrapping_add(1);
                let notice = clear_notices_later(state.notice_seq);
                match result {
                    Ok(entry) => {
                        state.modstore_status =
                            format!("Installed {} ✓", entry.version_number);
                        state.modstore_installed =
                            modrinth::installed_mods(&state.current_version);
                        if state.modstore_update_confirm.as_deref()
                            == Some(&entry.project_id)
                        {
                            state.modstore_update_confirm = None;
                        }
                        // Refresh the update entry, but ONLY from the version
                        // list of this same project (see helper): a stale
                        // list of another mod must never leak in. Otherwise
                        // drop it, the next auto/manual check rebuilds it.
                        let (mc, loader) =
                            modrinth::instance_loader(&state.current_version)
                                .unwrap_or_default();
                        match refreshed_update_entry(
                            state.modstore_versions_project.as_deref(),
                            &state.modstore_versions,
                            &loader,
                            &mc,
                            &entry,
                        ) {
                            Some(latest) => {
                                state
                                    .modstore_updates
                                    .insert(entry.project_id.clone(), latest);
                            }
                            None => {
                                state.modstore_updates.remove(&entry.project_id);
                            }
                        }
                    }
                    Err(err) => {
                        state.modstore_status = err;
                    }
                }
                return notice;
            }
            Message::ModDeletePressed(project_id) => {
                if state.modstore_delete_confirm.as_deref() == Some(&project_id) {
                    // Second click: confirmed.
                    if let Err(e) =
                        modrinth::delete_mod(&state.current_version, &project_id)
                    {
                        state.modstore_status = e;
                    } else {
                        state.modstore_status = String::from("Mod deleted.");
                    }
                    state.modstore_delete_confirm = None;
                    state.modstore_updates.remove(&project_id);
                    if state.modstore_update_confirm.as_deref() == Some(&project_id) {
                        state.modstore_update_confirm = None;
                    }
                    state.modstore_installed =
                        modrinth::installed_mods(&state.current_version);
                    state.notice_seq = state.notice_seq.wrapping_add(1);
                    return clear_notices_later(state.notice_seq);
                }
                // First click: arm confirmation.
                state.modstore_delete_confirm = Some(project_id);
                Task::none()
            }
            Message::ModStoreScrolled(y) => {
                state.modstore_scroll = y;
                Task::none()
            }
            Message::ModCheckUpdates => {
                if state.current_version.is_empty() {
                    return Task::none();
                }
                state.modstore_installed =
                    modrinth::installed_mods(&state.current_version);
                let (mc, loader) = modrinth::instance_loader(&state.current_version)
                    .unwrap_or_default();
                let checks =
                    spawn_update_checks(&state.modstore_installed, loader, mc);
                state.modstore_pending_checks = checks.len() as u32;
                if checks.is_empty() {
                    state.modstore_check_manual = false;
                    state.modstore_status = if state.modstore_installed.is_empty() {
                        String::from("No mods installed.")
                    } else {
                        String::from("All mods up to date ✓")
                    };
                    state.notice_seq = state.notice_seq.wrapping_add(1);
                    return clear_notices_later(state.notice_seq);
                }
                state.modstore_check_manual = true;
                state.modstore_status = String::from("Checking updates...");
                return Task::batch(checks);
            }
            Message::ModUpdateChecked(pid, latest) => {
                match latest {
                    Some(v) => {
                        state.modstore_updates.insert(pid, v);
                    }
                    None => {
                        state.modstore_updates.remove(&pid);
                    }
                }
                state.modstore_pending_checks =
                    state.modstore_pending_checks.saturating_sub(1);
                if state.modstore_pending_checks == 0 && state.modstore_check_manual {
                    state.modstore_check_manual = false;
                    state.modstore_status = if state.modstore_updates.is_empty() {
                        String::from("All mods up to date ✓")
                    } else {
                        format!("{} update(s) available", state.modstore_updates.len())
                    };
                    state.notice_seq = state.notice_seq.wrapping_add(1);
                    return clear_notices_later(state.notice_seq);
                }
                Task::none()
            }
            Message::ModUpdatePressed(pid) => {
                // Toggle the inline confirm, but only when update data exists
                // (otherwise there is nothing to confirm).
                if state.modstore_update_confirm.as_deref() == Some(&pid) {
                    state.modstore_update_confirm = None;
                } else if state.modstore_updates.contains_key(&pid) {
                    state.modstore_update_confirm = Some(pid);
                }
                Task::none()
            }
            Message::ModUpdateApply(pid) => {
                let version = match state.modstore_updates.get(&pid) {
                    Some(v) => v.clone(),
                    None => return Task::none(),
                };
                let entry = match state.modstore_installed.get(&pid) {
                    Some(e) => e.clone(),
                    None => return Task::none(),
                };
                if state.current_version.is_empty() {
                    return Task::none();
                }
                let detail = modrinth::ModDetail {
                    id: entry.project_id.clone(),
                    slug: entry.slug.clone(),
                    title: entry.title.clone(),
                    description: String::new(),
                    icon_url: entry.icon_url.clone(),
                    downloads: 0,
                };
                let instance = state.current_version.clone();
                state.modstore_downloading = true;
                state.modstore_update_confirm = None;
                state.modstore_status = format!("Updating {}...", entry.title);
                return Task::perform(
                    async move { modrinth::install_mod(&instance, &detail, &version).await },
                    Message::ModInstallFinished,
                );
            }
            Message::ModPageBack => {
                if let Some(prev) = state.modstore_history.pop() {
                    // Back through dependency pages without pushing again.
                    state.screen = Screen::ModPage;
                    state.modstore_detail = None;
                    state.modstore_versions = Vec::new();
                    state.modstore_versions_project = None;
                    state.modstore_downloading = false;
                    state.modstore_delete_confirm = None;
                    state.modstore_hovered_version = None;
                    state.modstore_deps_expanded = false;
                    state.modstore_status = String::from("Loading mod page...");
                    return fetch_mod_page(prev);
                }
                state.screen = Screen::ModStore;
                state.modstore_installed =
                    modrinth::installed_mods(&state.current_version);
                let y = state.modstore_scroll;
                return restore_store_scroll(y);
            }
            Message::ModDepsToggled => {
                state.modstore_deps_expanded = !state.modstore_deps_expanded;
                Task::none()
            }
            Message::GotModDepTitles(map) => {
                state.modstore_dep_titles.extend(map);
                Task::none()
            }
            Message::OpenInstanceSettings => {
                if state.current_version.is_empty() {
                    return Task::none();
                }
                state.screen = Screen::InstanceSettings;
                state.instance_name_edit = state.current_version.clone();
                let cfg = read_instance_config(&state.current_version);
                state.instance_ram_text =
                    format!("{:.2}", cfg.ram.unwrap_or(state.game_ram));
                state.instance_settings_status = String::new();
                state.il_status = String::new();
                state.il_manual = String::new();
                state.il_loader_list = Vec::new();
                state.il_selected = String::new();
                // Prefill loader switch data for modded instances.
                // Kind (not the folder name): names are user-chosen.
                if version_kind(&state.current_version) == VersionKind::Fabric {
                    let base = instance_base_mc(&state.current_version);
                    return Task::perform(
                        async move {
                            downloader::get_fabric_loader_versions(&base).await
                        },
                        Message::GotInstanceFabricLoaders,
                    );
                }
                if version_kind(&state.current_version) == VersionKind::NeoForge {
                    if state.neoforge_all.is_empty() {
                        return Task::perform(
                            downloader::get_neoforge_versions(),
                            Message::GotInstanceNeoForgeList,
                        );
                    }
                    let mc = instance_base_mc(&state.current_version);
                    state.il_loader_list = state
                        .neoforge_all
                        .iter()
                        .filter(|(m, _)| *m == mc)
                        .map(|(_, nf)| nf.clone())
                        .collect();
                    state.il_selected =
                        state.il_loader_list.first().cloned().unwrap_or_default();
                }
                Task::none()
            }
            Message::InstanceNameChanged(name) => {
                state.instance_name_edit = name;
                Task::none()
            }
            Message::InstanceRenamePressed => {
                let old = state.current_version.clone();
                let new = state.instance_name_edit.trim().to_owned();
                match rename_instance(&launcher::get_minecraft_dir(), &old, &new) {
                    Ok(()) => {
                        state.current_version = new.clone();
                        state.current_version_info = describe_version(&new);
                        state.delete_confirm = None;
                        state.instance_name_edit = new;
                        state.instance_settings_status = String::from("Renamed ✓");
                        state.notice_seq = state.notice_seq.wrapping_add(1);
                        return Task::batch(vec![
                            Task::perform(
                                launcher::getinstalledversions(),
                                Message::LoadVersionList,
                            ),
                            clear_notices_later(state.notice_seq),
                        ]);
                    }
                    Err(e) => {
                        state.instance_settings_status = e;
                        state.notice_seq = state.notice_seq.wrapping_add(1);
                        return clear_notices_later(state.notice_seq);
                    }
                }
            }
            Message::InstanceIconChanged(preset) => {
                let mut cfg = read_instance_config(&state.current_version);
                cfg.icon = if preset.is_empty() {
                    None
                } else {
                    Some(preset)
                };
                if let Err(e) = write_instance_config(&state.current_version, &cfg) {
                    state.instance_settings_status = e;
                    state.notice_seq = state.notice_seq.wrapping_add(1);
                    return clear_notices_later(state.notice_seq);
                }
                Task::none()
            }
            Message::InstanceRamSlider(ram) => {
                let mut cfg = read_instance_config(&state.current_version);
                cfg.ram = Some(ram);
                if let Err(e) = write_instance_config(&state.current_version, &cfg) {
                    state.instance_settings_status = e;
                } else {
                    state.instance_settings_status = String::new();
                }
                state.instance_ram_text = format!("{ram:.2}");
                Task::none()
            }
            Message::InstanceRamText(s) => {
                state.instance_ram_text = s.clone();
                let normalized = s.replace(',', ".");
                match normalized.trim().parse::<f64>() {
                    Ok(v) if (0.5..=32.0).contains(&v) => {
                        let mut cfg = read_instance_config(&state.current_version);
                        cfg.ram = Some(v);
                        if let Err(e) =
                            write_instance_config(&state.current_version, &cfg)
                        {
                            state.instance_settings_status = e;
                        } else {
                            state.instance_settings_status = String::new();
                        }
                    }
                    _ => {
                        state.instance_settings_status =
                            String::from("Enter 0.5 – 32 (GiB).");
                        state.notice_seq = state.notice_seq.wrapping_add(1);
                        return clear_notices_later(state.notice_seq);
                    }
                }
                Task::none()
            }
            Message::InstanceRamReset => {
                let mut cfg = read_instance_config(&state.current_version);
                cfg.ram = None;
                if let Err(e) = write_instance_config(&state.current_version, &cfg) {
                    state.instance_settings_status = e;
                } else {
                    state.instance_settings_status = String::new();
                }
                state.instance_ram_text = format!("{:.2}", state.game_ram);
                Task::none()
            }
            Message::InstanceJavaChanged(name) => {
                let mut cfg = read_instance_config(&state.current_version);
                cfg.java = if name == "Global" {
                    None
                } else {
                    Some(name)
                };
                if let Err(e) = write_instance_config(&state.current_version, &cfg) {
                    state.instance_settings_status = e;
                }
                Task::none()
            }
            Message::GotInstanceFabricLoaders(result) => {
                match result {
                    Ok(list) => {
                        state.il_loader_list = list.clone();
                        state.il_selected = list.first().cloned().unwrap_or_default();
                        state.il_status = String::new();
                    }
                    Err(err) => {
                        state.il_status = err;
                        state.notice_seq = state.notice_seq.wrapping_add(1);
                        return clear_notices_later(state.notice_seq);
                    }
                }
                Task::none()
            }
            Message::GotInstanceNeoForgeList(result) => {
                match result {
                    Ok(pairs) => {
                        // Shared cache, the installer screen benefits too.
                        state.neoforge_all = pairs;
                        let mc = instance_base_mc(&state.current_version);
                        state.il_loader_list = state
                            .neoforge_all
                            .iter()
                            .filter(|(m, _)| *m == mc)
                            .map(|(_, nf)| nf.clone())
                            .collect();
                        state.il_selected = state
                            .il_loader_list
                            .first()
                            .cloned()
                            .unwrap_or_default();
                        state.il_status = String::new();
                    }
                    Err(err) => {
                        state.il_status = err;
                        state.notice_seq = state.notice_seq.wrapping_add(1);
                        return clear_notices_later(state.notice_seq);
                    }
                }
                Task::none()
            }
            Message::InstanceLoaderChanged(loader) => {
                state.il_selected = loader;
                state.il_manual = String::new();
                Task::none()
            }
            Message::InstanceLoaderManualChanged(version) => {
                state.il_manual = version;
                Task::none()
            }
            Message::InstanceLoaderReload => {
                state.il_status = String::from("Loading loader versions...");
                state.il_loader_list = Vec::new();
                state.il_selected = String::new();
                if version_kind(&state.current_version) == VersionKind::Fabric {
                    let base = instance_base_mc(&state.current_version);
                    return Task::perform(
                        async move {
                            downloader::get_fabric_loader_versions(&base).await
                        },
                        Message::GotInstanceFabricLoaders,
                    );
                }
                return Task::perform(
                    downloader::get_neoforge_versions(),
                    Message::GotInstanceNeoForgeList,
                );
            }
            Message::InstanceLoaderApply => {
                if version_kind(&state.current_version) == VersionKind::Fabric {
                    if state.il_selected.is_empty() {
                        state.il_status =
                            String::from("Select a Fabric loader version first.");
                        state.notice_seq = state.notice_seq.wrapping_add(1);
                        return clear_notices_later(state.notice_seq);
                    }
                    let base = instance_base_mc(&state.current_version);
                    let loader = state.il_selected.clone();
                    state.il_status = format!(
                        "Downloading Fabric {loader}... (progress on Installation screen)"
                    );
                    state.downloaders.push(Downloader::new(state.downloaders.len()));
                    let index = state.downloaders.len() - 1;
                    state.downloaders[index].start(
                        base,
                        downloader::VersionType::Fabric { loader },
                    );
                    return Task::none();
                }
                if version_kind(&state.current_version) == VersionKind::NeoForge {
                    let nf = if !state.il_manual.trim().is_empty() {
                        state.il_manual.trim().to_owned()
                    } else {
                        state.il_selected.clone()
                    };
                    if nf.is_empty() {
                        state.il_status = String::from(
                            "Select a NeoForge version or enter it manually.",
                        );
                        state.notice_seq = state.notice_seq.wrapping_add(1);
                        return clear_notices_later(state.notice_seq);
                    }
                    let mc = instance_base_mc(&state.current_version);
                    state.pending_neoforge_install = Some((mc.clone(), nf.clone()));
                    // Same installer flow as the Installation screen; when it
                    // finishes, the selection moves to the new instance.
                    state.pending_loader_switch = Some(format!("neoforge-{nf}"));
                    state.il_status =
                        String::from("Downloading Minecraft files first...");
                    state.downloaders.push(Downloader::new(state.downloaders.len()));
                    let index = state.downloaders.len() - 1;
                    state.downloaders[index]
                        .start(mc, downloader::VersionType::Vanilla);
                    return Task::none();
                }
                state.il_status = String::from("No loader to switch on vanilla.");
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
                state.settings_ram_text = format!("{new_ram:.2}");
                state.settings_status = String::new();
                Task::none()
            }
            Message::SettingsRamText(s) => {
                state.settings_ram_text = s.clone();
                let normalized = s.replace(',', ".");
                match normalized.trim().parse::<f64>() {
                    Ok(v) if (0.5..=32.0).contains(&v) => {
                        state.game_ram = v;
                        state.settings_status = String::new();
                    }
                    _ => {
                        state.settings_status =
                            String::from("Enter 0.5 – 32 (GiB).");
                    }
                }
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
                    Err(err) => {
                        state.download_text = err;
                        state.notice_seq = state.notice_seq.wrapping_add(1);
                        return clear_notices_later(state.notice_seq);
                    }
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
                    Err(err) => {
                        state.download_text = err;
                        state.notice_seq = state.notice_seq.wrapping_add(1);
                        return clear_notices_later(state.notice_seq);
                    }
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
            Message::InstallNameChanged(name) => {
                state.install_name = name;
                Task::none()
            }
            Message::ClearNotices(seq) => {
                // Delayed reset of terminal status notices (5s). Stale
                // timers (a newer notice arrived) never clear fresh text.
                // Progress lines ("Downloading...") are never armed.
                if seq == state.notice_seq {
                    state.download_text = String::new();
                    state.neoforge_status = String::new();
                    state.modstore_status = String::new();
                    state.instance_settings_status = String::new();
                    state.il_status = String::new();
                }
                Task::none()
            }
            Message::InstallPressed => {
                // A manual install cancels a pending loader switch target.
                state.pending_loader_switch = None;
                if state.install_mc_version.is_empty() {
                    state.download_text = String::from("Select a Minecraft version first.");
                    state.notice_seq = state.notice_seq.wrapping_add(1);
                    return clear_notices_later(state.notice_seq);
                }
                // Optional custom instance name, validated now (fail fast);
                // applied to the finished version dir (see Finished).
                let custom = state.install_name.trim().to_owned();
                if !custom.is_empty() {
                    if let Err(e) = valid_new_instance_name(
                        &launcher::get_minecraft_dir(),
                        &custom,
                    ) {
                        state.download_text = e;
                        state.notice_seq = state.notice_seq.wrapping_add(1);
                        return clear_notices_later(state.notice_seq);
                    }
                }
                // Remember (default id -> custom) for the finish handler.
                // `custom == default` needs no rename.
                let mut remember_custom = |default_id: String| {
                    state.pending_install_names.retain(|(o, _)| *o != default_id);
                    if !custom.is_empty() && custom != default_id {
                        state
                            .pending_install_names
                            .push((default_id, custom.clone()));
                    }
                };
                match state.install_loader {
                    LoaderChoice::Vanilla => {
                        let version = state.install_mc_version.clone();
                        remember_custom(version.clone());
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
                            state.notice_seq = state.notice_seq.wrapping_add(1);
                            return clear_notices_later(state.notice_seq);
                        }
                        let version = state.install_mc_version.clone();
                        let loader = state.fabric_loader_selected.clone();
                        remember_custom(format!("{version}-fabric"));
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
                            state.notice_seq = state.notice_seq.wrapping_add(1);
                            return clear_notices_later(state.notice_seq);
                        }
                        remember_custom(format!("neoforge-{nf}"));
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
                        // Which version dir just finished? (MC id vs
                        // "{mc}-fabric" mapping lives in the helper.)
                        let finished_version = state
                            .downloaders
                            .iter()
                            .find(|d| d.id == id)
                            .and_then(|d| finished_version_dir_id(&d.state));
                        state.download_text =
                            String::from("Version installed successfully.");
                        for (index, downloader) in state.downloaders.iter().enumerate() {
                            if downloader.id == id {
                                state.downloaders.remove(index);
                                break;
                            }
                        }
                        // Custom instance name: rename the finished version.
                        let mut refresh = false;
                        if let Some(old_id) = finished_version {
                            if let Some(pos) = state
                                .pending_install_names
                                .iter()
                                .position(|(o, _)| *o == old_id)
                            {
                                let (_, custom) =
                                    state.pending_install_names.remove(pos);
                                match rename_instance(
                                    &launcher::get_minecraft_dir(),
                                    &old_id,
                                    &custom,
                                ) {
                                    Ok(()) => {
                                        state.download_text = format!(
                                            "Version installed as {custom}."
                                        );
                                        refresh = true;
                                    }
                                    Err(e) => {
                                        state.download_text = format!(
                                            "Installed, but rename failed: {e}"
                                        );
                                    }
                                }
                            }
                        }
                        state.notice_seq = state.notice_seq.wrapping_add(1);
                        let mut done_tasks =
                            vec![clear_notices_later(state.notice_seq)];
                        if refresh {
                            done_tasks.push(Task::perform(
                                launcher::getinstalledversions(),
                                Message::LoadVersionList,
                            ));
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
                        return Task::batch(done_tasks);
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
                        state.notice_seq = state.notice_seq.wrapping_add(1);
                        return clear_notices_later(state.notice_seq);
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
                        // Custom name for the freshly installed instance.
                        let finished_nf = state
                            .pending_neoforge_install
                            .clone()
                            .map(|(_, nf)| nf);
                        state.pending_neoforge_install = None;
                        // Loader switch from instance settings: move the
                        // selection to the freshly installed instance.
                        if let Some(new_id) = state.pending_loader_switch.take() {
                            state.current_version = new_id.clone();
                            state.current_version_info = describe_version(&new_id);
                            state.delete_confirm = None;
                            state.il_status =
                                format!("Switched to {new_id}.");
                        }
                        state.neoforge_status =
                            String::from("NeoForge installed successfully.");
                        state.download_text =
                            String::from("NeoForge installed successfully.");
                        if let Some(nf) = finished_nf {
                            let old_id = format!("neoforge-{nf}");
                            if let Some(pos) = state
                                .pending_install_names
                                .iter()
                                .position(|(o, _)| *o == old_id)
                            {
                                let (_, custom) =
                                    state.pending_install_names.remove(pos);
                                match rename_instance(
                                    &launcher::get_minecraft_dir(),
                                    &old_id,
                                    &custom,
                                ) {
                                    Ok(()) => {
                                        state.download_text = format!(
                                            "NeoForge installed as {custom}."
                                        );
                                    }
                                    Err(e) => {
                                        state.download_text = format!(
                                            "Installed, but rename failed: {e}"
                                        );
                                    }
                                }
                            }
                        }
                        for (index, downloader) in state.downloaders.iter().enumerate() {
                            if downloader.id == id {
                                state.downloaders.remove(index);
                                break;
                            }
                        }
                        state.notice_seq = state.notice_seq.wrapping_add(1);
                        return Task::batch(vec![
                            Task::perform(
                                launcher::getinstalledversions(),
                                Message::LoadVersionList,
                            ),
                            clear_notices_later(state.notice_seq),
                        ]);
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
                    state.screen = Screen::Accounts;
                }
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
                    state.screen = Screen::Accounts;
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
        // Fixed full-height sidebar: top items stay on top, account and
        // settings are pinned to the bottom via a Fill spacer, so the bar
        // no longer jumps between pages of different heights.
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

                space::vertical().height(Length::Fill),
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
                        include_bytes!("icons/settings.svg").as_slice()
                    )))
                    .on_press(Message::ChangeScreen(Screen::Settings))
                    .style(theme::transparent_button)
                    .width(Length::Fixed(42.))
                    .height(Length::Fixed(42.)),
                    "Settings"
                ),
            ]
            .spacing(12)
            .align_x(Alignment::Center)
            .width(Length::Fill)
            .height(Length::Fill),
        )
        .style(theme::black_container)
        .align_x(alignment::Horizontal::Center)
        .width(50)
        .height(Length::Fill)
        .padding(4);
        let screen = screens::get_screen_content(state);
        container(
            row![sidebar, screen]
                .spacing(15)
                .height(Length::Fill)
                .align_y(Alignment::Start),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .align_y(alignment::Vertical::Center)
        .padding(15)
        .into()
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
/// Loader kind of an installed version, detected from the version json
/// CONTENT (mainClass/libraries), never from the folder name: folder names
/// are user-chosen since renames, while content always tells the truth.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionKind {
    Vanilla,
    Fabric,
    NeoForge,
    Forge,
    UnknownModded,
}
pub fn version_kind(id: &str) -> VersionKind {
    version_kind_at(&launcher::get_minecraft_dir(), id)
}
pub fn version_kind_at(mc_dir: &str, id: &str) -> VersionKind {
    let path = format!("{mc_dir}/versions/{id}/{id}.json");
    let content = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return VersionKind::Vanilla,
    };
    let v: Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return VersionKind::Vanilla,
    };
    if v["inheritsFrom"].as_str().is_none() {
        return VersionKind::Vanilla;
    }
    let main = v["mainClass"].as_str().unwrap_or("").to_lowercase();
    if main.contains("knot") {
        return VersionKind::Fabric;
    }
    if main.contains("bootstraplauncher") {
        // "neoforge" contains "forge": check neo first everywhere.
        let libs: Vec<String> = v["libraries"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|l| l["name"].as_str().map(str::to_lowercase))
                    .collect()
            })
            .unwrap_or_default();
        if id.to_lowercase().contains("neoforge")
            || libs.iter().any(|l| l.contains("neoforged"))
        {
            return VersionKind::NeoForge;
        }
        return VersionKind::Forge;
    }
    // Odd profiles without a known mainClass: fall back to name markers.
    let lower = id.to_lowercase();
    if lower.contains("neoforge") {
        VersionKind::NeoForge
    } else if lower.contains("fabric") {
        VersionKind::Fabric
    } else if lower.contains("forge") {
        VersionKind::Forge
    } else {
        VersionKind::UnknownModded
    }
}
/// Human-readable description of an installed version, e.g.
/// `Minecraft 1.21.1 • NeoForge`, so modded entries show which Minecraft
/// version they are for. Loader comes from json content (rename-proof).
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
        let loader = match version_kind(id) {
            VersionKind::Fabric => "Fabric",
            VersionKind::NeoForge => "NeoForge",
            VersionKind::Forge => "Forge",
            _ => "Modded",
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
/// Version dir id for a finished version download. NB: the downloader
/// stores the MC id, while Fabric lives in "{mc}-fabric".
fn finished_version_dir_id(state: &DownloaderState) -> Option<String> {
    match state {
        DownloaderState::Downloading(version, version_type) => Some(match version_type {
            downloader::VersionType::Vanilla => version.clone(),
            downloader::VersionType::Fabric { .. } => format!("{version}-fabric"),
        }),
        _ => None,
    }
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
fn instance_dir_for(mc_dir: &str, version: &str) -> String {
    format!(
        "{}/dgrlauncher_instances/{}",
        mc_dir,
        sanitize_instance_name(version)
    )
}
fn game_instance_dir_for_version(version: &str) -> String {
    instance_dir_for(&launcher::get_minecraft_dir(), version)
}
/// Move an installed version (and its instance data) to a new user-chosen
/// name. Pure filesystem work, no launcher state: the result is verified,
/// and any failure rolls the version dir back, so a half-rename never
/// stays on disk. Profile bytes are moved untouched (no "id" patching),
/// so javaVersion/inheritsFrom/libraries can never be corrupted.
/// Name rules shared by rename and custom install names.
pub fn valid_new_instance_name(mc_dir: &str, new: &str) -> Result<(), String> {
    if new.is_empty() || new == "." || new == ".." || new.contains('/') || new.contains('\\')
    {
        return Err(String::from("Invalid name."));
    }
    if Path::new(&format!("{mc_dir}/versions/{new}")).exists() {
        return Err(String::from(
            "A version with this name already exists.",
        ));
    }
    Ok(())
}
pub fn rename_instance(mc_dir: &str, old: &str, new: &str) -> Result<(), String> {
    valid_new_instance_name(mc_dir, new)?;
    if new == old {
        return Err(String::from("Same name, nothing to do."));
    }
    let rollback_dir = || {
        let _ = fs::rename(
            format!("{mc_dir}/versions/{new}"),
            format!("{mc_dir}/versions/{old}"),
        );
    };
    // 1. Version dir (authoritative for launch).
    fs::rename(
        format!("{mc_dir}/versions/{old}"),
        format!("{mc_dir}/versions/{new}"),
    )
    .map_err(|e| format!("Rename failed: {e}"))?;
    // 2. Inner files follow the folder name.
    for ext in ["json", "jar"] {
        let from = format!("{mc_dir}/versions/{new}/{old}.{ext}");
        if Path::new(&from).exists() {
            if let Err(e) = fs::rename(&from, format!("{mc_dir}/versions/{new}/{new}.{ext}"))
            {
                rollback_dir();
                return Err(format!("Rename failed: {e}"));
            }
        }
    }
    // 3. Verify the version is still readable, else roll back.
    let readable = fs::read_to_string(format!("{mc_dir}/versions/{new}/{new}.json"))
        .ok()
        .and_then(|c| serde_json::from_str::<Value>(&c).ok())
        .is_some();
    if !readable {
        rollback_dir();
        return Err(String::from(
            "Version files unreadable after rename, rolled back.",
        ));
    }
    // 4. Instance data dir (worlds, mods, configs move along). On failure
    // the version move above is rolled back too: no half-renames.
    let old_inst = instance_dir_for(mc_dir, old);
    if Path::new(&old_inst).exists() {
        if let Err(e) = fs::rename(&old_inst, instance_dir_for(mc_dir, new)) {
            rollback_dir();
            return Err(format!("Instance folder failed, rolled back: {e}"));
        }
    }
    Ok(())
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
#[cfg(test)]
mod instance_rename_tests {
    use super::*;
    use std::path::PathBuf;

    const VANILLA_JSON: &str = r#"{
        "id": "1.21.1",
        "javaVersion": { "majorVersion": 21 },
        "mainClass": "net.minecraft.client.main.Main"
    }"#;
    const FABRIC_JSON: &str = r#"{
        "id": "fabric-loader-0.16.14-1.21.1",
        "inheritsFrom": "1.21.1",
        "mainClass": "net.fabricmc.loader.impl.launch.knot.KnotClient",
        "libraries": []
    }"#;
    const NEOFORGE_JSON: &str = r#"{
        "id": "neoforge-21.1.213",
        "inheritsFrom": "1.21.1",
        "mainClass": "cpw.mods.bootstraplauncher.BootstrapLauncher",
        "libraries": [{ "name": "net.neoforged:neoforge:21.1.213" }]
    }"#;

    fn test_root(case: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "dgr_rename_{}_{}",
            std::process::id(),
            case
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("versions")).unwrap();
        dir
    }

    fn put_version(mc: &Path, name: &str, json: &str, jar: bool) {
        let dir = mc.join("versions").join(name);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(format!("{name}.json")), json).unwrap();
        if jar {
            fs::write(dir.join(format!("{name}.jar")), b"JARBYTES").unwrap();
        }
    }

    fn mc_str(root: &Path) -> String {
        root.to_string_lossy().into_owned()
    }

    #[test]
    fn vanilla_rename_moves_files_untouched() {
        let root = test_root("vanilla");
        let mc = mc_str(&root);
        put_version(&root, "1.21.1", VANILLA_JSON, true);
        rename_instance(&mc, "1.21.1", "My Vanilla").unwrap();
        assert!(!root.join("versions/1.21.1").exists());
        let json =
            fs::read_to_string(root.join("versions/My Vanilla/My Vanilla.json")).unwrap();
        assert_eq!(json, VANILLA_JSON);
        assert_eq!(
            fs::read(root.join("versions/My Vanilla/My Vanilla.jar")).unwrap(),
            b"JARBYTES"
        );
        assert_eq!(version_kind_at(&mc, "My Vanilla"), VersionKind::Vanilla);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn fabric_rename_keeps_loader_and_vanilla_copy() {
        let root = test_root("fabric");
        let mc = mc_str(&root);
        // Loader profile as saved by the installer ({mc}-fabric.json)...
        put_version(&root, "1.21.1-fabric", FABRIC_JSON, true);
        // ...plus the vanilla copy inside the same dir.
        fs::write(
            root.join("versions/1.21.1-fabric/1.21.1.json"),
            VANILLA_JSON,
        )
        .unwrap();
        rename_instance(&mc, "1.21.1-fabric", "My Pack").unwrap();
        // Loader detection must survive custom names (content, not name).
        assert_eq!(version_kind_at(&mc, "My Pack"), VersionKind::Fabric);
        // Profile bytes untouched (no "id" patching).
        let json =
            fs::read_to_string(root.join("versions/My Pack/My Pack.json")).unwrap();
        assert_eq!(json, FABRIC_JSON);
        // Vanilla copy keeps its own name.
        assert!(root.join("versions/My Pack/1.21.1.json").exists());
        assert!(root.join("versions/My Pack/My Pack.jar").exists());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn neoforge_rename_keeps_loader() {
        let root = test_root("neoforge");
        let mc = mc_str(&root);
        put_version(&root, "neoforge-21.1.213", NEOFORGE_JSON, false);
        rename_instance(&mc, "neoforge-21.1.213", "My Forge Pack").unwrap();
        assert_eq!(
            version_kind_at(&mc, "My Forge Pack"),
            VersionKind::NeoForge
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn rename_collision_and_invalid_fail_unchanged() {
        let root = test_root("collision");
        let mc = mc_str(&root);
        put_version(&root, "1.21.1", VANILLA_JSON, false);
        put_version(&root, "taken", VANILLA_JSON, false);
        assert!(rename_instance(&mc, "1.21.1", "taken").is_err());
        assert!(rename_instance(&mc, "1.21.1", "../evil").is_err());
        assert!(rename_instance(&mc, "1.21.1", "").is_err());
        assert!(root.join("versions/1.21.1/1.21.1.json").exists());
        assert!(!root.join("versions/taken/../evil").exists());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn finished_dir_id_maps_fabric() {
        assert_eq!(
            finished_version_dir_id(&DownloaderState::Downloading(
                String::from("1.21.1"),
                downloader::VersionType::Vanilla
            )),
            Some(String::from("1.21.1"))
        );
        assert_eq!(
            finished_version_dir_id(&DownloaderState::Downloading(
                String::from("1.21.1"),
                downloader::VersionType::Fabric {
                    loader: String::from("0.16.14"),
                }
            )),
            Some(String::from("1.21.1-fabric"))
        );
        assert_eq!(finished_version_dir_id(&DownloaderState::Idle), None);
    }

    #[test]
    fn update_entry_never_uses_another_projects_list() {
        // Regression: updating mod A and then mod B offered A's version
        // to B (with a stuck update icon), because the stale page list
        // of A was reused for B's entry.
        let ver = |id: &str, num: &str| modrinth::ModVersion {
            id: id.to_owned(),
            version_number: num.to_owned(),
            loaders: vec![String::from("fabric")],
            game_versions: vec![String::from("1.21.1")],
            ..Default::default()
        };
        let installed_b = modrinth::InstalledMod {
            project_id: String::from("B"),
            slug: String::from("mod-b"),
            title: String::from("Mod B"),
            icon_url: String::new(),
            version_id: String::from("b1"),
            version_number: String::from("1.0"),
            filename: String::from("mod-b-1.0.jar"),
        };
        let a_list = vec![ver("a2", "2.0"), ver("a1", "1.0")];
        // B just installed, but the loaded list belongs to A -> drop entry.
        assert!(
            refreshed_update_entry(
                Some("A"),
                &a_list,
                "fabric",
                "1.21.1",
                &installed_b
            )
            .is_none()
        );
        // Same project, newer compatible first -> offer it.
        let installed_a = modrinth::InstalledMod {
            project_id: String::from("A"),
            version_id: String::from("a1"),
            ..installed_b.clone()
        };
        let offered =
            refreshed_update_entry(Some("A"), &a_list, "fabric", "1.21.1", &installed_a);
        assert_eq!(offered.map(|v| v.id), Some(String::from("a2")));
        // Already at latest -> no entry.
        let installed_a2 = modrinth::InstalledMod {
            version_id: String::from("a2"),
            ..installed_a.clone()
        };
        assert!(
            refreshed_update_entry(
                Some("A"),
                &a_list,
                "fabric",
                "1.21.1",
                &installed_a2
            )
            .is_none()
        );
        // Same project, but nothing compatible (other loader) -> no entry.
        assert!(
            refreshed_update_entry(
                Some("A"),
                &a_list,
                "neoforge",
                "1.21.1",
                &installed_a
            )
            .is_none()
        );
    }

    #[test]
    fn rename_without_json_rolls_back() {
        let root = test_root("rollback");
        let mc = mc_str(&root);
        // Dir with a jar but no version json: rename must refuse and the
        // old dir must be back in place afterwards.
        let dir = root.join("versions/broken");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("broken.jar"), b"JARBYTES").unwrap();
        assert!(rename_instance(&mc, "broken", "renamed").is_err());
        assert!(root.join("versions/broken").exists());
        assert!(!root.join("versions/renamed").exists());
        let _ = fs::remove_dir_all(&root);
    }
}
