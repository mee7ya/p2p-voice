use cpal::{Device, traits::DeviceTrait};

use std::fmt::Debug;

#[derive(Clone)]
pub struct DeviceWrapper(pub Device);

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
