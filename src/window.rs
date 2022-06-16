use bevy::{prelude::*, window::WindowMode};
use bevy_pkv::PkvStore;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default, PartialEq, Eq)]
pub enum WindowModeSerial {
    Fullscreen,
    #[default]
    Windowed,
}

impl From<WindowModeSerial> for WindowMode {
    fn from(mode: WindowModeSerial) -> Self {
        match mode {
            WindowModeSerial::Fullscreen => WindowMode::BorderlessFullscreen,
            WindowModeSerial::Windowed => WindowMode::Windowed,
        }
    }
}

impl From<WindowMode> for WindowModeSerial {
    fn from(mode: WindowMode) -> Self {
        match mode {
            WindowMode::BorderlessFullscreen => WindowModeSerial::Fullscreen,
            WindowMode::Windowed => WindowModeSerial::Windowed,
            _ => WindowModeSerial::Windowed,
        }
    }
}

pub fn descriptor_from_settings(settings: &PkvStore) -> WindowDescriptor {
    let (width, height) = match settings.get("window size") {
        Ok(d) => d,
        Err(_) => (1280.0, 720.0),
    };
    debug!("read: {}/{}", width, height);
    let window_pos = settings.get("window pos").ok();
    let mode = settings
        .get::<WindowModeSerial>("window mode")
        .unwrap_or_default()
        .into();

    let window_descriptor = WindowDescriptor {
        width,
        height,
        position: window_pos,
        mode,
        cursor_visible: false,
        title: "Measure Once".into(),
        ..Default::default()
    };

    debug!("window desc: {:?}", window_descriptor);
    window_descriptor
}

pub fn update_window(settings: &PkvStore, window: &mut Window) {
    let desc = descriptor_from_settings(settings);

    window.set_mode(desc.mode);
    if desc.mode == WindowMode::Windowed {
        if let Some(pos) = desc.position {
            window.set_position(pos.as_ivec2());
        }
        window.set_resolution(0.0, 0.0); // force resize
        window.set_resolution(desc.width, desc.height);
    }
}
