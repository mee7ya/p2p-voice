use cpal::{
    Host,
    traits::{DeviceTrait, HostTrait},
};
use iced::{
    Alignment, Element, Padding, Size, Task,
    alignment::Horizontal,
    widget::{button, canvas, column, combo_box, container, row, text, text_input},
};
use tracing::info;

use crate::voice_app::{
    app_tracing::TRACING_TARGET,
    audio,
    message::Message,
    mic_icon::{MIC_ICON_DISABLED, MIC_ICON_ENABLED, MicIcon},
    state::State,
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
        container(column![
            column![
                combo_box(
                    &state.input_devices,
                    "Select input device...",
                    state.input_device.as_ref(),
                    Message::InputDeviceChange,
                ),
                combo_box(
                    &state.output_devices,
                    "Select output device...",
                    state.output_device.as_ref(),
                    Message::OutputDeviceChange,
                ),
                row![
                    button(text!("Test").align_x(Horizontal::Center))
                        .width(60)
                        .on_press(Message::SelfListenPressed),
                    canvas(MicIcon {
                        radius: 10.0,
                        color: if state.self_listen.is_some() {
                            MIC_ICON_ENABLED
                        } else {
                            MIC_ICON_DISABLED
                        }
                    })
                    .width(25)
                    .height(25)
                ]
                .spacing(10)
                .align_y(Alignment::Center),
            ]
            .spacing(10),
            column![
                column![
                    text_input("Peer address...", &state.peer_address)
                        .on_input(Message::PeerAddressChange),
                    button(text!("Connect").align_x(Horizontal::Center))
                        .on_press(Message::SelfListenPressed)
                ]
                .spacing(10)
            ]
            .padding(Padding {
                top: 25.0,
                right: 0.0,
                bottom: 0.0,
                left: 0.0
            })
        ])
        .padding(15)
        .into()
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
            .window_size((400.0, 300.0))
            .antialiasing(true)
            .run_with(Self::init)
            .expect("Failed to run application");
    }
}
