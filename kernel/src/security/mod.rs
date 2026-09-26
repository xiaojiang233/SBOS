#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Capabilities(pub u64);
impl Capabilities {
    pub const NONE: Self = Self(0);
    pub const FILE_READ: Self = Self(1 << 0);
    pub const FILE_WRITE: Self = Self(1 << 1);
    pub const PROCESS_CONTROL: Self = Self(1 << 2);
    pub const DEVICE_QUERY: Self = Self(1 << 3);
    pub const CONFIG_READ: Self = Self(1 << 4);
    pub const CONFIG_WRITE: Self = Self(1 << 5);
    pub const IDENTITY_ADMIN: Self = Self(1 << 6);
    pub const ALL_BOOTSTRAP: Self = Self((1 << 6) - 1);
    pub const ALL_PRIVILEGED: Self = Self((1 << 7) - 1);
    pub const fn contains(self, right: Self) -> bool {
        self.0 & right.0 == right.0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SecurityContext {
    pub capabilities: Capabilities,
    pub application_sandboxed: bool,
}

impl SecurityContext {
    pub const fn bootstrap() -> Self {
        Self {
            capabilities: Capabilities::ALL_PRIVILEGED,
            application_sandboxed: false,
        }
    }
    pub const fn root() -> Self {
        Self::bootstrap()
    }
    pub const fn for_uid(uid: u32) -> Self {
        if uid == 0 {
            Self::root()
        } else {
            Self::interactive_shell()
        }
    }
    pub const fn interactive_shell() -> Self {
        Self {
            capabilities: Capabilities::ALL_BOOTSTRAP,
            application_sandboxed: true,
        }
    }
    pub const fn restricted() -> Self {
        Self {
            capabilities: Capabilities::FILE_READ,
            application_sandboxed: true,
        }
    }
}
