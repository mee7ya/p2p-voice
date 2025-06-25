use cpal::{
    Host,
    traits::{DeviceTrait, HostTrait},
};
use iced::{
    Alignment, Element, Size, Task,
    alignment::{Horizontal, Vertical},
    widget::{
        button, canvas, column, combo_box, container, horizontal_rule, row, text, text_input,
    },
};
use iced_aw::{TabLabel, Tabs};
use tracing::info;

use crate::voice_app::{
    app_tracing::TRACING_TARGET,
    app_type::{VoiceAppButton, VoiceAppDeviceComboBox, VoiceAppMicIcon, VoiceAppTabBar},
    audio,
    message::Message,
    mic_icon::{MIC_ICON_DISABLED, MIC_ICON_ENABLED, MicIcon},
    state::State,
    style::{
        BUTTON_TEXT_SIZE, COMBO_BOX_TEXT_SIZE, CONNECT_BUTTON_HEIGHT, CONNECT_BUTTON_WIDTH,
        MIC_ICON_HEIGHT, MIC_ICON_WIDTH, SELF_LISTEN_BUTTON_HEIGHT, SELF_LISTEN_BUTTON_WIDTH,
        TABS_HEIGHT, TABS_TEXT_SIZE, tabs_style, theme,
    },
    wrapper::DeviceWrapper,
};

pub struct VoiceApp {
    pub window_size: Size,
}

impl VoiceApp {
    pub fn new(window_size: impl Into<Size>) -> Self {
        VoiceApp {
            window_size: window_size.into(),
        }
    }

    fn init() -> (State, Task<Message>) {
        let host: Host = cpal::default_host();
        let state: State = State {
            input_devices: combo_box::State::<DeviceWrapper>::new(
                host.input_devices()
                    .expect("Failed to get input devices")
                    .map(|x| DeviceWrapper(x))
                    .collect(),
            ),
            output_devices: combo_box::State::<DeviceWrapper>::new(
                host.output_devices()
                    .expect("Failed to get output devices")
                    .map(|x| DeviceWrapper(x))
                    .collect(),
            ),
            input_device: host
                .default_input_device()
                .and_then(|x| Some(DeviceWrapper(x))),
            output_device: host
                .default_output_device()
                .and_then(|x| Some(DeviceWrapper(x))),
            self_listen: None,
            peer_address: String::new(),
            active_tab: String::from("Action"),
        };
        info!(
            target: TRACING_TARGET,
            "Init done.\nInput devices:\n    {}\nOutput devices:\n    {}",
            state.input_devices.options()
            .into_iter()
            .map(|x| x.0.name().unwrap_or(String::from("Unknown")))
            .collect::<Vec<String>>()
            .join("\n    "),
            state.output_devices.options()
            .into_iter()
            .map(|x| x.0.name().unwrap_or(String::from("Unknown")))
            .collect::<Vec<String>>()
            .join("\n    "),
        );
        (state, Task::<Message>::none())
    }

    fn view(state: &State) -> Element<Message> {
        let input_combo_box: VoiceAppDeviceComboBox = combo_box(
            &state.input_devices,
            "Select input device...",
            state.input_device.as_ref(),
            Message::InputDeviceChange,
        )
        .size(COMBO_BOX_TEXT_SIZE);

        let output_combo_box: VoiceAppDeviceComboBox = combo_box(
            &state.output_devices,
            "Select output device...",
            state.output_device.as_ref(),
            Message::OutputDeviceChange,
        )
        .size(COMBO_BOX_TEXT_SIZE);

        let test_button: VoiceAppButton = button(
            text!("Test")
                .size(BUTTON_TEXT_SIZE)
                .align_x(Horizontal::Center)
                .align_y(Vertical::Center),
        )
        .width(SELF_LISTEN_BUTTON_WIDTH)
        .height(SELF_LISTEN_BUTTON_HEIGHT)
        .on_press(Message::SelfListenPressed);

        let mic_icon: VoiceAppMicIcon = canvas(MicIcon {
            radius: 10.0,
            color: if state.self_listen.is_some() {
                MIC_ICON_ENABLED
            } else {
                MIC_ICON_DISABLED
            },
        })
        .width(MIC_ICON_WIDTH)
        .height(MIC_ICON_HEIGHT);

        let connect_button: VoiceAppButton = button(
            text!("Connect")
                .size(BUTTON_TEXT_SIZE)
                .align_x(Horizontal::Center)
                .align_y(Vertical::Center),
        )
        .width(CONNECT_BUTTON_WIDTH)
        .height(CONNECT_BUTTON_HEIGHT)
        .on_press(Message::PeerConnect);

        let tabs: VoiceAppTabBar = Tabs::new(Message::TabSelected)
            .push(
                String::from("Main"),
                TabLabel::Text(String::from("Main")),
                column![
                    row![connect_button]
                        .height(iced::Length::Fill)
                        .align_y(Vertical::Center)
                ]
                .width(iced::Length::Fill)
                .align_x(Horizontal::Center),
            )
            .push(
                String::from("Settings"),
                TabLabel::Text(String::from("Settings")),
                column![
                    input_combo_box,
                    output_combo_box,
                    row![test_button, mic_icon]
                        .spacing(10)
                        .align_y(Alignment::Center),
                    horizontal_rule(2),
                    text_input("Peer address...", &state.peer_address)
                        .on_input(Message::PeerAddressChange),
                ]
                .padding(10)
                .spacing(10),
            )
            .tab_bar_style(tabs_style)
            .tab_bar_height(TABS_HEIGHT)
            .text_size(TABS_TEXT_SIZE)
            .tab_label_padding(0)
            .set_active_tab(&state.active_tab);

        let app_container = container(tabs);

        app_container.into()
    }

    fn update(state: &mut State, message: Message) {
        match message {
            Message::InputDeviceChange(device) => {
                state.input_device = Some(device);
                info!(target: TRACING_TARGET, "Input device changed to: {}", state.input_device.as_ref().unwrap().0.name().unwrap_or(String::from("Unknown")));
            }
            Message::OutputDeviceChange(device) => {
                state.output_device = Some(device);
                info!(target: TRACING_TARGET, "Output device changed to: {}", state.output_device.as_ref().unwrap().0.name().unwrap_or(String::from("Unknown")));
            }
            Message::PeerAddressChange(peer_address) => {
                state.peer_address = peer_address;
            }
            Message::PeerConnect => {}
            Message::TabSelected(tab) => {
                state.active_tab = tab;
            }
            Message::SelfListenPressed => {
                if state.self_listen.is_none() {
                    info!(target: TRACING_TARGET, "Attempting to create streams...");

                    state.self_listen = Some(audio::SelfListen::new(
                        &state.input_device.as_ref().unwrap().0,
                        &state.output_device.as_ref().unwrap().0,
                    ));
                } else {
                    info!(target: TRACING_TARGET, "Dropping streams");
                    state.self_listen = None;
                }
            }
        }
    }

    pub fn run(&self) {
        iced::application("Voice", Self::update, Self::view)
            .theme(theme)
            .window_size(self.window_size)
            .antialiasing(true)
            .run_with(Self::init)
            .expect("Failed to run application");
    }
}
