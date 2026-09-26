use alloc::string::String;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NodeId(pub [u8; 16]);
impl NodeId {
    pub const fn tmpfs(id: u64) -> Self {
        let low = id.to_le_bytes();
        let mut bytes = [0u8; 16];
        let mut index = 0;
        while index < 8 {
            bytes[index + 8] = low[index];
            index += 1;
        }
        Self(bytes)
    }

    pub fn handle(self) -> u64 {
        u64::from_le_bytes(self.0[8..16].try_into().unwrap_or([0; 8]))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AclPrincipal {
    Owner,
    Group,
    Everyone,
    User(u32),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AclEntry {
    pub principal: AclPrincipal,
    pub rights: u16,
}
impl AclEntry {
    pub const EMPTY: Self = Self { principal: AclPrincipal::Everyone, rights: 0 };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Acl {
    pub entries: [AclEntry; 4],
    pub count: u8,
}
impl Acl {
    pub const READ: u16 = 1;
    pub const WRITE: u16 = 2;
    pub const EXECUTE: u16 = 4;
    pub const EMPTY: Self = Self { entries: [AclEntry::EMPTY; 4], count: 0 };
    pub const OWNER_DEFAULT: Self = Self {
        entries: [
            AclEntry { principal: AclPrincipal::Owner, rights: 7 },
            AclEntry { principal: AclPrincipal::Group, rights: 5 },
            AclEntry { principal: AclPrincipal::Everyone, rights: 5 },
            AclEntry::EMPTY,
        ],
        count: 3,
    };
    pub const SHARED_DEFAULT: Self = Self {
        entries: [
            AclEntry { principal: AclPrincipal::Owner, rights: 7 },
            AclEntry { principal: AclPrincipal::Group, rights: 7 },
            AclEntry { principal: AclPrincipal::Everyone, rights: 7 },
            AclEntry::EMPTY,
        ],
        count: 3,
    };

    pub fn allows(self, owner: u32, group: u32, uid: u32, gid: u32, requested: u16) -> bool {
        let principal = if self.entries[..self.count as usize]
            .iter()
            .any(|entry| entry.principal == AclPrincipal::User(uid))
        {
            AclPrincipal::User(uid)
        } else if uid == owner {
            AclPrincipal::Owner
        } else if gid == group {
            AclPrincipal::Group
        } else {
            AclPrincipal::Everyone
        };
        let rights = self.entries[..self.count as usize]
            .iter()
            .find(|entry| entry.principal == principal)
            .map(|entry| entry.rights)
            .unwrap_or(0);
        rights & requested == requested
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VNodeKind {
    File,
    Directory,
}

#[derive(Clone, Copy, Debug)]
pub struct VNodeMetadata {
    pub id: u64,
    pub node_id: NodeId,
    pub kind: VNodeKind,
    pub size: usize,
    pub executable: bool,
    pub uid: u32,
    pub gid: u32,
    pub acl: Acl,
    pub created: u64,
    pub modified: u64,
    pub accessed: u64,
    pub flags: u32,
}

pub struct VNode {
    pub id: u64,
    pub node_id: NodeId,
    pub parent: Option<u64>,
    pub name: String,
    pub kind: VNodeKind,
    pub executable: bool,
    pub uid: u32,
    pub gid: u32,
    pub acl: Acl,
    pub created: u64,
    pub modified: u64,
    pub accessed: u64,
    pub flags: u32,
    pub(crate) data: alloc::vec::Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Path {
    value: alloc::string::String,
}
impl Path {
    pub fn new(value: &str) -> Result<Self, &'static str> {
        if value.is_empty() || value.len() > 512 || value.as_bytes().contains(&0) {
            return Err("invalid path");
        }
        if !value.starts_with('/') && !value.starts_with('.') {
            return Err("path must be absolute or relative");
        }
        Ok(Self {
            value: value.into(),
        })
    }
    pub fn as_str(&self) -> &str {
        &self.value
    }
}
