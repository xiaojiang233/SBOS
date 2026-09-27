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
    pub fn surface_info() -> Option<(u32, u32, u32)> { None }
    pub fn fill_rect(_x: u32, _y: u32, _width: u32, _height: u32, _color: u32) -> bool { false }
    pub fn blit_rect(_x: u32, _y: u32, _width: u32, _height: u32, _stride: usize, _pixels: &[u32]) -> bool { false }
}
#[cfg(feature = "driver-e1000")]
pub mod e1000;
#[cfg(feature = "driver-ps2")]
pub mod keyboard;
#[cfg(feature = "driver-ps2-mouse")]
pub mod mouse;
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
