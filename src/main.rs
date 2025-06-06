#![windows_subsystem = "windows"]

mod voice_app;

use tracing::info;

use crate::voice_app::app_tracing::{self, TRACING_TARGET};
use crate::voice_app::voice_app::VoiceApp;

fn main() {
    app_tracing::init();
    info!(target: TRACING_TARGET, "Starting app...");
    VoiceApp::new((400.0, 300.0)).run();
}
