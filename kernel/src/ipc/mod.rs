pub mod pipe;

use crate::object::{KernelObject, ObjectHeader, ObjectType};
use crate::handle::{HandleTable, TransferredHandle};
use crate::sync::SpinLock;
use alloc::sync::Arc;
use core::any::Any;

const QUEUE_CAPACITY: usize = 8;
const MAX_MESSAGE: usize = 256;
const MAX_TRANSFER_HANDLES: usize = 8;
struct Message {
    data: [u8; MAX_MESSAGE],
    length: usize,
    handles: [Option<TransferredHandle>; MAX_TRANSFER_HANDLES],
    handle_count: usize,
}
impl Message {
    const fn empty() -> Self {
        Self { data: [0; MAX_MESSAGE], length: 0, handles: [const { None }; MAX_TRANSFER_HANDLES], handle_count: 0 }
    }
}
struct MessageQueue {
    items: [Message; QUEUE_CAPACITY],
    head: usize,
    count: usize,
}
impl MessageQueue {
    const fn new() -> Self {
        Self {
            items: [const { Message::empty() }; QUEUE_CAPACITY],
            head: 0,
            count: 0,
        }
    }
    fn send(&mut self, data: &[u8], handles: &[TransferredHandle]) -> Result<(), &'static str> {
        if data.len() > MAX_MESSAGE {
            return Err("message exceeds channel limit");
        }
        if handles.len() > MAX_TRANSFER_HANDLES {
            return Err("too many transferred handles");
        }
        if self.count == QUEUE_CAPACITY {
            return Err("channel queue is full");
        }
        let index = (self.head + self.count) % QUEUE_CAPACITY;
        let mut message = Message::empty();
        message.data[..data.len()].copy_from_slice(data);
        message.length = data.len();
        message.handle_count = handles.len();
        for (slot, handle) in message.handles.iter_mut().zip(handles) {
            *slot = Some(handle.clone());
        }
        self.items[index] = message;
        self.count += 1;
        Ok(())
    }
    fn receive(
        &mut self,
        out: &mut [u8],
        table: Option<&mut HandleTable>,
        output_handles: &mut [u32],
    ) -> Result<(usize, usize), &'static str> {
        if self.count == 0 {
            return Err("channel has no message");
        }
        let index = self.head;
        let (count, handle_count) = {
            let item = &self.items[index];
            if out.len() < item.length {
                return Err("receive buffer is too small");
            }
            if item.handle_count > output_handles.len() {
                return Err("transfer output buffer is too small");
            }
            let handle_count = if item.handle_count == 0 {
                0
            } else {
                table.ok_or("message contains handles")?.import_transfers_with(
                    item.handle_count,
                    output_handles,
                    |position| item.handles[position].clone(),
                )?
            };
            out[..item.length].copy_from_slice(&item.data[..item.length]);
            (item.length, handle_count)
        };
        self.items[index] = Message::empty();
        self.head = (self.head + 1) % QUEUE_CAPACITY;
        self.count -= 1;
        Ok((count, handle_count))
    }
}

struct ChannelCore {
    queues: [SpinLock<MessageQueue>; 2],
    waiters: [crate::task::wait::WaitQueue; 2],
}
pub struct ChannelEndpoint {
    header: ObjectHeader,
    core: Arc<ChannelCore>,
    side: usize,
}
impl ChannelEndpoint {
    pub fn pair() -> (Arc<Self>, Arc<Self>) {
        let core = Arc::new(ChannelCore {
            queues: [
                SpinLock::new(MessageQueue::new()),
                SpinLock::new(MessageQueue::new()),
            ],
            waiters: [
                crate::task::wait::WaitQueue::new(),
                crate::task::wait::WaitQueue::new(),
            ],
        });
        (
            Arc::new(Self {
                header: ObjectHeader::new(ObjectType::Channel),
                core: core.clone(),
                side: 0,
            }),
            Arc::new(Self {
                header: ObjectHeader::new(ObjectType::Channel),
                core,
                side: 1,
            }),
        )
    }
    pub fn send(&self, data: &[u8]) -> Result<(), &'static str> {
        self.send_with_transfers(data, &[])
    }
    pub fn send_with_transfers(&self, data: &[u8], handles: &[TransferredHandle]) -> Result<(), &'static str> {
        self.core.queues[self.side].lock().send(data, handles)?;
        self.core.waiters[self.side].wake_all();
        Ok(())
    }
    pub fn receive(&self, out: &mut [u8]) -> Result<usize, &'static str> {
        let queue = 1 - self.side;
        let (length, transferred) = self.core.queues[queue].lock().receive(out, None, &mut [])?;
        if transferred != 0 { return Err("message contains handles"); }
        self.core.waiters[queue].wake_all();
        Ok(length)
    }
    pub fn receive_with_transfers(
        &self,
        out: &mut [u8],
        table: &mut HandleTable,
        handles: &mut [u32],
    ) -> Result<(usize, usize), &'static str> {
        let queue = 1 - self.side;
        let result = self.core.queues[queue].lock().receive(out, Some(table), handles)?;
        self.core.waiters[queue].wake_all();
        Ok(result)
    }

    pub fn readable_wait_queue(&self) -> &crate::task::wait::WaitQueue {
        &self.core.waiters[1 - self.side]
    }

    pub fn writable_wait_queue(&self) -> &crate::task::wait::WaitQueue {
        &self.core.waiters[self.side]
    }
}
impl KernelObject for ChannelEndpoint {
    fn header(&self) -> &ObjectHeader {
        &self.header
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

// Messages remain bounded. Handle transfer duplicates an Arc-backed object
// reference with unchanged rights; sender handles remain valid after send.
