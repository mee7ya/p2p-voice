use crate::voice_app::wrapper::DeviceWrapper;

#[derive(Debug, Clone)]
pub enum Message {
    InputDeviceChange(DeviceWrapper),
    OutputDeviceChange(DeviceWrapper),
    PeerAddressChange(String),
    PeerConnect,
    SelfListenPressed,
}
