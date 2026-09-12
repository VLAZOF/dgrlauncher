use iced::{
    alignment,
    widget::{
        button, column, container, mouse_area, pick_list, row, scrollable, slider, svg, text,
        text_input, toggler, tooltip, Column, Row,
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
use rust_i18n::t;

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
    // Missing/corrupt json: say so honestly instead of guessing Java 8
    // from a user-chosen folder name via the fallback table.
    let parsed: Option<Value> = std::fs::read_to_string(&json_path)
        .ok()
        .and_then(|c| serde_json::from_str(&c).ok());
    let java_line = match parsed {
        Some(v) => {
            let mc_id = v["inheritsFrom"].as_str().unwrap_or(id).to_owned();
            let required = v["javaVersion"]["majorVersion"]
                .as_u64()
                .unwrap_or_else(|| crate::launcher::fallback_java_major(&mc_id));
            if java_name == "Automatic" || java_name.is_empty() {
                t!("main.java_auto", required = required).to_string()
            } else {
                t!("main.java_named", name = java_name).to_string()
            }
        }
        None => t!("main.java_unknown").to_string(),
    };
    (mc_line, java_line)
}

/// Shared memory row (global settings and per-instance settings):
/// value label + slider + manual input. The instance screen appends its
/// own "Global" reset button to the returned row.
fn ram_controls<'a>(
    value: f64,
    input: &'a str,
    on_slider: fn(f64) -> Message,
    on_input: fn(String) -> Message,
) -> Row<'a, Message, super::theme::Theme, Renderer> {
    row![
        text(format!("{value:.1} GiB")).size(14).width(Length::Fixed(80.)),
        slider(0.5..=16.0, value, on_slider)
            .step(0.25)
            .width(Length::Fill),
        text_input("GiB", input)
            .on_input(on_input)
            .size(14)
            .width(80),
    ]
    .spacing(10)
    .align_y(Alignment::Center)
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
                text(t!("main.instances")).size(30),
                pick_list(
                    account_name_list,
                    Some(app.current_account.username.clone()),
                    Message::CurrentAccountChanged
                )
                .placeholder(t!("main.select_account"))
                .width(220)
                .text_size(14),
            ]
            .spacing(15)
            .align_y(Alignment::Center);
            let list_content: Column<'a, Message, super::theme::Theme, Renderer> =
                if app.all_versions.is_empty() {
                    column![
                        text(t!("main.no_versions")).size(15),
                        button(text(t!("main.open_installer")).size(14))
                            .on_press(Message::ChangeScreen(Screen::Installation))
                            .padding(8),
                        text(t!("main.install_hint"))
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
                                button(text(t!("common.sure")).size(12))
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
                                    text(t!("main.add_version")).size(15),
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
                (t!("main.select_instance").to_string(), String::new())
            };
            let mut info_column = column![
                text(if has_selection {
                    app.current_version.clone()
                } else {
                    t!("main.no_instance").to_string()
                })
                .size(15),
                text(mc_line).size(12).style(theme::peach_text),
                text(java_line).size(12),
                row![
                    text(app.game_state_text.clone())
                        .size(12)
                        .style(theme::green_text),
                    button(text(t!("main.logs")).size(11))
                        .on_press(Message::ChangeScreen(Screen::Logs))
                        .padding(4),
                ]
                .spacing(10)
                .align_y(Alignment::Center),
            ]
            .spacing(4);
            if has_selection {
                info_column = info_column.push(text(t!(
                    "main.mods_count",
                    count = crate::modrinth::count_installed(&app.current_version)
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
                    text(t!("main.no_accounts"))
                        .size(12)
                        .style(theme::peach_text),
                );
            }
            main
        }
        Screen::Settings => {
            let update_line = if app.update_available {
                t!(
                    "settings.update_available",
                    current = env!("CARGO_PKG_VERSION"),
                    latest = &app.last_version
                )
                .to_string()
            } else {
                app.last_version.clone()
            };
            column![
                text(t!("settings.title")).size(30),
                container(
                    column![
                        text(t!("settings.default")).size(20),
                        row![
                            text(t!("settings.java")).size(14).width(Length::Fixed(120.)),
                            pick_list(
                                app.java_name_list.clone(),
                                Some(app.current_java_name.clone()),
                                Message::JavaChanged
                            )
                            .width(220)
                            .text_size(14),
                            button(text(t!("settings.custom_java")).size(13))
                                .on_press(Message::ChangeScreen(Screen::CustomJava))
                                .padding(6),
                        ]
                        .spacing(10)
                        .align_y(Alignment::Center),
                        row![
                            text(t!("settings.language"))
                                .size(14)
                                .width(Length::Fixed(120.)),
                            pick_list(
                                vec![String::from("English"), String::from("Русский")],
                                Some(crate::i18n::display_name(&app.language)),
                                |label| Message::LanguageChanged(
                                    crate::i18n::code_for_display(&label)
                                ),
                            )
                            .width(220)
                            .text_size(14),
                        ]
                        .spacing(10)
                        .align_y(Alignment::Center),
                        ram_controls(
                            app.game_ram,
                            &app.settings_ram_text,
                            Message::GameRamChanged,
                            Message::SettingsRamText,
                        ),
                        text(&app.settings_status)
                            .size(12)
                            .style(theme::peach_text),
                        row![
                            button(text(t!("cmd.wrapper")).size(13))
                                .on_press(Message::ChangeScreen(
                                    Screen::ModifyCommand
                                ))
                                .padding(6),
                        ]
                        .spacing(10),
                        text(t!("settings.other")).size(20),
                        row![
                            text(update_line).size(13).width(Length::Fill),
                            tooltip(
                                button(svg(svg::Handle::from_memory(
                                    include_bytes!("icons/refresh.svg").as_slice(),
                                )))
                                .on_press(Message::RecheckUpdates)
                                .style(theme::transparent_button)
                                .width(28)
                                .height(28)
                                .padding(2),
                                text(t!("settings.check_updates")),
                                tooltip::Position::Top,
                            )
                            .style(theme::black_container),
                            tooltip(
                                button(svg(svg::Handle::from_memory(
                                    include_bytes!("icons/github.svg").as_slice(),
                                )))
                                .on_press(Message::OpenURL(
                                    "https://github.com/VLAZOF/dgrlauncher".to_string()
                                ))
                                .style(theme::transparent_button)
                                .width(28)
                                .height(28)
                                .padding(2),
                                text(t!("settings.github")),
                                tooltip::Position::Top,
                            )
                            .style(theme::black_container),
                            button(text(t!("settings.update")).size(13))
                                .on_press_maybe(if app.update_available {
                                    Some(Message::Update)
                                } else {
                                    None
                                })
                                .style(theme::secondary_button)
                                .padding(6),
                        ]
                        .spacing(10)
                        .align_y(Alignment::Center),
                        text(&app.update_text).size(12),
                    ]
                    .spacing(12)
                )
                .style(theme::black_container)
                .padding(15)
                .width(Length::Fill),
            ]
            .spacing(15)
            .width(Length::Fill)
        }
        Screen::Installation => {
            use crate::LoaderChoice;
            let mc_pick_list = pick_list(
                app.vanilla_versions_download_list.clone(),
                Some(app.install_mc_version.clone()),
                Message::VanillaVersionToDownloadChanged,
            )
            .placeholder(t!("install.mc_placeholder"))
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
                text(t!("install.loader_label")).size(20),
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
                        .placeholder(t!("instance.loader_fabric_ph"))
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
                        .placeholder(t!("instance.loader_neoforge_ph"))
                    .width(250)
                    .text_size(15);
                    loader_column = loader_column
                        .push(neoforge_pick_list)
                        .push(
                            row![
                                text_input(
                                    crate::tr!("install.manual_placeholder"),
                                    &app.neoforge_manual
                                )
                                .on_input(Message::NeoForgeManualChanged)
                                .size(14)
                                .width(200),
                                button(text(t!("common.reload")).size(12))
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
                text(t!("install.install"))
                    .size(20)
                    .align_x(alignment::Horizontal::Center),
            )
            .width(250)
            .height(40)
            .on_press(Message::InstallPressed)
            .style(theme::secondary_button);
            column![
                text(t!("install.title")).size(50),
                container(
                    column![
                        text(t!("install.mc_label")).size(20),
                        mc_pick_list,
                        loader_column,
                        text(t!("install.name_label")).size(14),
                        text_input(
                            crate::tr!("install.name_placeholder"),
                            &app.install_name
                        )
                        .on_input(Message::InstallNameChanged)
                        .size(14)
                        .width(250),
                        install_button,
                    ]
                    .spacing(15)
                )
                .style(theme::black_container)
                .padding(15),
                row![
                    toggler(app.show_all_versions_in_download_list).on_toggle(Message::ShowAllVersionsInDownloadListChanged)
                    .width(Length::Shrink),
                    text(t!("install.show_non_release"))
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
                        button(text(t!("customjava.use")).size(12))
                            .on_press(Message::DetectedJavaSelected(java.path.clone()))
                            .padding(5),
                    ]
                    .spacing(10)
                    .align_y(Alignment::Center),
                );
            }
            column![
                text(t!("customjava.title")).size(50),
                container(
                    column![
                        text(t!("customjava.installed_label")),
                        button(text(t!("customjava.scan")).size(14))
                            .on_press(Message::ScanSystemJavas)
                            .padding(5),
                        text(&app.java_scan_status).size(12),
                        found_column,
                        text(t!("customjava.path_label")),
                        text_input(crate::tr!("customjava.path_placeholder"), &app.custom_java_path)
                            .on_input(Message::CustomJavaPathChanged)
                            .size(15)
                            .width(400),
                        text(t!("customjava.flags_label")),
                        text_input(crate::tr!("customjava.flags_placeholder"), &app.custom_java_flags)
                            .on_input(Message::CustomJavaFlagsChanged)
                            .size(15)
                            .width(400),
                        button(
                            text(t!("customjava.save"))
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
                text(t!("logs.title")).size(25),
                button(text(t!("logs.copy")).size(12))
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
            text(t!("cmd.title")).size(50),
            text(t!("cmd.wrapper")).size(25),
            text_input(
                crate::tr!("cmd.wrapper_ph"),
                &app.game_wrapper_commands
            )
            .on_input(Message::GameWrapperCommandsChanged)
            .size(12),
            text(t!("cmd.env")).size(25),
            text_input(
                crate::tr!("cmd.env_ph"),
                &app.game_enviroment_variables
            )
            .on_input(Message::GameEnviromentVariablesChanged)
            .size(12)
        ]
        .spacing(25),
        Screen::Accounts => {
            let mut accounts_column = column![];
            for i in &app.accounts {
                let account_type = if i.microsoft {
                    t!("accounts.type_ms")
                } else {
                    t!("accounts.type_local")
                };
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
                text(t!("accounts.title")).size(50),
                row![
                    container(
                        column![text(t!("accounts.list")).size(30), accounts_column]
                            .spacing(15)
                            .width(250)
                    )
                    .style(theme::black_container)
                    .padding(15),
                    container(
                        column![
                            button(text(t!("accounts.add_ms")))
                                .on_press(Message::ChangeScreen(Screen::MicrosoftAccount)),
                            button(text(t!("accounts.add_local")))
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
                text(t!("msauth.title")).size(50),
                container(column![
                    text(t!("msauth.hint")),
                    row![
                        text(t!("msauth.page", link = &app.auth_code.link)),
                        button(text(t!("msauth.open_browser")))
                            .on_press(Message::OpenURL(app.auth_code.link.clone()))
                    ]
                    .spacing(10),
                    row![
                        text(t!("msauth.code", code = &app.auth_code.code)),
                        button(text(t!("msauth.copy_code")))
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
            text(t!("localauth.title")).size(50),
            container(
                column![
                    text(t!("localauth.name_label")),
                    text_input(crate::tr!("localauth.name_ph"), &app.local_account_to_add_name)
                        .on_input(Message::LocalAccountNameChanged)
                        .width(285),
                    button(text(t!("localauth.add"))).on_press(Message::AddedLocalAccount),
                    text(t!("localauth.hint")).size(12)
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
                text(t!("instance.title", name = &app.current_version)).size(24),
            ]
            .spacing(10)
            .align_y(Alignment::Center);
            // Icon presets: "" = default cube.
            let current_icon = cfg.icon.clone().unwrap_or_default();
            let mut icon_row = row![text(t!("instance.icon")).size(14)].spacing(10);
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
                if crate::version_kind(&app.current_version) == crate::VersionKind::Fabric {
                    column![
                        text(t!("instance.loader_fabric")).size(15),
                        pick_list(
                            app.il_loader_list.clone(),
                            Some(app.il_selected.clone()),
                            Message::InstanceLoaderChanged,
                        )
                        .placeholder(t!("instance.loader_fabric_ph"))
                        .width(250)
                        .text_size(14),
                        row![
                            button(text(t!("instance.apply_loader")).size(13))
                                .on_press(Message::InstanceLoaderApply)
                                .style(theme::secondary_button)
                                .padding(6),
                            button(text(t!("common.reload")).size(12))
                                .on_press(Message::InstanceLoaderReload)
                                .padding(6),
                        ]
                        .spacing(10),
                        text(&app.il_status).size(12),
                        text(&app.download_text).size(12),
                    ]
                    .spacing(8)
                } else if crate::version_kind(&app.current_version)
                    == crate::VersionKind::NeoForge
                {
                    column![
                        text(t!("instance.loader_neoforge")).size(15),
                        pick_list(
                            app.il_loader_list.clone(),
                            Some(app.il_selected.clone()),
                            Message::InstanceLoaderChanged,
                        )
                        .placeholder(t!("instance.loader_neoforge_ph"))
                        .width(250)
                        .text_size(14),
                        row![
                            text_input(crate::tr!("instance.manual_ph"), &app.il_manual)
                                .on_input(Message::InstanceLoaderManualChanged)
                                .size(13)
                                .width(200),
                            button(text(t!("common.reload")).size(12))
                                .on_press(Message::InstanceLoaderReload)
                                .padding(6),
                        ]
                        .spacing(10)
                        .align_y(Alignment::Center),
                        row![button(text(t!("instance.apply_version")).size(13))
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
                        text(t!("instance.vanilla_note"))
                            .size(12)
                            .style(theme::peach_text),
                    ]
                    .spacing(8)
                };
            let content = column![
                text(t!("instance.name_label")).size(15),
                row![
                    text_input(crate::tr!("instance.name_ph"), &app.instance_name_edit)
                        .on_input(Message::InstanceNameChanged)
                        .on_submit(Message::InstanceRenamePressed)
                        .size(14)
                        .width(Length::Fill),
                    button(text(t!("instance.rename")).size(13))
                        .on_press(Message::InstanceRenamePressed)
                        .padding(8),
                ]
                .spacing(10)
                .align_y(Alignment::Center),
                icon_row,
                text(t!("instance.memory", ram = format!("{:.1}", app.game_ram))).size(15),
                ram_controls(
                    eff_ram,
                    &app.instance_ram_text,
                    Message::InstanceRamSlider,
                    Message::InstanceRamText,
                )
                .push(
                    button(text(t!("instance.global_btn")).size(12))
                        .on_press(Message::InstanceRamReset)
                        .padding(6),
                ),
                text(t!("instance.java_label")).size(15),
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
                    button(text(t!("instance.open_game")).size(12))
                        .on_press(Message::OpenGameFolder)
                        .padding(6),
                    button(text(t!("instance.open_instance")).size(12))
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
                text(t!("store.title", name = &app.current_version)).size(20),
            ]
            .spacing(10)
            .align_y(Alignment::Center);
            let tab_button = |tab: ModStoreTab, label: String| {
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
                tab_button(ModStoreTab::Store, t!("store.tab_store").to_string()),
                tab_button(
                    ModStoreTab::Installed,
                    t!("store.tab_installed").to_string()
                ),
            ]
            .spacing(10);
            if app.modstore_tab == ModStoreTab::Installed {
                // Refresh re-reads the mods folder (hand-dropped files show
                // up without leaving the tab); Check updates hits Modrinth.
                tabs_row = tabs_row.push(
                    button(text(t!("store.refresh")).size(12))
                        .on_press(Message::ModStoreTabChanged(ModStoreTab::Installed))
                        .padding(6),
                );
                tabs_row = tabs_row.push(
                    button(text(t!("store.check_updates")).size(12))
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
            let search = text_input(crate::tr!("store.search_ph"), &app.modstore_query)
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
                                    text(t!("store.installed_badge"))
                                        .size(12)
                                        .style(theme::green_text),
                                );
                            }
                            let item = row![
                                mod_icon(&app.modstore_icons, &m.icon_url, 40.),
                                column![
                                    title_row,
                                    text(t!(
                                        "store.downloads",
                                        desc = short_desc(&m.description, 90),
                                        count = m.downloads
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
                                    button(text(t!("common.sure")).size(12))
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
                                                button(text(t!("common.update")).size(12))
                                                    .on_press(Message::ModUpdateApply(
                                                        e.project_id.clone(),
                                                    ))
                                                    .style(theme::secondary_button)
                                                    .padding(6),
                                                button(text(t!("common.cancel")).size(12))
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
                                text(t!("store.no_mods"))
                                    .size(14)
                                    .style(theme::peach_text),
                            );
                        }
                        // Hand-dropped jars: matched ones auto-link (updates
                        // work), the rest stay here with delete-only actions.
                        if app.modstore_linking {
                            list = list.push(
                                text(t!("store.identifying"))
                                    .size(12)
                                    .style(theme::peach_text),
                            );
                        }
                        if !app.modstore_unlinked.is_empty() {
                            list = list.push(text(t!("store.hand_added")).size(14));
                            for name in &app.modstore_unlinked {
                                let key = format!("file:{name}");
                                let armed = app.modstore_delete_confirm.as_deref()
                                    == Some(&key);
                                let delete_button = if armed {
                                    button(text(t!("common.sure")).size(12))
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
                                            text(t!("store.external_note"))
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
                        .unwrap_or_else(|| t!("store.page_fallback").to_string())
                )
                .size(20),
            ]
            .spacing(10)
            .align_y(Alignment::Center);
            let content: Column<'a, Message, super::theme::Theme, Renderer> =
                match &app.modstore_detail {
                    None => column![
                        text(if app.modstore_status.is_empty() {
                            t!("store.loading").to_string()
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
                                text(t!("store.downloads_label", count = d.downloads)).size(12),
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
                                        text(t!(
                                            "store.deps",
                                            count = latest.dependencies.len(),
                                            arrow = if app.modstore_deps_expanded {
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
                                            text(t!("store.required"))
                                                .size(11)
                                                .style(theme::red_text)
                                        } else {
                                            text(t!("store.optional"))
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
                                                        text(t!("store.unknown_project")).size(13),
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
                                        button(text(t!("common.sure")).size(12))
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
                                text(t!(
                                    "store.no_compatible",
                                    loader = &loader,
                                    mc = &mc
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
