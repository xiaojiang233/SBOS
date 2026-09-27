use crate::object::{KernelObject, ObjectType};
use crate::sync::SpinLock;
use alloc::sync::Arc;
use core::any::Any;
use core::marker::PhantomData;

pub const HANDLE_INDEX_BITS: u32 = 8;
pub const HANDLE_INDEX_MASK: u32 = (1 << HANDLE_INDEX_BITS) - 1;
pub const HANDLE_GENERATION_MASK: u32 = (1 << (32 - HANDLE_INDEX_BITS)) - 1;
pub const HANDLE_SLOT_COUNT: usize = 128;
const INITIAL_GENERATION: u32 = 1;

const fn encode_handle(index: usize, generation: u32) -> u32 {
    ((generation & HANDLE_GENERATION_MASK) << HANDLE_INDEX_BITS) | index as u32
}

const fn decode_handle(raw: u32) -> (usize, u32) {
    (
        (raw & HANDLE_INDEX_MASK) as usize,
        (raw >> HANDLE_INDEX_BITS) & HANDLE_GENERATION_MASK,
    )
}

fn next_generation(generation: u32) -> u32 {
    let next = generation.wrapping_add(1) & HANDLE_GENERATION_MASK;
    if next == 0 { INITIAL_GENERATION } else { next }
}

const _: () = assert!(HANDLE_SLOT_COUNT <= (HANDLE_INDEX_MASK as usize + 1));

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccessRights(pub u16);
impl AccessRights {
    pub const NONE: Self = Self(0);
    pub const READ: Self = Self(1 << 0);
    pub const WRITE: Self = Self(1 << 1);
    pub const EXECUTE: Self = Self(1 << 2);
    pub const MAP: Self = Self(1 << 3);
    pub const WAIT: Self = Self(1 << 4);
    pub const SIGNAL: Self = Self(1 << 5);
    pub const DUPLICATE: Self = Self(1 << 6);
    pub const TRANSFER: Self = Self(1 << 7);
    pub const CONTROL: Self = Self(1 << 8);
    pub const FILE_READ: Self = Self(Self::READ.0 | Self::MAP.0 | Self::DUPLICATE.0);
    pub const FILE_READ_WRITE: Self =
        Self(Self::READ.0 | Self::WRITE.0 | Self::MAP.0 | Self::DUPLICATE.0);
    pub const fn contains(self, requested: Self) -> bool {
        self.0 & requested.0 == requested.0
    }
}

#[repr(transparent)]
#[derive(Eq, PartialEq, Debug)]
pub struct Handle<T> {
    raw: u32,
    _type: PhantomData<fn() -> T>,
}
impl<T> Copy for Handle<T> {}
impl<T> Clone for Handle<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Handle<T> {
    pub const fn raw(self) -> u32 {
        self.raw
    }
    pub const unsafe fn from_raw(raw: u32) -> Self {
        Self {
            raw,
            _type: PhantomData,
        }
    }
}

#[derive(Clone)]
struct Entry {
    generation: u32,
    object: Option<Arc<dyn Any + Send + Sync>>,
    object_type: ObjectType,
    rights: AccessRights,
}

/// A duplicated object reference and its original rights, ready to be placed
/// in a bounded IPC message. The sender retains its original handle.
#[derive(Clone)]
pub struct TransferredHandle {
    object: Arc<dyn Any + Send + Sync>,
    object_type: ObjectType,
    rights: AccessRights,
}
const EMPTY: Entry = Entry {
    generation: INITIAL_GENERATION,
    object: None,
    object_type: ObjectType::Event,
    rights: AccessRights::NONE,
};

#[derive(Clone)]
pub struct HandleTable {
    entries: [Entry; HANDLE_SLOT_COUNT],
}
impl HandleTable {
    pub const fn new() -> Self {
        Self {
            entries: [const { EMPTY }; HANDLE_SLOT_COUNT],
        }
    }

    pub fn insert<T: KernelObject>(
        &mut self,
        object: Arc<T>,
        rights: AccessRights,
    ) -> Result<Handle<T>, &'static str> {
        let Some((index, slot)) = self
            .entries
            .iter_mut()
            .enumerate()
            .find(|(_, slot)| slot.object.is_none())
        else {
            return Err("handle table full");
        };
        let kind = object.header().object_type();
        slot.object_type = kind;
        slot.rights = rights;
        let value: Arc<dyn Any + Send + Sync> = object;
        slot.object = Some(value);
        let raw = encode_handle(index, slot.generation);
        Ok(Handle {
            raw,
            _type: PhantomData,
        })
    }

    pub fn get<T: KernelObject>(
        &self,
        handle: Handle<T>,
        required: AccessRights,
    ) -> Result<Arc<T>, &'static str> {
        let (index, generation) = decode_handle(handle.raw);
        let slot = self.entries.get(index).ok_or("invalid handle")?;
        if slot.generation != generation {
            return Err("stale handle");
        }
        if !slot.rights.contains(required) {
            return Err("handle access denied");
        }
        let object = slot.object.as_ref().ok_or("closed handle")?.clone();
        Arc::downcast::<T>(object).map_err(|_| "handle type mismatch")
    }

    pub fn close(&mut self, raw: u32) -> Result<(), &'static str> {
        let (index, generation) = decode_handle(raw);
        let slot = self.entries.get_mut(index).ok_or("invalid handle")?;
        if slot.generation != generation || slot.object.is_none() {
            return Err("stale or closed handle");
        }
        slot.object = None;
        slot.generation = next_generation(slot.generation);
        slot.rights = AccessRights::NONE;
        Ok(())
    }

    pub fn duplicate<T: KernelObject>(
        &mut self,
        handle: Handle<T>,
        rights: AccessRights,
    ) -> Result<Handle<T>, &'static str> {
        let (source_index, source_generation) = decode_handle(handle.raw);
        let source = self.entries.get(source_index).ok_or("invalid handle")?;
        if source.generation != source_generation
            || !source.rights.contains(AccessRights::DUPLICATE)
        {
            return Err("handle duplication denied");
        }
        if !source.rights.contains(rights) {
            return Err("cannot amplify handle rights");
        }
        let object_type = source.object_type;
        let object = source.object.as_ref().ok_or("closed handle")?.clone();
        let Some((index, slot)) = self
            .entries
            .iter_mut()
            .enumerate()
            .find(|(_, slot)| slot.object.is_none())
        else {
            return Err("handle table full");
        };
        slot.object_type = object_type;
        slot.rights = rights;
        slot.object = Some(object);
        Ok(Handle {
            raw: encode_handle(index, slot.generation),
            _type: PhantomData,
        })
    }

    pub fn object_type(&self, raw: u32) -> Option<ObjectType> {
        let (index, generation) = decode_handle(raw);
        let slot = self.entries.get(index)?;
        if slot.generation == generation && slot.object.is_some() {
            Some(slot.object_type)
        } else {
            None
        }
    }

    pub fn export_transfer(&self, raw: u32) -> Result<TransferredHandle, &'static str> {
        let (index, generation) = decode_handle(raw);
        let slot = self.entries.get(index).ok_or("invalid handle")?;
        if slot.generation != generation || slot.object.is_none() {
            return Err("stale or closed handle");
        }
        if !slot.rights.contains(AccessRights::TRANSFER) {
            return Err("handle transfer denied");
        }
        Ok(TransferredHandle {
            object: slot.object.as_ref().ok_or("closed handle")?.clone(),
            object_type: slot.object_type,
            rights: slot.rights,
        })
    }

    /// Insert all received duplicates atomically. If the receiver lacks slots,
    /// no table entry is changed and the queued message remains available.
    pub fn import_transfers(
        &mut self,
        transfers: &[TransferredHandle],
        output: &mut [u32],
    ) -> Result<usize, &'static str> {
        self.import_transfers_with(transfers.len(), output, |index| transfers.get(index).cloned())
    }

    pub fn import_transfers_with(
        &mut self,
        count: usize,
        output: &mut [u32],
        mut get: impl FnMut(usize) -> Option<TransferredHandle>,
    ) -> Result<usize, &'static str> {
        if output.len() < count {
            return Err("transfer output buffer is too small");
        }
        if count > HANDLE_SLOT_COUNT {
            return Err("receiver handle table full");
        }
        let mut indices = [0usize; HANDLE_SLOT_COUNT];
        let mut found = 0usize;
        for (index, entry) in self.entries.iter().enumerate() {
            if entry.object.is_none() {
                indices[found] = index;
                found += 1;
                if found == count { break; }
            }
        }
        if found != count {
            return Err("receiver handle table full");
        }
        let mut prepared: [Option<TransferredHandle>; HANDLE_SLOT_COUNT] =
            [const { None }; HANDLE_SLOT_COUNT];
        for (index, slot) in prepared.iter_mut().take(count).enumerate() {
            *slot = Some(get(index).ok_or("invalid transferred handle")?);
        }
        for position in 0..count {
            let transfer = prepared[position].take().ok_or("invalid transferred handle")?;
            let slot = &mut self.entries[indices[position]];
            slot.object_type = transfer.object_type;
            slot.rights = transfer.rights;
            slot.object = Some(transfer.object);
            output[position] = encode_handle(indices[position], slot.generation);
        }
        Ok(count)
    }

    pub fn clear(&mut self) {
        for entry in &mut self.entries {
            if entry.object.take().is_some() {
                entry.generation = next_generation(entry.generation);
            }
            entry.rights = AccessRights::NONE;
        }
    }

}

pub struct SharedHandleTable(pub SpinLock<HandleTable>);
impl SharedHandleTable {
    pub const fn new() -> Self {
        Self(SpinLock::new(HandleTable::new()))
    }
}
