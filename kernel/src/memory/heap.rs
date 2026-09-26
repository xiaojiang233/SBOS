use crate::sync::SpinLock;
use core::alloc::{GlobalAlloc, Layout};
use core::ptr::{addr_of_mut, null_mut};

const HEAP_SIZE: usize = 2 * 1024 * 1024;
const ALLOCATION_MAGIC: usize = 0x5342_4f53_4845_4150;

#[repr(align(4096))]
struct Heap([u8; HEAP_SIZE]);
static mut HEAP: Heap = Heap([0; HEAP_SIZE]);

#[repr(C)]
struct FreeBlock {
    size: usize,
    next: *mut FreeBlock,
}

#[repr(C)]
struct AllocationHeader {
    block_start: *mut u8,
    block_size: usize,
    magic: usize,
}

struct HeapState {
    head: *mut FreeBlock,
}
unsafe impl Send for HeapState {}

impl HeapState {
    const fn empty() -> Self {
        Self { head: null_mut() }
    }

    unsafe fn insert_free(&mut self, start: usize, size: usize) {
        let mut previous = null_mut::<FreeBlock>();
        let mut next = self.head;
        while !next.is_null() && (next as usize) < start {
            previous = next;
            next = (*next).next;
        }

        let node = start as *mut FreeBlock;
        (*node).size = size;
        (*node).next = next;
        if previous.is_null() {
            self.head = node;
        } else {
            (*previous).next = node;
        }

        if !next.is_null() && start + (*node).size == next as usize {
            (*node).size += (*next).size;
            (*node).next = (*next).next;
        }
        if !previous.is_null() && (previous as usize) + (*previous).size == node as usize {
            (*previous).size += (*node).size;
            (*previous).next = (*node).next;
        }
    }
}

static HEAP_STATE: SpinLock<HeapState> = SpinLock::new(HeapState::empty());

pub struct KernelHeap;

unsafe impl GlobalAlloc for KernelHeap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut heap = HEAP_STATE.lock();
        let mut previous = null_mut::<FreeBlock>();
        let mut current = heap.head;
        let header_size = core::mem::size_of::<AllocationHeader>();
        let minimum_free = core::mem::size_of::<FreeBlock>();
        let request_size = layout.size().max(1);

        while !current.is_null() {
            let block_start = current as usize;
            let block_end = match block_start.checked_add((*current).size) {
                Some(end) => end,
                None => return null_mut(),
            };
            let data_start = match block_start
                .checked_add(minimum_free)
                .and_then(|value| value.checked_add(header_size))
                .and_then(|value| align_up(value, layout.align()))
            {
                Some(value) => value,
                None => return null_mut(),
            };
            let header_start = data_start - header_size;
            let prefix_size = header_start - block_start;
            let allocation_start = if prefix_size >= minimum_free {
                header_start
            } else {
                block_start
            };
            let payload_start = if allocation_start == header_start {
                data_start
            } else {
                match block_start
                    .checked_add(minimum_free)
                    .and_then(|value| value.checked_add(header_size))
                    .and_then(|value| align_up(value, layout.align()))
                {
                    Some(value) => value,
                    None => return null_mut(),
                }
            };
            let header_address = payload_start - header_size;
            let payload_end = match payload_start.checked_add(request_size) {
                Some(end) => end,
                None => return null_mut(),
            };
            let mut allocation_end = match align_up(payload_end, core::mem::align_of::<FreeBlock>())
            {
                Some(end) => end,
                None => return null_mut(),
            };
            if allocation_end > block_end {
                previous = current;
                current = (*current).next;
                continue;
            }

            let mut suffix_size = block_end - allocation_end;
            if suffix_size < minimum_free {
                allocation_end = block_end;
                suffix_size = 0;
            }

            let after = (*current).next;
            let prefix = header_address - block_start;
            let suffix = if suffix_size != 0 {
                let suffix_node = allocation_end as *mut FreeBlock;
                (*suffix_node).size = suffix_size;
                (*suffix_node).next = after;
                suffix_node
            } else {
                after
            };
            if prefix >= minimum_free {
                (*current).size = prefix;
                (*current).next = suffix;
            } else if previous.is_null() {
                heap.head = suffix;
            } else {
                (*previous).next = suffix;
            }

            let header = header_address as *mut AllocationHeader;
            header.write(AllocationHeader {
                block_start: allocation_start as *mut u8,
                block_size: allocation_end - allocation_start,
                magic: ALLOCATION_MAGIC,
            });
            return payload_start as *mut u8;
        }
        null_mut()
    }

    unsafe fn dealloc(&self, pointer: *mut u8, _: Layout) {
        if pointer.is_null() {
            return;
        }
        let header = pointer.sub(core::mem::size_of::<AllocationHeader>()) as *mut AllocationHeader;
        if (*header).magic != ALLOCATION_MAGIC {
            return;
        }
        let start = (*header).block_start as usize;
        let size = (*header).block_size;
        (*header).magic = 0;
        HEAP_STATE.lock().insert_free(start, size);
    }
}

fn align_up(value: usize, alignment: usize) -> Option<usize> {
    value
        .checked_add(alignment - 1)
        .map(|aligned| aligned & !(alignment - 1))
}

pub fn init() {
    let start = unsafe { addr_of_mut!(HEAP.0) as *mut u8 as usize };
    let mut heap = HEAP_STATE.lock();
    heap.head = start as *mut FreeBlock;
    unsafe {
        (*heap.head).size = HEAP_SIZE;
        (*heap.head).next = null_mut();
    }
}
