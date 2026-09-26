pub mod pipe;

use crate::object::{KernelObject, ObjectHeader, ObjectType};
use crate::sync::SpinLock;
use alloc::sync::Arc;
use core::any::Any;

const QUEUE_CAPACITY: usize = 8;
const MAX_MESSAGE: usize = 256;
struct Message {
    data: [u8; MAX_MESSAGE],
    length: usize,
}
const EMPTY_MESSAGE: Message = Message {
    data: [0; MAX_MESSAGE],
    length: 0,
};
struct MessageQueue {
    items: [Message; QUEUE_CAPACITY],
    head: usize,
    count: usize,
}
impl MessageQueue {
    const fn new() -> Self {
        Self {
            items: [EMPTY_MESSAGE; QUEUE_CAPACITY],
            head: 0,
            count: 0,
        }
    }
    fn send(&mut self, data: &[u8]) -> Result<(), &'static str> {
        if data.len() > MAX_MESSAGE {
            return Err("message exceeds channel limit");
        }
        if self.count == QUEUE_CAPACITY {
            return Err("channel queue is full");
        }
        let index = (self.head + self.count) % QUEUE_CAPACITY;
        self.items[index].data[..data.len()].copy_from_slice(data);
        self.items[index].length = data.len();
        self.count += 1;
        Ok(())
    }
    fn receive(&mut self, out: &mut [u8]) -> Result<usize, &'static str> {
        if self.count == 0 {
            return Err("channel has no message");
        }
        let item = &self.items[self.head];
        if out.len() < item.length {
            return Err("receive buffer is too small");
        }
        out[..item.length].copy_from_slice(&item.data[..item.length]);
        let count = item.length;
        self.items[self.head].length = 0;
        self.head = (self.head + 1) % QUEUE_CAPACITY;
        self.count -= 1;
        Ok(count)
    }
}

struct ChannelCore {
    queues: [SpinLock<MessageQueue>; 2],
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
        self.core.queues[self.side].lock().send(data)
    }
    pub fn receive(&self, out: &mut [u8]) -> Result<usize, &'static str> {
        self.core.queues[1 - self.side].lock().receive(out)
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

// Channel messages are bounded and nonblocking in the first release. Handle
// transfer and event-driven wakeups are extension points, not silently dropped.
