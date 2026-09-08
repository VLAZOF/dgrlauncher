use iced::{
    alignment,
    widget::{
        button, column, container, pick_list, row, scrollable, slider, svg, text, text_input,
        toggler, tooltip, Column,
    },
    Alignment, Length,
};
use crate::{downloader, theme, widget::Renderer, LauncherState, Message, Screen};
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
                            .text_size(15)
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
                            text("JVM:"),
                            pick_list(
                                app.java_name_list.clone(),
                                Some(app.current_java_name.clone()),
                                Message::JavaChanged
                            )
                            .width(250)
                            .text_size(25),
                            button(
                                text("Manage JVMs")
                                    .width(250)
                                    .align_x(alignment::Horizontal::Center)
                            )
                            .height(32)
                            .on_press(Message::ChangeScreen(Screen::Java))
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
            let vanilla_pick_list = pick_list(
                app.vanilla_versions_download_list.clone(),
                Some(app.vanilla_version_to_download.clone()),
                Message::VanillaVersionToDownloadChanged,
            )
            .placeholder("Select a version")
            .width(250)
            .text_size(15);
            let fabric_pick_list = pick_list(
                app.fabric_versions_download_list.clone(),
                Some(app.fabric_version_to_download.clone()),
                Message::FabricVersionToDownloadChanged,
            )
            .placeholder("Select a version")
            .width(250)
            .text_size(15);
            let vanilla_button_message = match app.vanilla_version_to_download.is_empty() {
                true => None,
                false => Some(Message::InstallVersion(downloader::VersionType::Vanilla)),
            };
            let fabric_button_message = match app.fabric_version_to_download.is_empty() {
                true => None,
                false => Some(Message::InstallVersion(downloader::VersionType::Fabric)),
            };
            let vanilla_install_button = button(
                text("Install")
                    .size(20)
                    .align_x(alignment::Horizontal::Center),
            )
            .width(250)
            .height(40)
            .on_press_maybe(vanilla_button_message)
            .style(theme::secondary_button);
            let fabric_install_button = button(
                text("Install")
                    .size(20)
                    .align_x(alignment::Horizontal::Center),
            )
            .width(250)
            .height(40)
            .on_press_maybe(fabric_button_message)
            .style(theme::secondary_button);
            column![
                text("Version installer").size(50),
                row![
                    container(
                        column![
                            text("Vanilla"),
                            vanilla_pick_list,
                            vanilla_install_button
                        ]
                        .spacing(15)
                    )
                    .style(theme::black_container)
                    .padding(10),
                    container(
                        column![
                            text("Fabric"),
                            fabric_pick_list,
                            fabric_install_button
                        ]
                        .spacing(15)
                    )
                    .style(theme::black_container)
                    .padding(10)
                ]
                .spacing(15),
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
        Screen::Java => column![
            text("Manage JVMs")
                .size(50)
                .align_x(alignment::Horizontal::Center),
            container(
                column![
                    text("New JVM"),
                    text("JVM name:"),
                    text_input("", &app.jvm_to_add_name)
                        .on_input(Message::JvmNameToAddChanged)
                        .size(25)
                        .width(250),
                    text("JVM path:"),
                    text_input("", &app.jvm_to_add_path)
                        .on_input(Message::JvmPathToAddChanged)
                        .size(25)
                        .width(250),
                    text("JVM flags:"),
                    text_input("", &app.jvm_to_add_flags)
                        .on_input(Message::JvmFlagsToAddChanged)
                        .size(25)
                        .width(250),
                    button(
                        text("Add")
                            .size(15)
                            .align_x(alignment::Horizontal::Center)
                    )
                    .width(135)
                    .height(35)
                    .on_press(Message::JvmAdded)
                ]
                .spacing(5)
            )
            .style(theme::black_container)
            .padding(15)
        ]
        .spacing(15)
        .max_width(800),
        Screen::Logs => column![
            text("Game logs").size(25),
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
