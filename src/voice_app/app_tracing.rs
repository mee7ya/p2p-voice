use std::{fs::File, panic};

use tracing::{Level, error};
use tracing_subscriber::{
    filter::Targets, fmt::layer, layer::SubscriberExt, registry, util::SubscriberInitExt,
};

pub const TRACING_TARGET: &str = "app";

pub fn init() {
    let file: File = File::create("app.log").unwrap();

    let layer = layer().compact().with_ansi(false).with_writer(file);
    registry()
        .with(layer)
        .with(Targets::default().with_target(TRACING_TARGET, Level::INFO))
        .init();

    panic::set_hook(Box::new(|panic_info| {
        error!(target: TRACING_TARGET, "{panic_info}");
    }));
}
