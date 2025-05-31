#![windows_subsystem = "windows"]

mod audio;

use std::fmt::Debug;
use std::fs::File;
use std::panic;

use audio::SelfListen;
use cpal::traits::{DeviceTrait, HostTrait};
use cpal::{Device, Host};
use iced::Alignment;
use iced::alignment::Horizontal;
use iced::mouse::Cursor;
use iced::widget::{button, canvas, column, combo_box, container, row, text};
use iced::{Color, Element, Rectangle, Renderer, Task, Theme, color};
use tracing::{Level, error, info};
use tracing_subscriber::filter::Targets;
use tracing_subscriber::fmt::layer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::registry;
use tracing_subscriber::util::SubscriberInitExt;

const TRACING_TARGET: &str = "app";
const MIC_ICON_DISABLED: Color = color!(104.0, 104.0, 104.0);
const MIC_ICON_ENABLED: Color = color!(0.0, 128.0, 0.0);

#[derive(Clone)]
struct DeviceWrapper(Device);

impl std::fmt::Display for DeviceWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0.name().unwrap_or(String::from("Unknown")).as_str())
    }
}

impl Debug for DeviceWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("DeviceWrapper")
            .field(&self.0.name())
            .finish()
    }
}

#[derive(Debug)]
struct MicIcon {
    radius: f32,
    color: Color,
}

impl<Message> canvas::Program<Message> for MicIcon {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let circle = canvas::Path::circle(frame.center(), self.radius);
        frame.fill(&circle, self.color);
        vec![frame.into_geometry()]
    }
}

#[derive(Debug, Clone)]
enum Message {
    InputDeviceChange(DeviceWrapper),
    OutputDeviceChange(DeviceWrapper),
    SelfListenPressed,
}

struct State {
    input_devices: combo_box::State<DeviceWrapper>,
    output_devices: combo_box::State<DeviceWrapper>,
    input_device: Option<DeviceWrapper>,
    output_device: Option<DeviceWrapper>,
    self_listen: Option<SelfListen>,
}

fn view(state: &State) -> Element<Message> {
    container(
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
    )
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

fn main() {
    let file: File = File::create("app.log").unwrap();

    let layer = layer().compact().with_ansi(false).with_writer(file);
    registry()
        .with(layer)
        .with(Targets::default().with_target(TRACING_TARGET, Level::INFO))
        .init();

    panic::set_hook(Box::new(|panic_info| {
        error!(target: TRACING_TARGET, "{panic_info}");
    }));

    info!(target: TRACING_TARGET, "Starting app...");
    iced::application("Voice", update, view)
        .window_size((400.0, 300.0))
        .antialiasing(true)
        .run_with(init)
        .expect("Failed to run application");
}
