use iced::widget::combo_box;

use crate::voice_app::{
    audio::{P2P, SelfListen},
    wrapper::DeviceWrapper,
};

pub struct State {
    pub input_devices: combo_box::State<DeviceWrapper>,
    pub output_devices: combo_box::State<DeviceWrapper>,
    pub input_device: Option<DeviceWrapper>,
    pub output_device: Option<DeviceWrapper>,
    pub self_listen: Option<SelfListen>,
    pub p2p: Option<P2P>,
    pub peer_address: String,
    pub active_tab: String,
}
