pub mod console;
#[cfg(feature = "driver-ata")]
pub mod ata;
#[cfg(feature = "driver-framebuffer")]
pub mod framebuffer;
#[cfg(not(feature = "driver-framebuffer"))]
pub mod framebuffer {
    #[derive(Clone, Copy)]
    pub struct FramebufferInfo {
        pub base: u64,
        pub size: u64,
        pub width: usize,
        pub height: usize,
        pub stride: usize,
        pub pixel_format: u32,
    }
    pub fn init(_info: FramebufferInfo) {}
    pub fn write(_bytes: &[u8]) {}
}
#[cfg(feature = "driver-e1000")]
pub mod e1000;
#[cfg(feature = "driver-ps2")]
pub mod keyboard;
#[cfg(feature = "driver-pci")]
pub mod pci;
pub mod serial;
pub mod tty;
#[cfg(feature = "driver-rtc")]
pub mod rtc;

pub fn read_key() -> Option<u8> {
    #[cfg(feature = "driver-ps2")]
    { return keyboard::try_read_byte(); }
    #[cfg(not(feature = "driver-ps2"))]
    { None }
}
