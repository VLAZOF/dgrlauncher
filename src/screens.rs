use iced::{
    alignment,
    widget::{
        button, column, container, pick_list, row, scrollable, slider, svg, text, text_input,
        toggler, tooltip, Column,
    },
    Alignment, Length,
};
use crate::{theme, widget::Renderer, LauncherState, Message, Screen};
pub fn get_screen_content<'a>(
    app: &'a super::DgrLauncher,
) -> Column<'a, Message, super::theme::Theme, Renderer> {
    match app.screen {
        Screen::Main => {
            let (launch_text, launch_message) = match app.launcher.state {
                LauncherState::Idle => ("Launch", Option::Some(Message::Launch)),
                LauncherState::Launching(_) => ("Launching", Option::None),
                LauncherState::GettingLogs(_) => ("Running", Option::None),
                LauncherState::Waiting => ("...", Option::None),
            };
            let launch_button = button(
                text(launch_text)
                    .size(35)
                    .align_x(alignment::Horizontal::Center)
                    .align_y(alignment::Vertical::Center),
            )
            .width(285)
            .height(60)
            .on_press_maybe(launch_message);
            let close_button = match app.launcher.state {
                LauncherState::GettingLogs(_) => Some(
                    button(
                        text("Close game")
                            .size(15)
                            .align_x(alignment::Horizontal::Center)
                            .align_y(alignment::Vertical::Center),
                    )
                    .width(189)
                    .height(35)
                    .on_press(Message::CloseGame)
                    .style(theme::red_button),
                ),
                _ => None,
            };
            let mut account_name_list = vec![];
            for i in &app.accounts {
                account_name_list.push(i.username.clone())
            }
            column![
                column![
                    text("DgrLauncher").size(50),
                    text(format!("Hello {}!", app.current_account.username))
                        .style(theme::peach_text)
                        .size(18)
                ]
                .spacing(5),
                row![
                    container(
                        column![
                            text("Account"),
                            pick_list(
                                account_name_list,
                                Some(app.current_account.username.clone()),
                                Message::CurrentAccountChanged
                            )
                            .placeholder("Select an Account")
                            .width(285)
                            .text_size(15),
                            text("Version:"),
                            pick_list(
                                app.all_versions.clone(),
                                Some(app.current_version.clone()),
                                Message::VersionChanged,
                            )
                            .placeholder("Select a version")
                            .width(285)
                            .text_size(15),
                            text(&app.current_version_info)
                                .size(12)
                                .style(theme::peach_text)
                        ]
                        .spacing(10)
                    )
                    .style(theme::black_container)
                    .padding(10),
                    container(
                        column![
                            button(
                                text("Open game folder")
                                    .align_x(alignment::Horizontal::Center)
                            )
                            .width(200)
                            .height(32)
                            .on_press(Message::OpenGameFolder),
                            button(
                                text("Open current instance folder")
                                    .align_x(alignment::Horizontal::Center)
                            )
                            .width(200)
                            .height(32)
                            .on_press(Message::OpenGameInstanceFolder),
                            button(
                                text("Logs").align_x(alignment::Horizontal::Center)
                            )
                            .width(80)
                            .height(32)
                            .on_press(Message::ChangeScreen(Screen::Logs)),
                        ]
                        .spacing(10)
                        .align_x(Alignment::Center)
                    )
                    .style(theme::black_container)
                    .padding(20)
                ]
                .spacing(15),
                row![
                    {
                        let launch_col =
                            column![launch_button,].spacing(15).align_x(Alignment::Center);
                        match close_button {
                            Some(btn) => launch_col.push(btn),
                            None => launch_col,
                        }
                    },
                    column![text(app.game_state_text.to_string())
                        .style(theme::green_text)
                        .size(15)
                        .height(40), text(app.game_state_text_2.to_string())
                        .style(theme::green_text)
                        .size(15)
                        .height(40)].spacing(5)
                ]
                .spacing(10),
            ]
            .spacing(25)
            .max_width(800)
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
        Screen::GettingStarted => column![
            text("Getting started").size(50),
            container(
                column![
                    text("Hi, this is DgrLauncher, an open source Minecraft Launcher! To start, let's add an account."),
                    row![button("Add Microsoft account")
                    .on_press(Message::ChangeScreen(Screen::MicrosoftAccount)),
                button("Add local account")
                    .on_press(Message::ChangeScreen(Screen::LocalAccount)),].spacing(10),
                ]
                .spacing(15)
            )
            .style(theme::black_container)
            .padding(15)
        ].spacing(25),
        Screen::GettingStarted2 => column![
            text("Getting started").size(50),
            container(
                column![
                    text(format!("Great, nice to meet you, {}. You don't have any Minecraft version installed, you can install a Minecraft version in the installation menu.", app.current_account.username)),
                    row![button("Installation menu")
                    .on_press(Message::ChangeScreen(Screen::Installation)),
                button("Skip")
                    .on_press(Message::ChangeScreen(Screen::Main)),].spacing(10),
                ]
                .spacing(15)
            )
            .style(theme::black_container)
            .padding(15)
        ].spacing(25),
    }
}
