use iced::{Background, Color, Length, Theme, border::Radius};
use iced_aw::{style::Status, tab_bar};

use crate::voice_app::state::State;

pub const THEME: Theme = Theme::Dark;

pub fn theme(_state: &State) -> Theme {
    THEME
}

// Tabs
pub const TABS_HEIGHT: Length = Length::Fixed(25.0);
pub const TABS_TEXT_SIZE: f32 = 12.0;

pub fn tabs_style(theme: &Theme, status: Status) -> tab_bar::Style {
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
        Status::Active => {
            base.tab_label_background = Background::Color(extended.primary.strong.color);
        }
        Status::Hovered => {
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
pub const CONNECT_BUTTON_HEIGHT: f32 = 40.0;
//

// Mic Icon
pub const MIC_ICON_HEIGHT: f32 = 25.0;
pub const MIC_ICON_WIDTH: f32 = 25.0;
//
