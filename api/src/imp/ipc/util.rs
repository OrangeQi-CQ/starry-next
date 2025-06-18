use lazy_static::lazy_static;

pub struct IpcidAllocator {
    next_ipcid: i32,
}

impl IpcidAllocator {
    fn new() -> Self {
        IpcidAllocator { next_ipcid: 0 }
    }

    pub fn alloc(&mut self) -> i32 {
        let ipcid = self.next_ipcid;
        self.next_ipcid += 1;
        ipcid
    }
}

lazy_static! {
    pub static ref IPCID_ALLOCATOR: Mutex<IpcidAllocator> = Mutex::new(IpcidAllocator::new());
}