pub fn write(bytes: &[u8]) {
    super::serial::write(bytes);
    super::framebuffer::write(bytes);
}
