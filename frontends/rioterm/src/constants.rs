#[cfg(not(any(target_os = "macos")))]
pub const PADDING_Y: f32 = 2.0;

#[cfg(target_os = "macos")]
pub const PADDING_Y: f32 = 26.;

#[cfg(target_os = "macos")]
pub const ADDITIONAL_PADDING_Y_ON_UNIFIED_TITLEBAR: f32 = 2.;

#[cfg(target_os = "macos")]
pub const TRAFFIC_LIGHT_PADDING: f64 = 9.;

pub const BELL_DURATION: std::time::Duration = std::time::Duration::from_millis(200);
