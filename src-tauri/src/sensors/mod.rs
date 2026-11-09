//! Sensor subsystem: real telemetry backends for Snapdragon X.
pub mod chips;
pub mod core_tiers;
pub mod identity;
pub mod pdh;
pub mod tray;
#[cfg(target_os = "windows")]
pub mod tray_render;
pub mod types;

pub use chips::ChipProfile;
pub use pdh::PdhProvider;
pub use types::SensorSnapshot;
