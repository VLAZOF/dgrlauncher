use iced::{
    alignment,
    widget::{
        button, column, container, mouse_area, pick_list, row, scrollable, slider, svg, text,
        text_input, toggler, tooltip, Column,
    },
    Alignment, Length,
};
use crate::{
    theme,
    widget::{Element, Renderer},
    LauncherState, Message, Screen,
};
use serde_json::Value;
use std::collections::HashMap;

/// Short detail lines for the selected instance:
/// `(mc_and_loader_line, java_line)`.
/// Reads `{minecraft_dir}/versions/{id}/{id}.json` when available.
fn instance_details(id: &str, java_name: &str) -> (String, String) {
    let mc_line = super::describe_version(id);
    let mc_line = if mc_line.is_empty() {
        format!("Minecraft {id}")
    } else {
        mc_line
    };
    let json_path = format!(
        "{}/versions/{}/{}.json",
        crate::launcher::get_minecraft_dir(),
        id,
        id
    );
    let (mc_id, required) = match std::fs::read_to_string(&json_path) {
        Ok(content) => match serde_json::from_str::<Value>(&content) {
            Ok(v) => {
                let mc_id = v["inheritsFrom"]
                    .as_str()
                    .unwrap_or(id)
                    .to_owned();
                let required = v["javaVersion"]["majorVersion"]
                    .as_u64()
                    .unwrap_or_else(|| crate::launcher::fallback_java_major(&mc_id));
                (mc_id, required)
            }
            Err(_) => (id.to_owned(), crate::launcher::fallback_java_major(id)),
        },
        Err(_) => (id.to_owned(), crate::launcher::fallback_java_major(id)),
    };
    let _ = mc_id;
    let java_line = if java_name == "Automatic" {
        format!("Java: Automatic (requires Java {required})")
    } else if java_name.is_empty() {
        String::from("Java: Automatic")
    } else {
        format!("Java: {java_name}")
    };
    (mc_line, java_line)
}

/// One-line description: single line, capped length.
fn short_desc(s: &str, max: usize) -> String {
    let one_line = s
        .replace('\n', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if one_line.chars().count() > max {
        let mut t: String = one_line.chars().take(max).collect();
        t.push('…');
        t
    } else {
        one_line
    }
}

/// Per-instance icon from its config: cube (default), star, heart or
/// a letter avatar. Falls back to cube.
/// Nothing is borrowed: icons are static bytes and labels are owned,
// so the result is `'static` and never ties temporaries to the view.
fn instance_icon(icon: &str, title: &str, size: f32) -> Element<'static, Message> {
    let icon_svg = |name: &str| {
        let bytes: &'static [u8] = match name {
            "star" => include_bytes!("icons/star.svg").as_slice(),
            "heart" => include_bytes!("icons/heart.svg").as_slice(),
            _ => include_bytes!("icons/cube.svg").as_slice(),
        };
        svg(svg::Handle::from_memory(bytes))
            .width(size)
            .height(size)
    };
    match icon {
        "star" | "heart" => icon_svg(icon).into(),
        "letter" => {
            let letter = title
                .chars()
                .next()
                .map(|c| c.to_uppercase().to_string())
                .unwrap_or_else(|| String::from("?"));
            container(
                text(letter)
                    .size(size * 0.55)
                    .align_x(alignment::Horizontal::Center)
                    .align_y(alignment::Vertical::Center),
            )
            .style(theme::black_container)
            .width(Length::Fixed(size))
            .height(Length::Fixed(size))
            .align_x(alignment::Horizontal::Center)
            .align_y(alignment::Vertical::Center)
            .into()
        }
        _ => icon_svg("cube").into(),
    }
}

/// Round Back button with a left-arrow icon.
fn back_button<'a>(msg: Message) -> Element<'a, Message> {
    button(
        svg(svg::Handle::from_memory(
            include_bytes!("icons/arrow-left.svg").as_slice(),
        ))
        .width(18)
        .height(18),
    )
    .width(Length::Fixed(40.))
    .height(Length::Fixed(40.))
    .style(theme::round_icon_button)
    .on_press(msg)
    .into()
}

/// Mod icon from the downloaded cache, cube fallback (also on failure).
fn mod_icon<'a>(
    icons: &'a HashMap<String, Option<iced::widget::image::Handle>>,
    url: &'a str,
    size: f32,
) -> Element<'a, Message> {
    if !url.is_empty() {
        if let Some(Some(handle)) = icons.get(url) {
            return iced::widget::image::Image::new(handle.clone())
                .width(size)
                .height(size)
                .into();
        }
    }
    svg(svg::Handle::from_memory(
        include_bytes!("icons/cube.svg").as_slice(),
    ))
    .width(size)
    .height(size)
    .into()
}
pub fn get_screen_content<'a>(
    app: &'a super::DgrLauncher,
) -> Column<'a, Message, super::theme::Theme, Renderer> {
    match app.screen {
        Screen::Main => {
            let has_selection = !app.current_version.is_empty();
            let mut account_name_list = vec![];
            for i in &app.accounts {
                account_name_list.push(i.username.clone())
            }
            let header = row![
                text("Instances").size(30),
                pick_list(
                    account_name_list,
                    Some(app.current_account.username.clone()),
                    Message::CurrentAccountChanged
                )
                .placeholder("Select an account")
                .width(220)
                .text_size(14),
            ]
            .spacing(15)
            .align_y(Alignment::Center);
            let list_content: Column<'a, Message, super::theme::Theme, Renderer> =
                if app.all_versions.is_empty() {
                    column![
                        text("No versions installed yet.").size(15),
                        button(text("Open installer").size(14))
                            .on_press(Message::ChangeScreen(Screen::Installation))
                            .padding(8),
                        text("Install a version, then select it here.")
                            .size(12)
                            .style(theme::peach_text),
                    ]
                    .spacing(10)
                    .align_x(Alignment::Center)
                } else {
                    let mut list = column![].spacing(8);
                    for v in &app.all_versions {
                        let selected = *v == app.current_version;
                        let icon_name =
                            super::read_instance_config(v).icon.unwrap_or_default();
                        let row_content = row![
                            instance_icon(&icon_name, v, 24.),
                            column![
                                text(v.clone()).size(15),
                                text(super::describe_version(v))
                                    .size(11)
                                    .style(theme::peach_text),
                            ]
                            .spacing(2),
                        ]
                        .spacing(10)
                        .align_y(Alignment::Center);
                        let select_button = button(row_content)
                            .on_press(Message::VersionChanged(v.clone()))
                            .style(theme::instance_row_button(selected))
                            .padding(8)
                            .width(Length::Fill);
                        let mut instance_row = row![select_button]
                            .spacing(8)
                            .align_y(Alignment::Center);
                        // Red trash on the right of the selected instance only:
                        // first click arms ("Sure?"), second click deletes.
                        if selected {
                            let armed = app.delete_confirm.as_deref() == Some(v);
                            let delete_button = if armed {
                                button(text("Sure?").size(12))
                                    .on_press(Message::DeleteInstancePressed(v.clone()))
                                    .style(theme::red_button)
                                    .padding(8)
                            } else {
                                button(
                                    svg(svg::Handle::from_memory(
                                        include_bytes!("icons/trash.svg").as_slice(),
                                    ))
                                    .width(16)
                                    .height(16),
                                )
                                .on_press(Message::DeleteInstancePressed(v.clone()))
                                .style(theme::red_button)
                                .padding(8)
                            };
                            instance_row = instance_row.push(delete_button);
                        }
                        list = list.push(instance_row);
                    }
                    // Installer lives here now (sidebar button removed):
                    // a tile that looks like an instance, with a plus.
                    list = list.push(
                        button(
                            row![
                                svg(svg::Handle::from_memory(
                                    include_bytes!("icons/plus.svg").as_slice(),
                                ))
                                .width(24)
                                .height(24),
                                column![
                                    text("Add new version").size(15),
                                    text("Vanilla • Fabric • NeoForge")
                                        .size(11)
                                        .style(theme::peach_text),
                                ]
                                .spacing(2),
                            ]
                            .spacing(10)
                            .align_y(Alignment::Center),
                        )
                        .on_press(Message::ChangeScreen(Screen::Installation))
                        .style(theme::instance_row_button(false))
                        .padding(8)
                        .width(Length::Fill),
                    );
                    list
                };
            let list_area = container(
                scrollable(list_content)
                    .width(Length::Fill)
                    .height(Length::Fill),
            )
            .style(theme::black_container)
            .padding(10)
            .width(Length::Fill)
            .height(Length::Fill);
            let eff_java_name = super::read_instance_config(&app.current_version)
                .java
                .unwrap_or_else(|| app.current_java_name.clone());
            let (mc_line, java_line) = if has_selection {
                instance_details(&app.current_version, &eff_java_name)
            } else {
                (String::from("Select an instance"), String::new())
            };
            let mut info_column = column![
                text(if has_selection {
                    app.current_version.clone()
                } else {
                    String::from("No instance selected")
                })
                .size(15),
                text(mc_line).size(12).style(theme::peach_text),
                text(java_line).size(12),
                row![
                    text(app.game_state_text.clone())
                        .size(12)
                        .style(theme::green_text),
                    button(text("Logs").size(11))
                        .on_press(Message::ChangeScreen(Screen::Logs))
                        .padding(4),
                ]
                .spacing(10)
                .align_y(Alignment::Center),
            ]
            .spacing(4);
            if has_selection {
                info_column = info_column.push(text(format!(
                    "Mods: {}",
                    crate::modrinth::count_installed(&app.current_version)
                )));
            }
            if !app.game_state_text_2.is_empty() {
                info_column = info_column.push(
                    text(app.game_state_text_2.clone())
                        .size(12)
                        .style(theme::green_text),
                );
            }
            let info_box = container(info_column)
                .style(theme::black_container)
                .padding(10)
                .width(Length::Fill);
            // While the game runs, Play becomes a red pause button
            // that closes the game. Otherwise it launches.
            let game_running = matches!(app.launcher.state, LauncherState::GettingLogs(_));
            let play_button = if game_running {
                button(
                    svg(svg::Handle::from_memory(
                        include_bytes!("icons/pause.svg").as_slice(),
                    ))
                    .width(28)
                    .height(28),
                )
                .width(Length::Fixed(64.))
                .height(Length::Fixed(64.))
                .style(theme::round_close_button)
                .on_press(Message::CloseGame)
            } else {
                button(
                    svg(svg::Handle::from_memory(
                        include_bytes!("icons/play.svg").as_slice(),
                    ))
                    .width(28)
                    .height(28),
                )
                .width(Length::Fixed(64.))
                .height(Length::Fixed(64.))
                .style(theme::round_play_button)
                .on_press_maybe(if has_selection {
                    Some(Message::Launch)
                } else {
                    None
                })
            };
            let folder_button = button(
                svg(svg::Handle::from_memory(
                    include_bytes!("icons/folder.svg").as_slice(),
                ))
                .width(20)
                .height(20),
            )
            .width(Length::Fixed(44.))
            .height(Length::Fixed(44.))
            .style(theme::round_icon_button)
            .on_press_maybe(if has_selection {
                Some(Message::OpenGameInstanceFolder)
            } else {
                None
            });
            let gear_button = button(
                svg(svg::Handle::from_memory(
                    include_bytes!("icons/settings.svg").as_slice(),
                ))
                .width(20)
                .height(20),
            )
            .width(Length::Fixed(44.))
            .height(Length::Fixed(44.))
            .style(theme::round_icon_button)
            .on_press_maybe(if has_selection {
                Some(Message::OpenInstanceSettings)
            } else {
                None
            });
            // Mod store: only for modded instances, disabled for vanilla.
            let store_available = has_selection
                && crate::modrinth::instance_loader(&app.current_version).is_some();
            let store_button = button(
                svg(svg::Handle::from_memory(
                    include_bytes!("icons/store.svg").as_slice(),
                ))
                .width(20)
                .height(20),
            )
            .width(Length::Fixed(44.))
            .height(Length::Fixed(44.))
            .style(theme::round_icon_button)
            .on_press_maybe(if store_available {
                Some(Message::OpenModStore)
            } else {
                None
            });
            let controls = row![store_button, folder_button, gear_button, play_button]
                .spacing(10)
                .align_y(Alignment::Center);
            let bottom = row![info_box, controls]
                .spacing(10)
                .align_y(Alignment::End);
            let mut main = column![header, list_area, bottom]
                .spacing(12)
                .width(Length::Fill)
                .height(Length::Fill);
            if app.accounts.is_empty() {
                main = main.push(
                    text("No accounts yet — add one in the Accounts menu.")
                        .size(12)
                        .style(theme::peach_text),
                );
            }
            main
        }
        Screen::Settings => column![
            text("Settings").size(50),
            row![
                container(
                    column![
                        column![
                            text("Java:"),
                            pick_list(
                                app.java_name_list.clone(),
                                Some(app.current_java_name.clone()),
                                Message::JavaChanged
                            )
                            .width(250)
                            .text_size(25),
                            button(
                                text("Custom Java")
                                    .width(250)
                                    .align_x(alignment::Horizontal::Center)
                            )
                            .height(32)
                            .on_press(Message::ChangeScreen(Screen::CustomJava))
                        ]
                        .spacing(10)
                        .max_width(800),
                        column![
                            text("Game data folder:"),
                            text(if app.current_version.is_empty() {
                                String::from("Select a version first")
                            } else {
                                super::game_instance_dir_for_version(&app.current_version)
                            })
                            .size(13),
                            text("Each version keeps its own saves, mods and configs.").size(12)
                        ]
                        .spacing(10)
                        .max_width(800)
                    ]
                    .spacing(10)
                )
                .style(theme::black_container)
                .padding(10),
                container(
                    column![
                        column![
                            text(format!("Allocated memory: {}GiB", app.game_ram))
                                .size(25)
                                .align_x(alignment::Horizontal::Center),
                            slider(0.5..=16.0, app.game_ram, Message::GameRamChanged)
                                .width(250)
                                .step(0.5)
                        ],
                        button("Add wrapper commands")
                            .on_press(Message::ChangeScreen(Screen::ModifyCommand))
                    ]
                    .spacing(50)
                )
                .style(theme::black_container)
                .padding(10)
            ]
            .spacing(15),
        ]
        .spacing(15)
        .max_width(800),
        Screen::Installation => {
            use crate::LoaderChoice;
            let mc_pick_list = pick_list(
                app.vanilla_versions_download_list.clone(),
                Some(app.install_mc_version.clone()),
                Message::VanillaVersionToDownloadChanged,
            )
            .placeholder("Select a Minecraft version")
            .width(250)
            .text_size(15);
            let loader_button = |loader: LoaderChoice| {
                let (label, selected) = match loader {
                    LoaderChoice::Vanilla => ("Vanilla", app.install_loader == loader),
                    LoaderChoice::Fabric => ("Fabric", app.install_loader == loader),
                    LoaderChoice::NeoForge => ("NeoForge", app.install_loader == loader),
                };
                button(text(label).size(14))
                    .on_press(Message::InstallLoaderChanged(loader))
                    .style(if selected {
                        theme::secondary_button
                    } else {
                        theme::primary_button
                    })
                    .padding(5)
            };
            let mut loader_column = column![
                text("Mod loader").size(20),
                row![
                    loader_button(LoaderChoice::Vanilla),
                    loader_button(LoaderChoice::Fabric),
                    loader_button(LoaderChoice::NeoForge),
                ]
                .spacing(10),
            ]
            .spacing(10);
            match app.install_loader {
                LoaderChoice::Vanilla => {}
                LoaderChoice::Fabric => {
                    let fabric_pick_list = pick_list(
                        app.fabric_loader_list.clone(),
                        Some(app.fabric_loader_selected.clone()),
                        Message::FabricLoaderChanged,
                    )
                    .placeholder("Select a loader version")
                    .width(250)
                    .text_size(15);
                    loader_column = loader_column.push(fabric_pick_list);
                }
                LoaderChoice::NeoForge => {
                    let neoforge_pick_list = pick_list(
                        app.neoforge_versions_for_mc.clone(),
                        Some(app.neoforge_selected.clone()),
                        Message::NeoForgeVersionChanged,
                    )
                    .placeholder("Select a NeoForge version")
                    .width(250)
                    .text_size(15);
                    loader_column = loader_column
                        .push(neoforge_pick_list)
                        .push(
                            row![
                                text_input(
                                    "Or enter version manually",
                                    &app.neoforge_manual
                                )
                                .on_input(Message::NeoForgeManualChanged)
                                .size(14)
                                .width(200),
                                button(text("Reload list").size(12))
                                    .on_press(Message::ReloadNeoForgeList)
                                    .padding(5),
                            ]
                            .spacing(10)
                            .align_y(Alignment::Center),
                        )
                        .push(text(&app.neoforge_status).size(12));
                }
            }
            let install_button = button(
                text("Install")
                    .size(20)
                    .align_x(alignment::Horizontal::Center),
            )
            .width(250)
            .height(40)
            .on_press(Message::InstallPressed)
            .style(theme::secondary_button);
            column![
                text("Version installer").size(50),
                container(
                    column![
                        text("Minecraft version").size(20),
                        mc_pick_list,
                        loader_column,
                        install_button,
                    ]
                    .spacing(15)
                )
                .style(theme::black_container)
                .padding(15),
                row![
                    toggler(app.show_all_versions_in_download_list).on_toggle(Message::ShowAllVersionsInDownloadListChanged)
                    .width(Length::Shrink),
                    text("Show non-release versions")
                        .align_x(alignment::Horizontal::Center)
                ]
                .spacing(10),
                text(&app.download_text).size(15)
            ]
            .spacing(15)
            .max_width(800)
        }
        Screen::CustomJava => {
            let mut found_column = column![].spacing(5);
            for java in &app.detected_javas {
                found_column = found_column.push(
                    row![
                        text(format!("Java {} ({})", java.major, java.version)).size(13),
                        button(text("Use").size(12))
                            .on_press(Message::DetectedJavaSelected(java.path.clone()))
                            .padding(5),
                    ]
                    .spacing(10)
                    .align_y(Alignment::Center),
                );
            }
            column![
                text("Custom Java").size(50),
                container(
                    column![
                        text("Installed Java"),
                        button(text("Scan for installed Java").size(14))
                            .on_press(Message::ScanSystemJavas)
                            .padding(5),
                        text(&app.java_scan_status).size(12),
                        found_column,
                        text("Java path:"),
                        text_input("Path to java binary", &app.custom_java_path)
                            .on_input(Message::CustomJavaPathChanged)
                            .size(15)
                            .width(400),
                        text("Java flags (optional):"),
                        text_input("Example: -XX:+UseG1GC", &app.custom_java_flags)
                            .on_input(Message::CustomJavaFlagsChanged)
                            .size(15)
                            .width(400),
                        button(
                            text("Save and use")
                                .size(15)
                                .align_x(alignment::Horizontal::Center)
                        )
                        .width(135)
                        .height(35)
                        .on_press(Message::SaveCustomJava)
                    ]
                    .spacing(10)
                )
                .style(theme::black_container)
                .padding(15)
            ]
            .spacing(15)
            .max_width(800)
        }
        Screen::Logs => column![
            row![
                text("Game logs").size(25),
                button(text("Copy logs").size(12))
                    .on_press(Message::CopyLogs)
                    .padding(5),
            ]
            .spacing(15)
            .align_y(Alignment::Center),
            container(
                scrollable(text(app.logs.join("\n")).size(10))
                    .width(700.0)
                    .height(345.)
            )
            .style(theme::black_container)
            .padding(5)
        ]
        .spacing(10),
        Screen::ModifyCommand => column![
            text("Modify game command").size(50),
            text("Wraper commands").size(25),
            text_input(
                "Example: command1 command2",
                &app.game_wrapper_commands
            )
            .on_input(Message::GameWrapperCommandsChanged)
            .size(12),
            text("Enviroment variables").size(25),
            text_input(
                "Example: KEY1=value1 KEY2=value2",
                &app.game_enviroment_variables
            )
            .on_input(Message::GameEnviromentVariablesChanged)
            .size(12)
        ]
        .spacing(25),
        Screen::InfoAndUpdates => {
            let credits = format!("DgrLauncher {} by VLAZOF.", env!("CARGO_PKG_VERSION"));
            let update_text = if app.update_available {
                format!(
                    "Update available: {} -> {}",
                    env!("CARGO_PKG_VERSION"),
                    app.last_version
                )
            } else {
                app.last_version.clone()
            };
            let update_button_message = match app.update_available {
                true => Some(Message::Update),
                false => None,
            };
            column![
                text("Info and updates").size(50),
                row![
                    container(
                        column![
                            row![
                                text("Updates").size(15),
                                tooltip(
                                    button(svg(svg::Handle::from_memory(
                                        include_bytes!("icons/refresh.svg").as_slice(),
                                    )))
                                    .on_press(Message::RecheckUpdates)
                                    .style(theme::transparent_button)
                                    .width(28)
                                    .height(28)
                                    .padding(2),
                                    "Check for updates",
                                    tooltip::Position::Top,
                                )
                                .style(theme::black_container)
                            ]
                            .spacing(8)
                            .align_y(Alignment::Center),
                            text(update_text),
                            button("Update")
                                .on_press_maybe(update_button_message)
                                .style(theme::secondary_button)
                                .padding(5),
                            text(app.update_text.clone())
                        ]
                        .spacing(30)
                    )
                    .style(theme::black_container)
                    .padding(20),
                    container(
                        column![
                            text("Info").size(15),
                            text(credits),
                            row![button(text("Github repository").size(12))
                                .on_press(Message::OpenURL(
                                    "https://github.com/VLAZOF/dgrlauncher".to_string()
                                ))
                                .padding(5)]
                            .spacing(10)
                        ]
                        .spacing(30)
                    )
                    .style(theme::black_container)
                    .padding(20)
                ]
                .spacing(15),
            ]
            .spacing(25)
        }
        Screen::Accounts => {
            let mut accounts_column = column![];
            for i in &app.accounts {
                let account_type = if i.microsoft { "Microsoft" } else { "Local" };
                let text_content = format!("{} ({})", i.username, account_type);
                let delete_button = button(svg(svg::Handle::from_memory(
                    include_bytes!("icons/trash.svg").as_slice(),
                )))
                .width(30)
                .height(30)
                .style(theme::red_button)
                .on_press(Message::RemoveAccount(i.username.clone()));
                accounts_column =
                    accounts_column.push(row![text(text_content), delete_button].spacing(10));
            }
            column![
                text("Accounts").size(50),
                row![
                    container(
                        column![text("Account list").size(30), accounts_column]
                            .spacing(15)
                            .width(250)
                    )
                    .style(theme::black_container)
                    .padding(15),
                    container(
                        column![
                            button("Add Microsoft account")
                                .on_press(Message::ChangeScreen(Screen::MicrosoftAccount)),
                            button("Add local account")
                                .on_press(Message::ChangeScreen(Screen::LocalAccount)),
                        ]
                        .spacing(15)
                    )
                    .style(theme::black_container)
                    .padding(10)
                ]
                .spacing(15)
            ]
            .spacing(25)
        }
        Screen::MicrosoftAccount => {
            column![
                text("Microsoft Account").size(50),
                container(column![
                    text("Open the page below in the browser and enter the code to authenticate."),
                    row![
                        text(format!("Page: {}", app.auth_code.link)),
                        button("Open in browser")
                            .on_press(Message::OpenURL(app.auth_code.link.clone()))
                    ]
                    .spacing(10),
                    row![
                        text(format!("Code: {}", app.auth_code.code)),
                        button("Copy to clipboard")
                            .on_press(Message::CopyToClipboard(app.auth_code.code.clone()))
                    ]
                    .spacing(10),
                    text(app.auth_status.clone())
                ].spacing(15))
                .style(theme::black_container)
                .padding(15)
            ].spacing(25)
        }
        Screen::LocalAccount => column![
            text("Local Account").size(50),
            container(
                column![
                    text("Account name"),
                    text_input("Account Name", &app.local_account_to_add_name)
                        .on_input(Message::LocalAccountNameChanged)
                        .width(285),
                    button("Add local account").on_press(Message::AddedLocalAccount),
                    text("Account name requires 3 to 16 characters.").size(12)
                ]
                .spacing(15)
            )
            .style(theme::black_container)
            .padding(15)
        ].spacing(25),
        Screen::InstanceSettings => {
            let cfg = super::read_instance_config(&app.current_version);
            let eff_ram = cfg.ram.unwrap_or(app.game_ram);
            let eff_java =
                cfg.java.clone().unwrap_or_else(|| String::from("Global"));
            let header = row![
                back_button(Message::ChangeScreen(Screen::Main)),
                text(format!("Settings — {}", app.current_version)).size(24),
            ]
            .spacing(10)
            .align_y(Alignment::Center);
            // Icon presets: "" = default cube.
            let current_icon = cfg.icon.clone().unwrap_or_default();
            let mut icon_row = row![text("Icon:").size(14)].spacing(10);
            for preset in super::instance_icon_presets() {
                let label = if preset.is_empty() {
                    String::from("cube")
                } else {
                    preset.clone()
                };
                icon_row = icon_row.push(
                    button(
                        row![
                            instance_icon(&label, "A", 20.),
                            text(label).size(12),
                        ]
                        .spacing(6)
                        .align_y(Alignment::Center),
                    )
                    .on_press(Message::InstanceIconChanged(preset.clone()))
                    .style(if current_icon == preset {
                        theme::secondary_button
                    } else {
                        theme::primary_button
                    })
                    .padding(6),
                );
            }
            // Loader switch block depends on the instance kind.
            let loader_block: Column<'a, Message, super::theme::Theme, Renderer> =
                if app.current_version.ends_with("-fabric") {
                    column![
                        text("Fabric loader").size(15),
                        pick_list(
                            app.il_loader_list.clone(),
                            Some(app.il_selected.clone()),
                            Message::InstanceLoaderChanged,
                        )
                        .placeholder("Select a loader version")
                        .width(250)
                        .text_size(14),
                        row![
                            button(text("Apply loader").size(13))
                                .on_press(Message::InstanceLoaderApply)
                                .style(theme::secondary_button)
                                .padding(6),
                            button(text("Reload list").size(12))
                                .on_press(Message::InstanceLoaderReload)
                                .padding(6),
                        ]
                        .spacing(10),
                        text(&app.il_status).size(12),
                        text(&app.download_text).size(12),
                    ]
                    .spacing(8)
                } else if app.current_version.starts_with("neoforge-") {
                    column![
                        text("NeoForge version").size(15),
                        pick_list(
                            app.il_loader_list.clone(),
                            Some(app.il_selected.clone()),
                            Message::InstanceLoaderChanged,
                        )
                        .placeholder("Select a NeoForge version")
                        .width(250)
                        .text_size(14),
                        row![
                            text_input("Or enter version manually", &app.il_manual)
                                .on_input(Message::InstanceLoaderManualChanged)
                                .size(13)
                                .width(200),
                            button(text("Reload list").size(12))
                                .on_press(Message::InstanceLoaderReload)
                                .padding(6),
                        ]
                        .spacing(10)
                        .align_y(Alignment::Center),
                        row![button(text("Apply version").size(13))
                            .on_press(Message::InstanceLoaderApply)
                            .style(theme::secondary_button)
                            .padding(6),]
                        .spacing(10),
                        text(&app.il_status).size(12),
                        text(&app.neoforge_status).size(12),
                        text(&app.download_text).size(12),
                    ]
                    .spacing(8)
                } else {
                    column![
                        text("Vanilla instance — no mod loader to switch.")
                            .size(12)
                            .style(theme::peach_text),
                    ]
                    .spacing(8)
                };
            let content = column![
                text("Name (renames the version and instance folders)").size(15),
                row![
                    text_input("Instance name", &app.instance_name_edit)
                        .on_input(Message::InstanceNameChanged)
                        .on_submit(Message::InstanceRenamePressed)
                        .size(14)
                        .width(Length::Fill),
                    button(text("Rename").size(13))
                        .on_press(Message::InstanceRenamePressed)
                        .padding(8),
                ]
                .spacing(10)
                .align_y(Alignment::Center),
                icon_row,
                text(format!("Memory (global: {:.1} GiB)", app.game_ram)).size(15),
                row![
                    slider(
                        0.5..=16.0,
                        eff_ram,
                        Message::InstanceRamSlider
                    )
                    .step(0.25)
                    .width(Length::Fill),
                    text_input("GiB", &app.instance_ram_text)
                        .on_input(Message::InstanceRamText)
                        .size(14)
                        .width(80),
                    button(text("Global").size(12))
                        .on_press(Message::InstanceRamReset)
                        .padding(6),
                ]
                .spacing(10)
                .align_y(Alignment::Center),
                text("Java (Global = Settings choice)").size(15),
                pick_list(
                    vec![
                        String::from("Global"),
                        String::from("Automatic"),
                        String::from("System Java"),
                        String::from("Custom"),
                    ],
                    Some(eff_java),
                    Message::InstanceJavaChanged,
                )
                .width(250)
                .text_size(14),
                loader_block,
                row![
                    button(text("Open game folder").size(12))
                        .on_press(Message::OpenGameFolder)
                        .padding(6),
                    button(text("Open instance folder").size(12))
                        .on_press(Message::OpenGameInstanceFolder)
                        .padding(6),
                ]
                .spacing(10),
                text(&app.instance_settings_status)
                    .size(12)
                    .style(theme::peach_text),
            ]
            .spacing(12);
            column![
                header,
                container(
                    scrollable(content)
                        .width(Length::Fill)
                        .height(Length::Fill)
                )
                .style(theme::black_container)
                .padding(15)
                .width(Length::Fill)
                .height(Length::Fill),
            ]
            .spacing(10)
            .width(Length::Fill)
            .height(Length::Fill)
        }
        Screen::ModStore => {
            use super::ModStoreTab;
            let header = row![
                back_button(Message::ChangeScreen(Screen::Main)),
                text(format!("Mod store — {}", app.current_version)).size(20),
            ]
            .spacing(10)
            .align_y(Alignment::Center);
            let tab_button = |tab: ModStoreTab, label: &'a str| {
                button(text(label).size(14))
                    .on_press(Message::ModStoreTabChanged(tab))
                    .style(if app.modstore_tab == tab {
                        theme::secondary_button
                    } else {
                        theme::primary_button
                    })
                    .padding(6)
            };
            let mut tabs_row = row![
                tab_button(ModStoreTab::Store, "Store"),
                tab_button(ModStoreTab::Installed, "Installed"),
            ]
            .spacing(10);
            if app.modstore_tab == ModStoreTab::Installed {
                tabs_row = tabs_row.push(
                    button(text("Check updates").size(12))
                        .on_press_maybe(if app.modstore_pending_checks == 0 {
                            Some(Message::ModCheckUpdates)
                        } else {
                            None
                        })
                        .padding(6),
                );
            }
            // Live search: catalog queries Modrinth with a debounce,
            // installed filters locally as you type. Enter still works.
            let search = text_input("Search mods...", &app.modstore_query)
                .on_input(Message::ModStoreQueryChanged)
                .on_submit(Message::ModStoreSearch)
                .size(14)
                .width(Length::Fill);
            let content: Column<'a, Message, super::theme::Theme, Renderer> =
                match app.modstore_tab {
                    ModStoreTab::Store => {
                        let mut list = column![].spacing(8);
                        for m in &app.modstore_results {
                            let page_id = if m.slug.is_empty() {
                                m.id.clone()
                            } else {
                                m.slug.clone()
                            };
                            let mut title_row =
                                row![text(m.title.clone()).size(15)].spacing(8);
                            if app.modstore_installed.contains_key(&m.id) {
                                title_row = title_row.push(
                                    text("✓ installed")
                                        .size(12)
                                        .style(theme::green_text),
                                );
                            }
                            let item = row![
                                mod_icon(&app.modstore_icons, &m.icon_url, 40.),
                                column![
                                    title_row,
                                    text(format!(
                                        "{}  •  {} downloads",
                                        short_desc(&m.description, 90),
                                        m.downloads
                                    ))
                                    .size(12)
                                    .style(theme::peach_text),
                                ]
                                .spacing(2)
                                .width(Length::Fill),
                            ]
                            .spacing(10)
                            .align_y(Alignment::Center);
                            list = list.push(
                                button(item)
                                    .on_press(Message::OpenModPage(page_id))
                                    .style(theme::instance_row_button(false))
                                    .padding(8)
                                    .width(Length::Fill),
                            );
                        }
                        list
                    }
                    ModStoreTab::Installed => {
                        let q = app.modstore_query.to_lowercase();
                        let mut entries: Vec<_> =
                            app.modstore_installed.values().collect();
                        entries.sort_by(|a, b| a.title.cmp(&b.title));
                        let mut list = column![].spacing(8);
                        for e in entries {
                            if !q.is_empty()
                                && !e.title.to_lowercase().contains(&q)
                                && !e.filename.to_lowercase().contains(&q)
                            {
                                continue;
                            }
                            let page_id = if e.slug.is_empty() {
                                e.project_id.clone()
                            } else {
                                e.slug.clone()
                            };
                            let item = row![
                                mod_icon(&app.modstore_icons, &e.icon_url, 40.),
                                column![
                                    text(e.title.clone()).size(15),
                                    text(format!(
                                        "{} • {}",
                                        e.version_number, e.filename
                                    ))
                                    .size(11)
                                    .style(theme::peach_text),
                                ]
                                .spacing(2)
                                .width(Length::Fill),
                            ]
                            .spacing(10)
                            .align_y(Alignment::Center);
                            let open_button = button(item)
                                .on_press(Message::OpenModPage(page_id))
                                .style(theme::instance_row_button(false))
                                .padding(8)
                                .width(Length::Fill);
                            let mut installed_row = row![open_button]
                                .spacing(8)
                                .align_y(Alignment::Center);
                            // Orange refresh when a newer compatible version
                            // was found (auto on store open + manual button).
                            if app.modstore_updates.contains_key(&e.project_id) {
                                installed_row = installed_row.push(
                                    button(
                                        svg(svg::Handle::from_memory(
                                            include_bytes!("icons/refresh.svg").as_slice(),
                                        ))
                                        .style(theme::orange_svg)
                                        .width(16)
                                        .height(16),
                                    )
                                    .on_press(Message::ModUpdatePressed(
                                        e.project_id.clone(),
                                    ))
                                    .style(theme::round_icon_button)
                                    .padding(8),
                                );
                            }
                            {
                                let armed = app.modstore_delete_confirm.as_deref()
                                    == Some(&e.project_id);
                                let delete_button = if armed {
                                    button(text("Sure?").size(12))
                                        .on_press(Message::ModDeletePressed(
                                            e.project_id.clone(),
                                        ))
                                        .style(theme::red_button)
                                        .padding(8)
                                } else {
                                    button(
                                        svg(svg::Handle::from_memory(
                                            include_bytes!("icons/trash.svg").as_slice(),
                                        ))
                                        .width(16)
                                        .height(16),
                                    )
                                    .on_press(Message::ModDeletePressed(
                                        e.project_id.clone(),
                                    ))
                                    .style(theme::red_button)
                                    .padding(8)
                                };
                                installed_row =
                                    installed_row.push(delete_button);
                            }
                            list = list.push(installed_row);
                            // Inline update confirm: current -> new version.
                            if app.modstore_update_confirm.as_deref()
                                == Some(&e.project_id)
                            {
                                if let Some(latest) =
                                    app.modstore_updates.get(&e.project_id)
                                {
                                    list = list.push(
                                        container(
                                            row![
                                                text(format!(
                                                    "{} → {}",
                                                    e.version_number,
                                                    latest.version_number
                                                ))
                                                .size(13)
                                                .width(Length::Fill),
                                                button(text("Update").size(12))
                                                    .on_press(Message::ModUpdateApply(
                                                        e.project_id.clone(),
                                                    ))
                                                    .style(theme::secondary_button)
                                                    .padding(6),
                                                button(text("Cancel").size(12))
                                                    .on_press(Message::ModUpdatePressed(
                                                        e.project_id.clone(),
                                                    ))
                                                    .padding(6),
                                            ]
                                            .spacing(10)
                                            .align_y(Alignment::Center),
                                        )
                                        .style(theme::black_container)
                                        .padding(8)
                                        .width(Length::Fill),
                                    );
                                }
                            }
                        }
                        if app.modstore_installed.is_empty() {
                            list = list.push(
                                text("No mods installed yet.")
                                    .size(14)
                                    .style(theme::peach_text),
                            );
                        }
                        // Hand-dropped jars: matched ones auto-link (updates
                        // work), the rest stay here with delete-only actions.
                        if app.modstore_linking {
                            list = list.push(
                                text("Identifying hand-added files...")
                                    .size(12)
                                    .style(theme::peach_text),
                            );
                        }
                        if !app.modstore_unlinked.is_empty() {
                            list = list.push(text("Hand-added files:").size(14));
                            for name in &app.modstore_unlinked {
                                let key = format!("file:{name}");
                                let armed = app.modstore_delete_confirm.as_deref()
                                    == Some(&key);
                                let delete_button = if armed {
                                    button(text("Sure?").size(12))
                                        .on_press(Message::ModLocalFileDelete(
                                            name.clone(),
                                        ))
                                        .style(theme::red_button)
                                        .padding(8)
                                } else {
                                    button(
                                        svg(svg::Handle::from_memory(
                                            include_bytes!("icons/trash.svg").as_slice(),
                                        ))
                                        .width(16)
                                        .height(16),
                                    )
                                    .on_press(Message::ModLocalFileDelete(name.clone()))
                                    .style(theme::red_button)
                                    .padding(8)
                                };
                                list = list.push(
                                    row![
                                        column![
                                            text(name.clone()).size(14),
                                            text("external file • no updates")
                                                .size(11)
                                                .style(theme::peach_text),
                                        ]
                                        .spacing(2)
                                        .width(Length::Fill),
                                        delete_button,
                                    ]
                                    .spacing(10)
                                    .align_y(Alignment::Center),
                                );
                            }
                        }
                        list
                    }
                };
            let list_area = container(
                scrollable(content)
                    .id(iced::widget::Id::new("modstore-list"))
                    .on_scroll(|vp: scrollable::Viewport| {
                        Message::ModStoreScrolled(vp.absolute_offset().y)
                    })
                    .width(Length::Fill)
                    .height(Length::Fill),
            )
            .style(theme::black_container)
            .padding(10)
            .width(Length::Fill)
            .height(Length::Fill);
            let mut main = column![header, tabs_row, search, list_area]
                .spacing(10)
                .width(Length::Fill);
            if !app.modstore_status.is_empty() {
                main = main.push(
                    text(app.modstore_status.clone())
                        .size(12)
                        .style(theme::peach_text),
                );
            }
            main.height(Length::Fill)
        }
        Screen::ModPage => {
            let back = row![
                back_button(Message::ModPageBack),
                text(
                    app.modstore_detail
                        .as_ref()
                        .map(|d| d.title.clone())
                        .unwrap_or_else(|| String::from("Mod page"))
                )
                .size(20),
            ]
            .spacing(10)
            .align_y(Alignment::Center);
            let content: Column<'a, Message, super::theme::Theme, Renderer> =
                match &app.modstore_detail {
                    None => column![
                        text(if app.modstore_status.is_empty() {
                            String::from("Loading...")
                        } else {
                            app.modstore_status.clone()
                        })
                        .size(14)
                        .style(theme::peach_text),
                    ]
                    .spacing(10),
                    Some(d) => {
                        let (mc, loader) =
                            crate::modrinth::instance_loader(&app.current_version)
                                .unwrap_or((String::new(), String::new()));
                        let info = row![
                            mod_icon(&app.modstore_icons, &d.icon_url, 64.),
                            column![
                                text(short_desc(&d.description, 200)).size(13),
                                text(format!("Downloads: {}", d.downloads)).size(12),
                            ]
                            .spacing(4)
                            .width(Length::Fill),
                        ]
                        .spacing(12)
                        .align_y(Alignment::Center);
                        let mut top = column![info].spacing(10);
                        // Dependencies of the newest compatible version:
                        // expandable, required vs optional marked.
                        // A click opens that mod's page (Back returns here).
                        if let Some(latest) = crate::modrinth::latest_compatible(
                            &app.modstore_versions,
                            &loader,
                            &mc,
                        ) {
                            if !latest.dependencies.is_empty() {
                                top = top.push(
                                    button(
                                        text(format!(
                                            "Dependencies ({}) {}",
                                            latest.dependencies.len(),
                                            if app.modstore_deps_expanded {
                                                "▾"
                                            } else {
                                                "▸"
                                            }
                                        ))
                                        .size(13),
                                    )
                                    .on_press(Message::ModDepsToggled)
                                    .padding(6),
                                );
                                if app.modstore_deps_expanded {
                                    let mut deps_col = column![].spacing(4);
                                    for dep in &latest.dependencies {
                                        let badge = if dep.dependency_type
                                            == "required"
                                        {
                                            text("required")
                                                .size(11)
                                                .style(theme::red_text)
                                        } else {
                                            text("optional")
                                                .size(11)
                                                .style(theme::peach_text)
                                        };
                                        match &dep.project_id {
                                            Some(pid) => {
                                                let (slug, title) =
                                                    app.modstore_dep_titles
                                                        .get(pid)
                                                        .cloned()
                                                        .unwrap_or((
                                                            String::new(),
                                                            short_desc(pid, 24),
                                                        ));
                                                let page_id = if slug.is_empty() {
                                                    pid.clone()
                                                } else {
                                                    slug
                                                };
                                                deps_col = deps_col.push(
                                                    button(
                                                        row![
                                                            badge,
                                                            text(title).size(13),
                                                        ]
                                                        .spacing(8)
                                                        .align_y(Alignment::Center),
                                                    )
                                                    .on_press(Message::OpenModPage(
                                                        page_id,
                                                    ))
                                                    .style(
                                                        theme::instance_row_button(false),
                                                    )
                                                    .padding(6)
                                                    .width(Length::Fill),
                                                );
                                            }
                                            None => {
                                                deps_col = deps_col.push(
                                                    row![
                                                        badge,
                                                        text("unknown project").size(13),
                                                    ]
                                                    .spacing(8),
                                                );
                                            }
                                        }
                                    }
                                    top = top.push(deps_col);
                                }
                            }
                        }
                        // Compatible versions only. The action buttons live
                        // on the row under the cursor: Download, or ✓ and
                        // Delete for the installed one. No locked buttons.
                        let installed = app.modstore_installed.get(&d.id);
                        let mut versions = column![].spacing(6);
                        let mut shown = 0;
                        for v in &app.modstore_versions {
                            if !crate::modrinth::is_compatible(v, &loader, &mc) {
                                continue;
                            }
                            shown += 1;
                            let hovered =
                                app.modstore_hovered_version.as_deref() == Some(&v.id);
                            let is_installed = installed
                                .map(|e| e.version_id == v.id)
                                .unwrap_or(false);
                            let info_col = column![
                                text(v.version_number.clone()).size(14),
                                text(format!(
                                    "✓ {loader} • {}",
                                    short_mc(&v.game_versions)
                                ))
                                .size(11)
                                .style(theme::green_text),
                            ]
                            .spacing(2)
                            .width(Length::Fill);
                            let action: Element<'_, Message> = if is_installed {
                                let armed = app.modstore_delete_confirm.as_deref()
                                    == Some(&d.id);
                                row![
                                    text("✓").size(14).style(theme::green_text),
                                    if armed {
                                        button(text("Sure?").size(12))
                                            .on_press(Message::ModDeletePressed(
                                                d.id.clone(),
                                            ))
                                            .style(theme::red_button)
                                            .padding(6)
                                    } else {
                                        button(
                                            svg(svg::Handle::from_memory(
                                                include_bytes!("icons/trash.svg")
                                                    .as_slice(),
                                            ))
                                            .width(14)
                                            .height(14),
                                        )
                                        .on_press(Message::ModDeletePressed(
                                            d.id.clone(),
                                        ))
                                        .style(theme::red_button)
                                        .padding(6)
                                    },
                                ]
                                .spacing(8)
                                .align_y(Alignment::Center)
                                .into()
                            } else if hovered {
                                button(
                                    svg(svg::Handle::from_memory(
                                        include_bytes!("icons/download.svg").as_slice(),
                                    ))
                                    .width(14)
                                    .height(14),
                                )
                                .on_press_maybe(if app.modstore_downloading {
                                    None
                                } else {
                                    Some(Message::ModInstallVersion(v.id.clone()))
                                })
                                .style(theme::secondary_button)
                                .padding(8)
                                .into()
                            } else {
                                row![].into()
                            };
                            let version_row = row![info_col, action]
                                .spacing(10)
                                .align_y(Alignment::Center);
                            versions = versions.push(
                                mouse_area(version_row)
                                    .on_enter(Message::ModVersionHovered(v.id.clone()))
                                    .on_exit(Message::ModVersionUnhovered(
                                        v.id.clone(),
                                    )),
                            );
                        }
                        if shown == 0 {
                            versions = versions.push(
                                text(format!(
                                    "No compatible versions for {loader} • {mc}."
                                ))
                                .size(13)
                                .style(theme::peach_text),
                            );
                        }
                        // Extra right padding: the overlay scrollbar
                        // must not cover the row action buttons.
                        let versions_area = container(
                            scrollable(versions)
                                .width(Length::Fill)
                                .height(Length::Fill),
                        )
                        .style(theme::black_container)
                        .padding(iced::Padding {
                            top: 10.0,
                            right: 24.0,
                            bottom: 10.0,
                            left: 10.0,
                        })
                        .width(Length::Fill)
                        .height(Length::Fill);
                        top = top.push(versions_area);
                        top
                    }
                };
            let mut main = column![back, content]
                .spacing(10)
                .width(Length::Fill)
                .height(Length::Fill);
            if app.modstore_detail.is_some() && !app.modstore_status.is_empty() {
                main = main.push(
                    text(app.modstore_status.clone())
                        .size(12)
                        .style(theme::peach_text),
                );
            }
            main
        }
    }
}

/// Up to 3 game versions, comma-joined, for compact version rows.
fn short_mc(versions: &[String]) -> String {
    let shown: Vec<&str> = versions.iter().take(3).map(String::as_str).collect();
    let mut s = shown.join(", ");
    if versions.len() > 3 {
        s.push_str(", …");
    }
    if s.is_empty() {
        s.push('?');
    }
    s
}
