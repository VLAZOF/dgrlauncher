use iced::overlay::menu;
use iced::widget::{
    button, container, pick_list, progress_bar, scrollable, slider, svg, text,
    text_input, toggler,
};
use iced::{Background, Border, Color, Shadow, color, theme};
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Theme;
impl theme::Base for Theme {
    fn default(_preference: theme::Mode) -> Self {
        Theme
    }
    fn mode(&self) -> theme::Mode {
        theme::Mode::Dark
    }
    fn base(&self) -> theme::Style {
        theme::Style {
            background_color: Color::from_rgb8(30, 30, 46),
            text_color: color!(205, 214, 244),
        }
    }
    fn palette(&self) -> Option<theme::Palette> {
        None
    }
    fn name(&self) -> &str {
        "dgrlauncher"
    }
}
pub fn default_text(_theme: &Theme) -> text::Style {
    text::Style {
        color: Some(color!(205, 214, 244)),
    }
}
pub fn peach_text(_theme: &Theme) -> text::Style {
    text::Style {
        color: Some(color!(250, 179, 135)),
    }
}
pub fn green_text(_theme: &Theme) -> text::Style {
    text::Style {
        color: Some(color!(166, 218, 149)),
    }
}
pub fn red_text(_theme: &Theme) -> text::Style {
    text::Style {
        color: Some(color!(150, 0, 0)),
    }
}
impl text::Catalog for Theme {
    type Class<'a> = text::StyleFn<'a, Self>;
    fn default<'a>() -> Self::Class<'a> {
        Box::new(default_text)
    }
    fn style(&self, class: &Self::Class<'_>) -> text::Style {
        class(self)
    }
}
pub fn default_container(_theme: &Theme) -> container::Style {
    container::Style::default()
}
pub fn black_container(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgb8(49, 50, 68))),
        border: Border {
            radius: 25.0.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}
impl container::Catalog for Theme {
    type Class<'a> = container::StyleFn<'a, Self>;
    fn default<'a>() -> Self::Class<'a> {
        Box::new(default_container)
    }
    fn style(&self, class: &Self::Class<'_>) -> container::Style {
        class(self)
    }
}
fn button_primary_active() -> button::Style {
    button::Style {
        background: Some(Background::Color(Color::from_rgb8(30, 102, 245))),
        text_color: color!(205, 214, 244),
        border: Border {
            radius: 15.0.into(),
            width: 1.0,
            color: color!(30, 102, 245),
        },
        shadow: Shadow::default(),
        snap: false,
    }
}
pub fn primary_button(_theme: &Theme, status: button::Status) -> button::Style {
    let active = button_primary_active();
    match status {
        button::Status::Active | button::Status::Pressed => active,
        button::Status::Hovered => button::Style {
            border: Border {
                radius: 15.0.into(),
                width: 1.0,
                color: Color::from_rgb8(205, 214, 244),
                ..Default::default()
            },
            ..active
        },
        button::Status::Disabled => button::Style {
            background: Some(Background::Color(Color::from_rgb8(46, 59, 98))),
            border: Border {
                radius: 15.0.into(),
                width: 1.0,
                color: Color::from_rgb8(46, 59, 98),
                ..Default::default()
            },
            ..active
        },
    }
}
pub fn secondary_button(_theme: &Theme, status: button::Status) -> button::Style {
    let active = button::Style {
        background: Some(Background::Color(Color::from_rgb8(5, 194, 112))),
        text_color: color!(205, 214, 244),
        border: Border {
            radius: 15.0.into(),
            ..Default::default()
        },
        shadow: Shadow::default(),
        snap: false,
    };
    match status {
        button::Status::Active | button::Status::Pressed => active,
        button::Status::Hovered => button::Style {
            border: Border {
                radius: 15.0.into(),
                width: 1.0,
                color: Color::from_rgb8(205, 214, 244),
                ..Default::default()
            },
            ..active
        },
        button::Status::Disabled => button::Style {
            background: Some(Background::Color(Color::from_rgb8(90, 115, 90))),
            ..active
        },
    }
}
pub fn red_button(_theme: &Theme, _status: button::Status) -> button::Style {
    button::Style {
        background: Some(Background::Color(Color::from_rgb8(210, 15, 57))),
        text_color: color!(205, 214, 244),
        border: Border {
            radius: 15.0.into(),
            ..Default::default()
        },
        shadow: Shadow::default(),
        snap: false,
    }
}
/// Big round green Play button (64x64). Radius 100 makes it a circle.
pub fn round_play_button(_theme: &Theme, status: button::Status) -> button::Style {
    let active = button::Style {
        background: Some(Background::Color(Color::from_rgb8(5, 194, 112))),
        text_color: color!(205, 214, 244),
        border: Border {
            radius: 100.0.into(),
            ..Default::default()
        },
        shadow: Shadow::default(),
        snap: false,
    };
    match status {
        button::Status::Active | button::Status::Pressed => active,
        button::Status::Hovered => button::Style {
            border: Border {
                radius: 100.0.into(),
                width: 1.0,
                color: Color::from_rgb8(205, 214, 244),
            },
            ..active
        },
        button::Status::Disabled => button::Style {
            background: Some(Background::Color(Color::from_rgb8(90, 115, 90))),
            border: Border {
                radius: 100.0.into(),
                ..Default::default()
            },
            ..active
        },
    }
}
/// Big round red Stop button (64x64): replaces Play while the game runs.
pub fn round_close_button(_theme: &Theme, status: button::Status) -> button::Style {
    let active = button::Style {
        background: Some(Background::Color(Color::from_rgb8(210, 15, 57))),
        text_color: color!(205, 214, 244),
        border: Border {
            radius: 100.0.into(),
            ..Default::default()
        },
        shadow: Shadow::default(),
        snap: false,
    };
    match status {
        button::Status::Active | button::Status::Pressed => active,
        button::Status::Hovered => button::Style {
            border: Border {
                radius: 100.0.into(),
                width: 1.0,
                color: Color::from_rgb8(205, 214, 244),
            },
            ..active
        },
        button::Status::Disabled => button::Style {
            background: Some(Background::Color(Color::from_rgb8(90, 70, 75))),
            border: Border {
                radius: 100.0.into(),
                ..Default::default()
            },
            ..active
        },
    }
}
/// Small round icon button (folder / gear), 44x44 circle.
pub fn round_icon_button(_theme: &Theme, status: button::Status) -> button::Style {
    let active = button::Style {
        background: Some(Background::Color(Color::from_rgb8(49, 50, 68))),
        text_color: color!(205, 214, 244),
        border: Border {
            radius: 100.0.into(),
            ..Default::default()
        },
        shadow: Shadow::default(),
        snap: false,
    };
    match status {
        button::Status::Active | button::Status::Pressed => active,
        button::Status::Hovered => button::Style {
            border: Border {
                radius: 100.0.into(),
                width: 1.0,
                color: Color::from_rgb8(205, 214, 244),
            },
            ..active
        },
        button::Status::Disabled => button::Style {
            background: Some(Background::Color(Color::from_rgb8(40, 42, 54))),
            text_color: Color::from_rgb8(88, 91, 112),
            border: Border {
                radius: 100.0.into(),
                ..Default::default()
            },
            shadow: Shadow::default(),
            snap: false,
        },
    }
}
/// Instance row: highlighted when selected.
pub fn instance_row_button(selected: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_theme: &Theme, status: button::Status| {
        let base = if selected {
            Color::from_rgb8(30, 102, 245)
        } else {
            Color::from_rgb8(49, 50, 68)
        };
        let active = button::Style {
            background: Some(Background::Color(base)),
            text_color: color!(205, 214, 244),
            border: Border {
                radius: 15.0.into(),
                ..Default::default()
            },
            shadow: Shadow::default(),
            snap: false,
        };
        match status {
            button::Status::Hovered => button::Style {
                border: Border {
                    radius: 15.0.into(),
                    width: 1.0,
                    color: Color::from_rgb8(205, 214, 244),
                },
                ..active
            },
            _ => active,
        }
    }
}
pub fn transparent_button(_theme: &Theme, status: button::Status) -> button::Style {
    match status {
        button::Status::Hovered => button::Style {
            background: Some(Background::Color(Color::TRANSPARENT)),
            text_color: color!(205, 214, 244),
            border: Border {
                radius: 100.0.into(),
                ..Default::default()
            },
            shadow: Shadow::default(),
            snap: false,
        },
        _ => button::Style {
            background: Some(Background::Color(Color::TRANSPARENT)),
            text_color: color!(205, 214, 244),
            border: Border::default(),
            shadow: Shadow::default(),
            snap: false,
        },
    }
}
impl button::Catalog for Theme {
    type Class<'a> = button::StyleFn<'a, Self>;
    fn default<'a>() -> Self::Class<'a> {
        Box::new(primary_button)
    }
    fn style(&self, class: &Self::Class<'_>, status: button::Status) -> button::Style {
        class(self, status)
    }
}
fn text_input_style(border_color: Color) -> text_input::Style {
    text_input::Style {
        background: Background::Color(Color::from_rgb8(49, 50, 68)),
        border: Border {
            radius: 15.0.into(),
            width: 1.0,
            color: border_color,
        },
        icon: Color::from_rgb8(205, 214, 244),
        placeholder: Color::from_rgb8(88, 91, 112),
        value: Color::from_rgb8(205, 214, 244),
        selection: Color::from_rgb8(205, 214, 244),
    }
}
pub fn text_input_default(theme: &Theme, status: text_input::Status) -> text_input::Style {
    let _ = theme;
    match status {
        text_input::Status::Active => text_input_style(Color::from_rgb8(49, 50, 68)),
        text_input::Status::Hovered | text_input::Status::Focused { .. } => {
            text_input_style(Color::from_rgb8(205, 214, 244))
        }
        text_input::Status::Disabled => text_input::Style {
            background: Background::Color(Color::from_rgb(
                0x20 as f32 / 255.0,
                0x22 as f32 / 255.0,
                0x25 as f32 / 255.0,
            )),
            border: Border {
                radius: 15.0.into(),
                width: 1.0,
                color: Color::from_rgb(
                    0x20 as f32 / 255.0,
                    0x22 as f32 / 255.0,
                    0x25 as f32 / 255.0,
                ),
                ..Default::default()
            },
            icon: Color::from_rgb8(205, 214, 244),
            placeholder: Color::from_rgb8(88, 91, 112),
            value: Color::from_rgb8(88, 91, 112),
            selection: Color::from_rgb8(205, 214, 244),
        },
    }
}
impl text_input::Catalog for Theme {
    type Class<'a> = text_input::StyleFn<'a, Self>;
    fn default<'a>() -> Self::Class<'a> {
        Box::new(text_input_default)
    }
    fn style(&self, class: &Self::Class<'_>, status: text_input::Status) -> text_input::Style {
        class(self, status)
    }
}
pub fn pick_list_default(_theme: &Theme, status: pick_list::Status) -> pick_list::Style {
    let active = pick_list::Style {
        text_color: Color::from_rgb8(205, 214, 244),
        background: Background::Color(Color::from_rgb8(49, 50, 68)),
        placeholder_color: Color::from_rgb(
            0x20 as f32 / 255.0,
            0x22 as f32 / 255.0,
            0x25 as f32 / 255.0,
        ),
        handle_color: Color::from_rgb8(205, 214, 244),
        border: Border {
            radius: 15.0.into(),
            width: 1.0,
            color: Color::from_rgb8(49, 50, 68),
        },
    };
    match status {
        pick_list::Status::Active => active,
        pick_list::Status::Hovered | pick_list::Status::Opened { .. } => pick_list::Style {
            border: Border {
                radius: 15.0.into(),
                width: 1.0,
                color: Color::from_rgb8(205, 214, 244),
            },
            ..active
        },
    }
}
impl pick_list::Catalog for Theme {
    type Class<'a> = pick_list::StyleFn<'a, Self>;
    fn default<'a>() -> <Self as pick_list::Catalog>::Class<'a> {
        Box::new(pick_list_default)
    }
    fn style(
        &self,
        class: &<Self as pick_list::Catalog>::Class<'_>,
        status: pick_list::Status,
    ) -> pick_list::Style {
        class(self, status)
    }
}
pub fn svg_default(_theme: &Theme, status: svg::Status) -> svg::Style {
    match status {
        svg::Status::Idle => svg::Style {
            color: Some(Color::from_rgb8(255, 255, 255)),
        },
        svg::Status::Hovered => svg::Style {
            color: Some(Color::from_rgb8(220, 220, 255)),
        },
    }
}
/// Bright-orange icon (mod update indicator).
pub fn orange_svg(_theme: &Theme, _status: svg::Status) -> svg::Style {
    svg::Style {
        color: Some(Color::from_rgb8(255, 165, 0)),
    }
}
impl svg::Catalog for Theme {
    type Class<'a> = svg::StyleFn<'a, Self>;
    fn default<'a>() -> Self::Class<'a> {
        Box::new(svg_default)
    }
    fn style(&self, class: &Self::Class<'_>, status: svg::Status) -> svg::Style {
        class(self, status)
    }
}
pub fn slider_default(_theme: &Theme, status: slider::Status) -> slider::Style {
    let rail = slider::Rail {
        backgrounds: (
            Background::Color(Color::from_rgb8(205, 214, 244)),
            Background::Color(Color::from_rgb8(205, 214, 244)),
        ),
        width: 2.0,
        border: Border {
            radius: 4.0.into(),
            ..Default::default()
        },
    };
    let shape = slider::HandleShape::Rectangle {
        width: 8,
        border_radius: 4.0.into(),
    };
    match status {
        slider::Status::Active => slider::Style {
            rail,
            handle: slider::Handle {
                shape,
                background: Background::Color(Color::from_rgb(
                    0x20 as f32 / 255.0,
                    0x22 as f32 / 255.0,
                    0x25 as f32 / 255.0,
                )),
                border_width: 1.0,
                border_color: Color::from_rgb8(205, 214, 244),
            },
        },
        slider::Status::Hovered | slider::Status::Dragged => slider::Style {
            rail,
            handle: slider::Handle {
                shape,
                background: Background::Color(Color::from_rgb8(30, 102, 245)),
                border_width: 1.0,
                border_color: Color::from_rgb8(30, 102, 245),
            },
        },
    }
}
impl slider::Catalog for Theme {
    type Class<'a> = slider::StyleFn<'a, Self>;
    fn default<'a>() -> Self::Class<'a> {
        Box::new(slider_default)
    }
    fn style(&self, class: &Self::Class<'_>, status: slider::Status) -> slider::Style {
        class(self, status)
    }
}
/// Slim download bar: dark track, green fill (matches accent text).
pub fn progress_bar_default(_theme: &Theme) -> progress_bar::Style {
    progress_bar::Style {
        background: Background::Color(Color::from_rgb8(49, 50, 68)),
        bar: Background::Color(Color::from_rgb8(166, 218, 149)),
        border: Border {
            radius: 6.0.into(),
            ..Default::default()
        },
    }
}
impl progress_bar::Catalog for Theme {
    type Class<'a> = progress_bar::StyleFn<'a, Self>;
    fn default<'a>() -> Self::Class<'a> {
        Box::new(progress_bar_default)
    }
    fn style(&self, class: &Self::Class<'_>) -> progress_bar::Style {
        class(self)
    }
}
fn scrollable_style() -> scrollable::Style {
    let rail = scrollable::Rail {
        background: Some(Background::Color(Color::from_rgb(
            0x20 as f32 / 255.0,
            0x22 as f32 / 255.0,
            0x25 as f32 / 255.0,
        ))),
        border: Border {
            radius: 15.0.into(),
            width: 0.0,
            color: Color::TRANSPARENT,
        },
        scroller: scrollable::Scroller {
            background: Background::Color(Color::from_rgb8(205, 214, 244)),
            border: Border {
                radius: 15.0.into(),
                width: 1.0,
                color: Color::TRANSPARENT,
            },
        },
    };
    scrollable::Style {
        container: container::Style::default(),
        vertical_rail: rail,
        horizontal_rail: rail,
        gap: None,
        auto_scroll: scrollable::AutoScroll {
            background: Background::Color(Color::from_rgb8(49, 50, 68)),
            border: Border {
                radius: 15.0.into(),
                ..Default::default()
            },
            shadow: Shadow::default(),
            icon: Color::from_rgb8(205, 214, 244),
        },
    }
}
pub fn scrollable_default(_theme: &Theme, _status: scrollable::Status) -> scrollable::Style {
    scrollable_style()
}
impl scrollable::Catalog for Theme {
    type Class<'a> = scrollable::StyleFn<'a, Self>;
    fn default<'a>() -> Self::Class<'a> {
        Box::new(scrollable_default)
    }
    fn style(&self, class: &Self::Class<'_>, status: scrollable::Status) -> scrollable::Style {
        class(self, status)
    }
}
pub fn menu_default(_theme: &Theme) -> menu::Style {
    menu::Style {
        text_color: Color::from_rgb8(205, 214, 244),
        background: Background::Color(Color::from_rgb8(49, 50, 68)),
        border: Border {
            radius: 15.0.into(),
            width: 0.5,
            color: Color::from_rgb8(30, 102, 245),
        },
        selected_text_color: Color::from_rgb8(205, 214, 244),
        selected_background: Background::Color(Color::from_rgb8(30, 102, 245)),
        shadow: Shadow::default(),
    }
}
impl menu::Catalog for Theme {
    type Class<'a> = menu::StyleFn<'a, Self>;
    fn default<'a>() -> <Self as menu::Catalog>::Class<'a> {
        Box::new(menu_default)
    }
    fn style(&self, class: &<Self as menu::Catalog>::Class<'_>) -> menu::Style {
        class(self)
    }
}
fn toggler_appearance(is_active: bool) -> toggler::Style {
    toggler::Style {
        background: Background::Color(if is_active {
            Color::from_rgb8(30, 102, 245)
        } else {
            Color::from_rgb8(205, 214, 244)
        }),
        background_border_width: 0.0,
        background_border_color: Color::WHITE,
        foreground: Background::Color(if is_active {
            Color::from_rgb8(205, 214, 244)
        } else {
            Color::from_rgb8(88, 91, 112)
        }),
        foreground_border_width: 0.0,
        foreground_border_color: Color::WHITE,
        text_color: None,
        border_radius: None,
        padding_ratio: 0.1,
    }
}
pub fn toggler_default(_theme: &Theme, status: toggler::Status) -> toggler::Style {
    match status {
        toggler::Status::Active { is_toggled } => toggler_appearance(is_toggled),
        toggler::Status::Hovered { is_toggled } => toggler_appearance(is_toggled),
        toggler::Status::Disabled { is_toggled } => toggler_appearance(is_toggled),
    }
}
impl toggler::Catalog for Theme {
    type Class<'a> = toggler::StyleFn<'a, Self>;
    fn default<'a>() -> Self::Class<'a> {
        Box::new(toggler_default)
    }
    fn style(&self, class: &Self::Class<'_>, status: toggler::Status) -> toggler::Style {
        class(self, status)
    }
}
