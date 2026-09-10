#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "linux")]
pub use linux::ScreenCapture;
#[cfg(target_os = "macos")]
pub use macos::ScreenCapture;

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
compile_error!("VibesVR atualmente suporta apenas Linux e macOS");
