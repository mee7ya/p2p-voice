use iced::{
    Renderer, Theme,
    widget::{Button, ComboBox},
};
use iced_aw::{TabBar, Tabs};

use crate::voice_app::{message::Message, wrapper::DeviceWrapper};

pub type VoiceAppComboBox<'a> = ComboBox<'a, DeviceWrapper, Message, Theme, Renderer>;
pub type VoiceAppButton<'a> = Button<'a, Message, Theme, Renderer>;
pub type VoiceAppTabBar<'a> = Tabs<'a, Message, String, Theme, Renderer>;
