use crate::sync::SpinLock;
const COMPLETION_CAPACITY: usize = 128;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IoOperation {
    Read,
    Write,
}
#[derive(Clone, Copy, Debug)]
pub struct IoCompletion {
    pub key: u64,
    pub result: i64,
    pub bytes: u64,
    pub sequence: u64,
}
pub struct CompletionQueue {
    next_sequence: u64,
    items: [Option<IoCompletion>; COMPLETION_CAPACITY],
}
impl CompletionQueue {
    pub const fn new() -> Self {
        Self {
            next_sequence: 1,
            items: [const { None }; COMPLETION_CAPACITY],
        }
    }
    pub fn post(&mut self, key: u64, result: i64, bytes: u64) -> IoCompletion {
        let completion = IoCompletion {
            key,
            result,
            bytes,
            sequence: self.next_sequence,
        };
        self.next_sequence = self.next_sequence.wrapping_add(1);
        let index = (completion.sequence as usize - 1) % COMPLETION_CAPACITY;
        self.items[index] = Some(completion);
        completion
    }
    pub fn take(&mut self, sequence: u64) -> Option<IoCompletion> {
        let index = (sequence as usize - 1) % COMPLETION_CAPACITY;
        let item = self.items[index]?;
        if item.sequence == sequence {
            self.items[index] = None;
            Some(item)
        } else {
            None
        }
    }
}
pub struct ProcessCompletions(pub SpinLock<CompletionQueue>);
impl ProcessCompletions {
    pub const fn new() -> Self {
        Self(SpinLock::new(CompletionQueue::new()))
    }
}

// The first native file backend completes RAM-backed tmpfs requests at submit
// time. Storage drivers can provide true deferred completions through this queue.
