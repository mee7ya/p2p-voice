use iced::{
    Renderer, Theme,
    widget::{Button, Canvas, ComboBox},
};
use iced_aw::Tabs;

use crate::voice_app::{message::Message, mic_icon::MicIcon, wrapper::DeviceWrapper};

pub type VoiceAppDeviceComboBox<'a> = ComboBox<'a, DeviceWrapper, Message, Theme, Renderer>;
pub type VoiceAppButton<'a> = Button<'a, Message, Theme, Renderer>;
pub type VoiceAppTabBar<'a> = Tabs<'a, Message, String, Theme, Renderer>;
pub type VoiceAppMicIcon = Canvas<MicIcon, Message>;
