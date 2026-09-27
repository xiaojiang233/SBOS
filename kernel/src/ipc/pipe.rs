use crate::object::{KernelObject, ObjectHeader, ObjectType};
use crate::sync::SpinLock;
use alloc::sync::Arc;
use core::any::Any;
use core::sync::atomic::{AtomicUsize, Ordering};

const CAPACITY: usize = 4096;

struct PipeState {
    bytes: [u8; CAPACITY],
    head: usize,
    length: usize,
}

struct PipeCore {
    state: SpinLock<PipeState>,
    readers: AtomicUsize,
    writers: AtomicUsize,
    readable: crate::task::wait::WaitQueue,
    writable: crate::task::wait::WaitQueue,
}

pub struct PipeReader {
    header: ObjectHeader,
    core: Arc<PipeCore>,
}

pub struct PipeWriter {
    header: ObjectHeader,
    core: Arc<PipeCore>,
}

impl PipeReader {
    pub fn pair() -> (Arc<Self>, Arc<PipeWriter>) {
        let core = Arc::new(PipeCore {
            state: SpinLock::new(PipeState {
                bytes: [0; CAPACITY],
                head: 0,
                length: 0,
            }),
            readers: AtomicUsize::new(1),
            writers: AtomicUsize::new(1),
            readable: crate::task::wait::WaitQueue::new(),
            writable: crate::task::wait::WaitQueue::new(),
        });
        (
            Arc::new(Self {
                header: ObjectHeader::new(ObjectType::PipeReader),
                core: core.clone(),
            }),
            Arc::new(PipeWriter {
                header: ObjectHeader::new(ObjectType::PipeWriter),
                core,
            }),
        )
    }

    /// `None` means no data is ready yet; `Some(0)` means every writer closed.
    pub fn try_read(&self, output: &mut [u8]) -> Option<usize> {
        let mut state = self.core.state.lock();
        if state.length == 0 {
            return (self.core.writers.load(Ordering::Acquire) == 0).then_some(0);
        }
        let count = output.len().min(state.length);
        for (index, byte) in output[..count].iter_mut().enumerate() {
            *byte = state.bytes[(state.head + index) % CAPACITY];
        }
        state.head = (state.head + count) % CAPACITY;
        state.length -= count;
        drop(state);
        self.core.writable.wake_all();
        Some(count)
    }

    pub fn wait_queue(&self) -> &crate::task::wait::WaitQueue { &self.core.readable }

    pub fn is_readable(&self) -> bool {
        let state = self.core.state.lock();
        state.length != 0 || self.core.writers.load(Ordering::Acquire) == 0
    }

}

impl Drop for PipeReader {
    fn drop(&mut self) {
        self.core.readers.fetch_sub(1, Ordering::AcqRel);
        self.core.writable.wake_all();
    }
}

impl KernelObject for PipeReader {
    fn header(&self) -> &ObjectHeader { &self.header }
    fn as_any(&self) -> &dyn Any { self }
}

impl PipeWriter {
    /// `None` means the fixed pipe buffer is full; `Err(())` means no readers remain.
    pub fn try_write(&self, input: &[u8]) -> Result<Option<usize>, ()> {
        if input.is_empty() { return Ok(Some(0)); }
        if self.core.readers.load(Ordering::Acquire) == 0 { return Err(()); }
        let mut state = self.core.state.lock();
        let available = CAPACITY - state.length;
        if available == 0 { return Ok(None); }
        let count = input.len().min(available);
        let tail = (state.head + state.length) % CAPACITY;
        for (index, byte) in input[..count].iter().enumerate() {
            state.bytes[(tail + index) % CAPACITY] = *byte;
        }
        state.length += count;
        drop(state);
        self.core.readable.wake_all();
        Ok(Some(count))
    }

    pub fn wait_queue(&self) -> &crate::task::wait::WaitQueue { &self.core.writable }

    pub fn is_writable(&self) -> bool {
        self.core.readers.load(Ordering::Acquire) == 0
            || self.core.state.lock().length < CAPACITY
    }

}

impl Drop for PipeWriter {
    fn drop(&mut self) {
        self.core.writers.fetch_sub(1, Ordering::AcqRel);
        self.core.readable.wake_all();
    }
}

impl KernelObject for PipeWriter {
    fn header(&self) -> &ObjectHeader { &self.header }
    fn as_any(&self) -> &dyn Any { self }
}
