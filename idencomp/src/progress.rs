use std::fmt::Debug;

use derive_more::{Add, AddAssign};

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Add, AddAssign)]
#[repr(transparent)]
pub struct ByteNum(usize);

impl ByteNum {
    pub const ZERO: ByteNum = ByteNum(0);

    #[inline]
    #[must_use]
    pub const fn new(bytes: usize) -> Self {
        Self(bytes)
    }

    #[inline]
    #[must_use]
    pub const fn get(&self) -> usize {
        self.0
    }
}

pub trait ProgressNotifier: Debug + Send + Sync {
    fn processed_bytes(&self, bytes: ByteNum);

    fn set_iter_num(&self, num_iter: u64);

    fn inc_iter(&self);
}

impl<T: ProgressNotifier> ProgressNotifier for &T {
    fn processed_bytes(&self, bytes: ByteNum) {
        T::processed_bytes(self, bytes)
    }

    fn set_iter_num(&self, num_iter: u64) {
        T::set_iter_num(self, num_iter)
    }

    fn inc_iter(&self) {
        T::inc_iter(self)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct DummyProgressNotifier;

impl ProgressNotifier for DummyProgressNotifier {
    fn processed_bytes(&self, _bytes: ByteNum) {
        // do nothing
    }

    fn set_iter_num(&self, _num_iter: u64) {
        // do nothing
    }

    fn inc_iter(&self) {
        // do nothing
    }
}

#[cfg(test)]
mod tests {
    use crate::progress::{ByteNum, DummyProgressNotifier, ProgressNotifier};

    #[test]
    fn test_dummy_progress_notifier() {
        let notifier = DummyProgressNotifier;
        notifier.processed_bytes(ByteNum::new(1337));
        let notifier_2 = notifier;
        notifier_2.processed_bytes(ByteNum::new(666));
    }
}
