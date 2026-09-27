#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum FdKind {
    Closed = 0,
    ConsoleRead = 1,
    ConsoleWrite = 2,
    File = 3,
    PipeRead = 4,
    PipeWrite = 5,
    SocketUdp = 6,
}

#[derive(Clone, Copy)]
pub struct FdEntry {
    pub handle: u32,
    pub kind: FdKind,
    pub status_flags: u32,
    pub descriptor_flags: u32,
}

pub const CLOSED_FD: FdEntry = FdEntry {
    handle: 0,
    kind: FdKind::Closed,
    status_flags: 0,
    descriptor_flags: 0,
};

#[derive(Clone)]
pub struct FdTable {
    entries: [FdEntry; 128],
}

impl FdTable {
    pub const fn new() -> Self {
        let mut entries = [CLOSED_FD; 128];
        entries[0] = FdEntry {
            handle: 0,
            kind: FdKind::ConsoleRead,
            status_flags: 0,
            descriptor_flags: 0,
        };
        entries[1] = FdEntry {
            handle: 0,
            kind: FdKind::ConsoleWrite,
            status_flags: 1,
            descriptor_flags: 0,
        };
        entries[2] = entries[1];
        Self { entries }
    }

    pub fn get(&self, fd: usize) -> Option<FdEntry> {
        self.entries
            .get(fd)
            .copied()
            .filter(|entry| entry.kind != FdKind::Closed)
    }

    pub fn allocate(&mut self, entry: FdEntry, minimum: usize) -> Result<usize, &'static str> {
        let index = (minimum..self.entries.len())
            .find(|index| self.entries[*index].kind == FdKind::Closed)
            .ok_or("process descriptor table is full")?;
        self.entries[index] = entry;
        Ok(index)
    }

    pub fn replace(&mut self, fd: usize, entry: FdEntry) -> Result<FdEntry, &'static str> {
        let slot = self.entries.get_mut(fd).ok_or("invalid file descriptor")?;
        let previous = *slot;
        *slot = entry;
        Ok(previous)
    }

    pub fn close(&mut self, fd: usize) -> Result<FdEntry, &'static str> {
        let slot = self.entries.get_mut(fd).ok_or("invalid file descriptor")?;
        if slot.kind == FdKind::Closed {
            return Err("file descriptor is closed");
        }
        let previous = *slot;
        *slot = CLOSED_FD;
        Ok(previous)
    }

    pub fn close_on_exec(&mut self, closed: &mut [FdEntry; 128]) -> usize {
        let mut count = 0;
        for entry in &mut self.entries {
            if entry.kind != FdKind::Closed && entry.descriptor_flags & 1 != 0 {
                closed[count] = *entry;
                count += 1;
                *entry = CLOSED_FD;
            }
        }
        count
    }

    pub fn collect_socket_ids(&self, ids: &mut [u32; 128]) -> usize {
        let mut count = 0;
        for entry in &self.entries {
            if entry.kind == FdKind::SocketUdp {
                ids[count] = entry.handle;
                count += 1;
            }
        }
        count
    }

    pub fn clear_and_collect_sockets(&mut self, ids: &mut [u32; 128]) -> usize {
        let count = self.collect_socket_ids(ids);
        self.entries.fill(CLOSED_FD);
        count
    }
}
