use crate::object::{EventObject, KernelObject, ObjectHeader, ObjectType};
use crate::sync::SpinLock;
use alloc::string::String;
use alloc::sync::{Arc, Weak};
use alloc::vec::Vec;

struct Entry {
    key: String,
    value: String,
}
struct Watch {
    prefix: String,
    event: Weak<EventObject>,
}
struct State {
    entries: Vec<Entry>,
    watchers: Vec<Watch>,
    generation: u64,
}
static STORE: SpinLock<State> = SpinLock::new(State {
    entries: Vec::new(),
    watchers: Vec::new(),
    generation: 0,
});

pub fn init() {
    let mut s = STORE.lock();
    s.entries.clear();
    s.watchers.clear();
    s.generation = 0;
    s.entries.push(Entry {
        key: "system/display/scaling".into(),
        value: "1".into(),
    });
    s.entries.push(Entry {
        key: "system/network/hostname".into(),
        value: "sbos".into(),
    });
    s.entries.push(Entry {
        key: "user/appearance/theme".into(),
        value: "dark".into(),
    });
}
pub fn get(key: &str) -> Option<String> {
    STORE
        .lock()
        .entries
        .iter()
        .find(|e| e.key == key)
        .map(|e| e.value.clone())
}
pub fn generation() -> u64 {
    STORE.lock().generation
}
pub fn watch(prefix: &str, event: &Arc<EventObject>) {
    STORE.lock().watchers.push(Watch {
        prefix: prefix.into(),
        event: Arc::downgrade(event),
    });
}

pub struct ConfigTransaction {
    changes: Vec<(String, Option<String>)>,
}
impl ConfigTransaction {
    pub fn new() -> Self {
        Self {
            changes: Vec::new(),
        }
    }
    pub fn set(&mut self, key: &str, value: &str) -> Result<(), &'static str> {
        validate_key(key)?;
        if value.len() > 1024 {
            return Err("configuration value is too large");
        }
        self.changes.push((key.into(), Some(value.into())));
        Ok(())
    }
    pub fn delete(&mut self, key: &str) -> Result<(), &'static str> {
        validate_key(key)?;
        self.changes.push((key.into(), None));
        Ok(())
    }
    pub fn commit(self) -> Result<u64, &'static str> {
        let mut state = STORE.lock();
        for (key, value) in &self.changes {
            if let Some(value) = value {
                if let Some(entry) = state.entries.iter_mut().find(|e| e.key == *key) {
                    entry.value = value.clone();
                } else {
                    state.entries.push(Entry {
                        key: key.clone(),
                        value: value.clone(),
                    });
                }
            } else {
                state.entries.retain(|e| e.key != *key);
            }
        }
        state.generation = state.generation.wrapping_add(1);
        let mut index = 0;
        while index < state.watchers.len() {
            let notify = self
                .changes
                .iter()
                .any(|(key, _)| key.starts_with(&state.watchers[index].prefix));
            if let Some(event) = state.watchers[index].event.upgrade() {
                if notify {
                    event.signal();
                }
                index += 1;
            } else {
                state.watchers.remove(index);
            }
        }
        Ok(state.generation)
    }
}
fn validate_key(key: &str) -> Result<(), &'static str> {
    if key.is_empty()
        || key.len() > 256
        || key.starts_with('/')
        || key.contains("..")
        || key.as_bytes().contains(&0)
    {
        Err("invalid configuration key")
    } else {
        Ok(())
    }
}
pub fn set(key: &str, value: &str) -> Result<u64, &'static str> {
    let mut transaction = ConfigTransaction::new();
    transaction.set(key, value)?;
    transaction.commit()
}
pub fn delete(key: &str) -> Result<u64, &'static str> {
    let mut transaction = ConfigTransaction::new();
    transaction.delete(key)?;
    transaction.commit()
}

pub struct ConfigTransactionObject {
    header: ObjectHeader,
    transaction: SpinLock<Option<ConfigTransaction>>,
}
impl ConfigTransactionObject {
    pub fn new() -> Self {
        Self {
            header: ObjectHeader::new(ObjectType::ConfigTransaction),
            transaction: SpinLock::new(Some(ConfigTransaction::new())),
        }
    }
    pub fn set(&self, key: &str, value: &str) -> Result<(), &'static str> {
        self.transaction
            .lock()
            .as_mut()
            .ok_or("transaction already finished")?
            .set(key, value)
    }
    pub fn delete(&self, key: &str) -> Result<(), &'static str> {
        self.transaction
            .lock()
            .as_mut()
            .ok_or("transaction already finished")?
            .delete(key)
    }
    pub fn commit(&self) -> Result<u64, &'static str> {
        self.transaction
            .lock()
            .take()
            .ok_or("transaction already finished")?
            .commit()
    }
}
impl KernelObject for ConfigTransactionObject {
    fn header(&self) -> &ObjectHeader {
        &self.header
    }
    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
}

// Values are currently RAM-backed. Crash-safe persistence and journal recovery
// are deliberately deferred until a persistent volume implementation exists.
