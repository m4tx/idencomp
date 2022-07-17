use std::mem;
use std::sync::{Condvar, Mutex, MutexGuard};
use std::time::Instant;

use number_prefix::NumberPrefix;

use crate::progress::ByteNum;

#[derive(Debug)]
pub(super) struct IdnBlockLock {
    current_block: Mutex<u32>,
    current_block_cvar: Condvar,
}

impl IdnBlockLock {
    #[must_use]
    pub fn new() -> Self {
        Self {
            current_block: Mutex::new(0),
            current_block_cvar: Condvar::new(),
        }
    }

    pub fn lock(&self, block_index: u32) -> IdnBlockLockGuard<'_> {
        IdnBlockLockGuard::new(&self.current_block, &self.current_block_cvar, block_index)
    }
}

#[derive(Debug)]
#[must_use]
pub(super) struct IdnBlockLockGuard<'a> {
    current_block: MutexGuard<'a, u32>,
    current_block_cvar: &'a Condvar,
}

impl<'a> IdnBlockLockGuard<'a> {
    fn new(current_block: &'a Mutex<u32>, cvar: &'a Condvar, block_index: u32) -> Self {
        let mut current_block = current_block.lock().expect("Could not acquire block lock");
        while *current_block != block_index {
            current_block = cvar
                .wait(current_block)
                .expect("Could not acquire block lock");
        }

        Self {
            current_block,
            current_block_cvar: cvar,
        }
    }
}

impl<'a> Drop for IdnBlockLockGuard<'a> {
    fn drop(&mut self) {
        *self.current_block += 1;
        self.current_block_cvar.notify_all();
    }
}

#[derive(Debug)]
struct DataQueueState<T> {
    data: Vec<T>,
    finished: bool,
}

impl<T> DataQueueState<T> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
            finished: false,
        }
    }
}

#[derive(Debug)]
pub(super) struct DataQueue<T> {
    state: Mutex<DataQueueState<T>>,
    cvar: Condvar,
}

impl<T> DataQueue<T> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: Mutex::new(DataQueueState::new()),
            cvar: Condvar::new(),
        }
    }

    pub fn add(&self, data: T) {
        let mut state = self
            .state
            .lock()
            .expect("Could not acquire data queue lock");

        state.data.push(data);
        self.cvar.notify_all();
    }

    pub fn set_finished(&self) {
        let mut state = self
            .state
            .lock()
            .expect("Could not acquire data queue lock");

        state.finished = true;
        self.cvar.notify_all();
    }

    pub fn add_all(&self, mut data: Vec<T>) {
        let mut state = self
            .state
            .lock()
            .expect("Could not acquire data queue lock");

        if data.is_empty() {
            state.finished = true;
        } else {
            state.data.append(&mut data);
        }
        self.cvar.notify_all();
    }

    pub fn retrieve_all(&self) -> Vec<T> {
        let mut state = self
            .state
            .lock()
            .expect("Could not acquire data queue lock");
        while !state.finished && state.data.is_empty() {
            state = self
                .cvar
                .wait(state)
                .expect("Could not acquire data queue lock");
        }

        mem::take(&mut state.data)
    }
}

#[must_use]
pub(crate) fn format_stats(start_time: Instant, bytes_compressed: ByteNum) -> String {
    let elapsed = start_time.elapsed();

    let size_human = format_bytes(bytes_compressed);

    let rate = bytes_compressed.get() as f32 / elapsed.as_secs_f32();
    let rate_human = match NumberPrefix::decimal(rate) {
        NumberPrefix::Standalone(bytes) => {
            format!("{} B/s", bytes)
        }
        NumberPrefix::Prefixed(prefix, n) => {
            format!("{:.3} {}B/s", n, prefix)
        }
    };

    format!(
        "{} in {:.2}s ({})",
        size_human,
        elapsed.as_secs_f32(),
        rate_human,
    )
}

#[must_use]
pub(crate) fn format_bytes(bytes: ByteNum) -> String {
    match NumberPrefix::decimal(bytes.get() as f32) {
        NumberPrefix::Standalone(bytes) => {
            format!("{} bytes", bytes)
        }
        NumberPrefix::Prefixed(prefix, n) => {
            format!("{:.2} {}B", n, prefix)
        }
    }
}
