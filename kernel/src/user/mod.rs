//! System identity manager. Identity records are persisted through VFS/SBFS,
//! not exposed as configuration or as an `/etc` tree.

use crate::fs::vfs;
use crate::fs::vnode::{Acl, AclEntry, AclPrincipal, VNodeKind};
use crate::sync::SpinLock;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

pub const ROOT_UID: u32 = 0;
pub const ROOT_GID: u32 = 0;
pub const GUEST_UID: u32 = 1000;
pub const GUEST_GID: u32 = 1000;
pub const ROOT_ACCOUNT: &str = "root";
pub const GUEST_ACCOUNT: &str = "guest";

const DB_PATH: &str = "/System/Accounts.db";
const DB_MAGIC: &[u8; 8] = b"SBOSID1\0";
const DB_VERSION: u32 = 1;
const DB_SIZE: usize = 4096;
const DB_HEADER_SIZE: usize = 32;
const MAX_USERS: usize = 16;
const MAX_GROUPS: usize = 16;
const USER_RECORD_SIZE: usize = 160;
const GROUP_RECORD_SIZE: usize = 64;
const USER_NAME_SIZE: usize = 48;
const HOME_SIZE: usize = 96;
const GROUP_NAME_SIZE: usize = 58;
const USER_FLAG_ADMIN: u32 = 1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UserAccount {
    pub uid: u32,
    pub primary_gid: u32,
    pub name: String,
    pub home: String,
    pub administrator: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupAccount {
    pub gid: u32,
    pub name: String,
}

struct State {
    node_id: u64,
    users: Vec<UserAccount>,
    groups: Vec<GroupAccount>,
}
impl State {
    const fn empty() -> Self {
        Self {
            node_id: 0,
            users: Vec::new(),
            groups: Vec::new(),
        }
    }
}
static STATE: SpinLock<State> = SpinLock::new(State::empty());

pub fn init() -> Result<(), &'static str> {
    let filesystem = vfs::fs();
    let node_id = match filesystem.resolve(DB_PATH, "/") {
        Ok(id) => id,
        Err(_) => vfs::create_file(DB_PATH, "/", false)?,
    };
    if filesystem.kind(node_id) != Some(VNodeKind::File) {
        return Err("identity database path is not a file");
    }

    let mut database = [0u8; DB_SIZE];
    let metadata = filesystem.metadata(node_id).ok_or("identity database metadata is missing")?;
    let (users, groups) = if metadata.size == 0 {
        let (users, groups) = defaults();
        encode_database(&users, &groups, &mut database)?;
        filesystem
            .write_at(node_id, 0, &database)
            .map_err(|_| "failed to initialize identity database")?;
        (users, groups)
    } else {
        if metadata.size != DB_SIZE {
            return Err("identity database has an unsupported size");
        }
        let read = filesystem
            .read_at(node_id, 0, &mut database)
            .map_err(|_| "failed to read identity database")?;
        if read != DB_SIZE {
            return Err("short read from identity database");
        }
        decode_database(&database)?
    };

    filesystem
        .set_owner_acl(node_id, ROOT_UID, ROOT_GID, root_private_acl())
        .map_err(|_| "failed to protect identity database")?;
    ensure_account_homes(&users)?;
    secure_system_layout()?;
    *STATE.lock() = State { node_id, users, groups };
    crate::kprintln!("identity: {} accounts loaded; default user is root (uid 0)", STATE.lock().users.len());
    Ok(())
}

pub fn account_by_uid(uid: u32) -> Option<UserAccount> {
    STATE.lock().users.iter().find(|user| user.uid == uid).cloned()
}

pub fn account_by_name(name: &str) -> Option<UserAccount> {
    STATE.lock().users.iter().find(|user| user.name == name).cloned()
}

pub fn group_by_gid(gid: u32) -> Option<GroupAccount> {
    STATE.lock().groups.iter().find(|group| group.gid == gid).cloned()
}

pub fn users() -> Vec<UserAccount> {
    STATE.lock().users.clone()
}

pub fn groups() -> Vec<GroupAccount> {
    STATE.lock().groups.clone()
}

pub fn default_credentials() -> (u32, u32) {
    account_by_name(ROOT_ACCOUNT)
        .map(|user| (user.uid, user.primary_gid))
        .unwrap_or((ROOT_UID, ROOT_GID))
}

pub fn is_administrator(uid: u32) -> bool {
    account_by_uid(uid).map(|user| user.administrator).unwrap_or(false)
}

pub fn create_group(actor_uid: u32, gid: u32, name: &str) -> Result<(), &'static str> {
    require_root(actor_uid)?;
    validate_name(name, GROUP_NAME_SIZE - 1)?;
    let mut state = STATE.lock();
    if state.groups.len() >= MAX_GROUPS {
        return Err("group database is full");
    }
    if state.groups.iter().any(|group| group.gid == gid || group.name == name) {
        return Err("group ID or name already exists");
    }
    let mut next = State {
        node_id: state.node_id,
        users: state.users.clone(),
        groups: state.groups.clone(),
    };
    next.groups.push(GroupAccount { gid, name: String::from(name) });
    persist(&next)?;
    state.groups = next.groups;
    Ok(())
}

pub fn create_user(
    actor_uid: u32,
    uid: u32,
    primary_gid: u32,
    name: &str,
    home: &str,
) -> Result<(), &'static str> {
    require_root(actor_uid)?;
    validate_name(name, USER_NAME_SIZE - 1)?;
    if uid == ROOT_UID || !valid_home(home) {
        return Err("invalid user ID or home path");
    }
    let mut state = STATE.lock();
    if state.users.len() >= MAX_USERS {
        return Err("user database is full");
    }
    if state.users.iter().any(|user| user.uid == uid || user.name == name || user.home == home) {
        return Err("user ID, name, or home path already exists");
    }
    if !state.groups.iter().any(|group| group.gid == primary_gid) {
        return Err("primary group does not exist");
    }
    ensure_directory(home)?;
    let home_acl = account_home_acl(uid);
    vfs::set_owner_acl(home, "/", uid, primary_gid, home_acl)?;

    let mut next = State {
        node_id: state.node_id,
        users: state.users.clone(),
        groups: state.groups.clone(),
    };
    next.users.push(UserAccount {
        uid,
        primary_gid,
        name: String::from(name),
        home: String::from(home),
        administrator: false,
    });
    persist(&next)?;
    state.users = next.users;
    Ok(())
}

pub fn delete_user(actor_uid: u32, uid: u32) -> Result<(), &'static str> {
    require_root(actor_uid)?;
    if uid == ROOT_UID {
        return Err("root account cannot be removed");
    }
    let mut state = STATE.lock();
    if !state.users.iter().any(|user| user.uid == uid) {
        return Err("user does not exist");
    }
    let mut next = State {
        node_id: state.node_id,
        users: state.users.iter().filter(|user| user.uid != uid).cloned().collect(),
        groups: state.groups.clone(),
    };
    persist(&next)?;
    state.users = core::mem::take(&mut next.users);
    Ok(())
}

fn require_root(actor_uid: u32) -> Result<(), &'static str> {
    if actor_uid == ROOT_UID && is_administrator(actor_uid) {
        Ok(())
    } else {
        Err("root administrator privileges are required")
    }
}

fn persist(state: &State) -> Result<(), &'static str> {
    let mut database = [0u8; DB_SIZE];
    encode_database(&state.users, &state.groups, &mut database)?;
    vfs::fs()
        .write_at(state.node_id, 0, &database)
        .map_err(|_| "failed to persist identity database")?;
    Ok(())
}

fn defaults() -> (Vec<UserAccount>, Vec<GroupAccount>) {
    (
        vec![
            UserAccount {
                uid: ROOT_UID,
                primary_gid: ROOT_GID,
                name: String::from(ROOT_ACCOUNT),
                home: String::from("/Users/Root"),
                administrator: true,
            },
            UserAccount {
                uid: GUEST_UID,
                primary_gid: GUEST_GID,
                name: String::from(GUEST_ACCOUNT),
                home: String::from("/Users/Guest"),
                administrator: false,
            },
        ],
        vec![
            GroupAccount { gid: ROOT_GID, name: String::from("root") },
            GroupAccount { gid: GUEST_GID, name: String::from("users") },
        ],
    )
}

fn ensure_account_homes(users: &[UserAccount]) -> Result<(), &'static str> {
    for user in users {
        if !valid_home(&user.home) {
            return Err("account database contains an invalid home path");
        }
        ensure_directory(&user.home)?;
        vfs::set_owner_acl(
            &user.home,
            "/",
            user.uid,
            user.primary_gid,
            account_home_acl(user.uid),
        )?;
    }
    Ok(())
}

fn ensure_directory(path: &str) -> Result<(), &'static str> {
    match vfs::fs().resolve(path, "/") {
        Ok(id) if vfs::fs().kind(id) == Some(VNodeKind::Directory) => Ok(()),
        Ok(_) => Err("account home exists but is not a directory"),
        Err(_) => {
            vfs::create_directory(path, "/")?;
            Ok(())
        }
    }
}

fn secure_system_layout() -> Result<(), &'static str> {
    let protected = [
        ("/", ROOT_UID, ROOT_GID, system_directory_acl()),
        ("/Applications", ROOT_UID, ROOT_GID, system_directory_acl()),
        ("/System", ROOT_UID, ROOT_GID, system_directory_acl()),
        ("/Users", ROOT_UID, ROOT_GID, system_directory_acl()),
        ("/Shared", ROOT_UID, ROOT_GID, shared_directory_acl()),
        ("/Volumes", ROOT_UID, ROOT_GID, system_directory_acl()),
        ("/System/Readme.txt", ROOT_UID, ROOT_GID, system_file_acl()),
        ("/Applications/bash", ROOT_UID, ROOT_GID, system_file_acl()),
        ("/Applications/posix-probe", ROOT_UID, ROOT_GID, system_file_acl()),
        ("/Applications/bash", ROOT_UID, ROOT_GID, system_file_acl()),
    ];
    for (path, uid, gid, acl) in protected {
        if vfs::fs().resolve(path, "/").is_ok() {
            vfs::set_owner_acl(path, "/", uid, gid, acl)?;
        }
    }
    Ok(())
}

fn root_private_acl() -> Acl {
    Acl {
        entries: [AclEntry { principal: AclPrincipal::Owner, rights: 7 }, AclEntry::EMPTY, AclEntry::EMPTY, AclEntry::EMPTY],
        count: 1,
    }
}

fn system_directory_acl() -> Acl {
    Acl {
        entries: [
            AclEntry { principal: AclPrincipal::Owner, rights: 7 },
            AclEntry { principal: AclPrincipal::Everyone, rights: 5 },
            AclEntry::EMPTY,
            AclEntry::EMPTY,
        ],
        count: 2,
    }
}

fn system_file_acl() -> Acl {
    system_directory_acl()
}

fn user_home_acl() -> Acl {
    Acl {
        entries: [
            AclEntry { principal: AclPrincipal::Owner, rights: 7 },
            AclEntry::EMPTY,
            AclEntry::EMPTY,
            AclEntry::EMPTY,
        ],
        count: 1,
    }
}

fn shared_directory_acl() -> Acl {
    Acl {
        entries: [
            AclEntry { principal: AclPrincipal::Owner, rights: 7 },
            AclEntry { principal: AclPrincipal::Everyone, rights: 7 },
            AclEntry::EMPTY,
            AclEntry::EMPTY,
        ],
        count: 2,
    }
}

fn account_home_acl(uid: u32) -> Acl {
    if uid == ROOT_UID { root_private_acl() } else { user_home_acl() }
}

fn encode_database(users: &[UserAccount], groups: &[GroupAccount], output: &mut [u8; DB_SIZE]) -> Result<(), &'static str> {
    if users.len() > MAX_USERS || groups.len() > MAX_GROUPS {
        return Err("identity database capacity exceeded");
    }
    output.fill(0);
    output[0..8].copy_from_slice(DB_MAGIC);
    put_u32(output, 8, DB_VERSION);
    put_u16(output, 12, users.len() as u16);
    put_u16(output, 14, groups.len() as u16);
    put_u32(output, 24, DB_SIZE as u32);
    put_u32(output, 28, USER_RECORD_SIZE as u32);
    for (index, user) in users.iter().enumerate() {
        let offset = DB_HEADER_SIZE + index * USER_RECORD_SIZE;
        put_u32(output, offset, user.uid);
        put_u32(output, offset + 4, user.primary_gid);
        put_u32(output, offset + 8, if user.administrator { USER_FLAG_ADMIN } else { 0 });
        put_u16(output, offset + 12, user.name.len() as u16);
        put_u16(output, offset + 14, user.home.len() as u16);
        output[offset + 16..offset + 16 + user.name.len()].copy_from_slice(user.name.as_bytes());
        output[offset + 64..offset + 64 + user.home.len()].copy_from_slice(user.home.as_bytes());
    }
    let groups_start = DB_HEADER_SIZE + MAX_USERS * USER_RECORD_SIZE;
    for (index, group) in groups.iter().enumerate() {
        let offset = groups_start + index * GROUP_RECORD_SIZE;
        put_u32(output, offset, group.gid);
        put_u16(output, offset + 4, group.name.len() as u16);
        output[offset + 6..offset + 6 + group.name.len()].copy_from_slice(group.name.as_bytes());
    }
    let checksum = hash_bytes(0xcbf29ce484222325, &output[DB_HEADER_SIZE..]);
    put_u64(output, 16, checksum);
    Ok(())
}

fn decode_database(input: &[u8; DB_SIZE]) -> Result<(Vec<UserAccount>, Vec<GroupAccount>), &'static str> {
    if &input[0..8] != DB_MAGIC
        || get_u32(input, 8) != DB_VERSION
        || get_u32(input, 24) as usize != DB_SIZE
        || get_u32(input, 28) as usize != USER_RECORD_SIZE
    {
        return Err("identity database header is invalid");
    }
    let user_count = get_u16(input, 12) as usize;
    let group_count = get_u16(input, 14) as usize;
    if user_count == 0 || user_count > MAX_USERS || group_count == 0 || group_count > MAX_GROUPS {
        return Err("identity database record count is invalid");
    }
    if hash_bytes(0xcbf29ce484222325, &input[DB_HEADER_SIZE..]) != get_u64(input, 16) {
        return Err("identity database checksum mismatch");
    }
    let mut users = Vec::with_capacity(user_count);
    for index in 0..user_count {
        let offset = DB_HEADER_SIZE + index * USER_RECORD_SIZE;
        let name_len = get_u16(input, offset + 12) as usize;
        let home_len = get_u16(input, offset + 14) as usize;
        if name_len == 0 || name_len >= USER_NAME_SIZE || home_len == 0 || home_len >= HOME_SIZE {
            return Err("identity database contains an invalid user record");
        }
        let name = core::str::from_utf8(&input[offset + 16..offset + 16 + name_len])
            .map_err(|_| "identity database user name is not UTF-8")?;
        let home = core::str::from_utf8(&input[offset + 64..offset + 64 + home_len])
            .map_err(|_| "identity database home path is not UTF-8")?;
        validate_name(name, USER_NAME_SIZE - 1)?;
        if !valid_home(home) {
            return Err("identity database home path is invalid");
        }
        users.push(UserAccount {
            uid: get_u32(input, offset),
            primary_gid: get_u32(input, offset + 4),
            name: String::from(name),
            home: String::from(home),
            administrator: get_u32(input, offset + 8) & USER_FLAG_ADMIN != 0,
        });
    }
    let groups_start = DB_HEADER_SIZE + MAX_USERS * USER_RECORD_SIZE;
    let mut groups = Vec::with_capacity(group_count);
    for index in 0..group_count {
        let offset = groups_start + index * GROUP_RECORD_SIZE;
        let name_len = get_u16(input, offset + 4) as usize;
        if name_len == 0 || name_len >= GROUP_NAME_SIZE {
            return Err("identity database contains an invalid group record");
        }
        let name = core::str::from_utf8(&input[offset + 6..offset + 6 + name_len])
            .map_err(|_| "identity database group name is not UTF-8")?;
        validate_name(name, GROUP_NAME_SIZE - 1)?;
        groups.push(GroupAccount { gid: get_u32(input, offset), name: String::from(name) });
    }
    if !users.iter().any(|user| user.uid == ROOT_UID && user.name == ROOT_ACCOUNT && user.administrator)
        || !users.iter().any(|user| user.uid == GUEST_UID && user.name == GUEST_ACCOUNT)
        || !groups.iter().any(|group| group.gid == ROOT_GID)
        || !groups.iter().any(|group| group.gid == GUEST_GID)
    {
        return Err("identity database is missing built-in accounts");
    }
    for user in &users {
        if !groups.iter().any(|group| group.gid == user.primary_gid) {
            return Err("identity database references an unknown primary group");
        }
    }
    if users.iter().enumerate().any(|(i, user)| users[..i].iter().any(|previous| previous.uid == user.uid || previous.name == user.name))
        || groups.iter().enumerate().any(|(i, group)| groups[..i].iter().any(|previous| previous.gid == group.gid || previous.name == group.name))
    {
        return Err("identity database contains duplicate IDs or names");
    }
    Ok((users, groups))
}

fn validate_name(name: &str, max_bytes: usize) -> Result<(), &'static str> {
    if name.is_empty() || name.len() > max_bytes || name == "." || name == ".." || name.contains('/') || name.as_bytes().contains(&0) {
        return Err("invalid account name");
    }
    Ok(())
}

fn valid_home(path: &str) -> bool {
    if path.len() > HOME_SIZE - 1 || !path.starts_with("/Users/") || path.ends_with('/') {
        return false;
    }
    let mut parts = path.split('/');
    parts.next() == Some("") && parts.all(|part| !part.is_empty() && part != "." && part != "..")
}

fn hash_bytes(mut hash: u64, bytes: &[u8]) -> u64 {
    for byte in bytes {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}
fn get_u16(input: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([input[offset], input[offset + 1]])
}
fn get_u32(input: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(input[offset..offset + 4].try_into().unwrap_or([0; 4]))
}
fn get_u64(input: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(input[offset..offset + 8].try_into().unwrap_or([0; 8]))
}
fn put_u16(output: &mut [u8], offset: usize, value: u16) {
    output[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}
fn put_u32(output: &mut [u8], offset: usize, value: u32) {
    output[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
fn put_u64(output: &mut [u8], offset: usize, value: u64) {
    output[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}
