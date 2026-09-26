use crate::object::{KernelObject, ObjectType};
use crate::sync::SpinLock;
use alloc::sync::Arc;
use core::any::Any;
use core::marker::PhantomData;

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
const EMPTY: Entry = Entry {
    generation: 1,
    object: None,
    object_type: ObjectType::Event,
    rights: AccessRights::NONE,
};

#[derive(Clone)]
pub struct HandleTable {
    entries: [Entry; 128],
}
impl HandleTable {
    pub const fn new() -> Self {
        Self {
            entries: [const { EMPTY }; 128],
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
        let raw = (slot.generation << 8) | index as u32;
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
        let index = (handle.raw & 0xff) as usize;
        let generation = handle.raw >> 8;
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
        let index = (raw & 0xff) as usize;
        let generation = raw >> 8;
        let slot = self.entries.get_mut(index).ok_or("invalid handle")?;
        if slot.generation != generation || slot.object.is_none() {
            return Err("stale or closed handle");
        }
        slot.object = None;
        slot.generation = slot.generation.wrapping_add(1).max(1);
        slot.rights = AccessRights::NONE;
        Ok(())
    }

    pub fn duplicate<T: KernelObject>(
        &mut self,
        handle: Handle<T>,
        rights: AccessRights,
    ) -> Result<Handle<T>, &'static str> {
        let source_index = (handle.raw & 0xff) as usize;
        let source_generation = handle.raw >> 8;
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
            raw: (slot.generation << 8) | index as u32,
            _type: PhantomData,
        })
    }

    pub fn object_type(&self, raw: u32) -> Option<ObjectType> {
        let index = (raw & 0xff) as usize;
        let generation = raw >> 8;
        let slot = self.entries.get(index)?;
        if slot.generation == generation && slot.object.is_some() {
            Some(slot.object_type)
        } else {
            None
        }
    }

    pub fn clear(&mut self) {
        for entry in &mut self.entries {
            if entry.object.take().is_some() {
                entry.generation = entry.generation.wrapping_add(1).max(1);
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
