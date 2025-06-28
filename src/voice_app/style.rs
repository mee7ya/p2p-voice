use iced::{
    Background, Border, Color, Length, Shadow, Theme, Vector, border::Radius, widget::button,
};
use iced_aw::tab_bar;

use crate::voice_app::state::State;

pub const THEME: Theme = Theme::Dark;

pub fn theme(_state: &State) -> Theme {
    THEME
}

// Tabs
pub const TABS_HEIGHT: Length = Length::Fixed(25.0);
pub const TABS_TEXT_SIZE: f32 = 12.0;

pub fn tabs_style(theme: &Theme, status: tab_bar::Status) -> tab_bar::Style {
    let extended = theme.extended_palette();
    let mut base: tab_bar::Style = tab_bar::Style {
        background: None,
        border_color: Some(extended.primary.strong.text),
        border_width: 0.7,
        tab_label_background: Background::Color(Color::TRANSPARENT),
        tab_label_border_color: Color::TRANSPARENT,
        tab_label_border_width: 0.0,
        icon_color: Color::TRANSPARENT,
        icon_background: None,
        icon_border_radius: Radius::new(0),
        text_color: extended.primary.strong.text,
    };
    match status {
        tab_bar::Status::Active => {
            base.tab_label_background = Background::Color(extended.primary.strong.color);
        }
        tab_bar::Status::Hovered => {
            base.tab_label_background = Background::Color(extended.primary.base.color);
        }
        _ => (),
    }
    base
}
//

// ComboBox
pub const COMBO_BOX_TEXT_SIZE: f32 = 14.0;
//

// Button
pub const BUTTON_TEXT_SIZE: f32 = 14.0;
pub const SELF_LISTEN_BUTTON_WIDTH: f32 = 55.0;
pub const SELF_LISTEN_BUTTON_HEIGHT: f32 = 25.0;

pub const CONNECT_BUTTON_WIDTH: f32 = 90.0;
pub const CONNECT_BUTTON_HEIGHT: f32 = 90.0;

pub fn connect_button_style(theme: &Theme, status: button::Status) -> button::Style {
    let extended = theme.extended_palette();
    let mut base = button::Style {
        background: Some(Background::Color(extended.primary.strong.color)),
        text_color: extended.primary.strong.text,
        border: Border {
            color: Color::TRANSPARENT,
            radius: Radius::from(CONNECT_BUTTON_HEIGHT / 2.0),
            width: 0.0,
        },
        shadow: Shadow {
            color: Color::TRANSPARENT,
            offset: Vector::from([0.0, 0.0]),
            blur_radius: 0.0,
        },
    };
    match status {
        button::Status::Active => {
            base.background = Some(Background::Color(extended.primary.strong.color));
        }
        button::Status::Hovered => {
            base.background = Some(Background::Color(extended.primary.base.color));
        }
        _ => (),
    }
    base
}
//

// Mic Icon
pub const MIC_ICON_HEIGHT: f32 = 25.0;
pub const MIC_ICON_WIDTH: f32 = 25.0;
//

// Text Input
pub const TEXT_INPUT_SIZE: f32 = 14.0;
