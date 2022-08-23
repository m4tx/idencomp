use std::fmt::Debug;

use derive_more::{Add, AddAssign};

/// An integer number of bytes.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Add, AddAssign)]
#[repr(transparent)]
pub struct ByteNum(usize);

impl ByteNum {
    /// The `ByteNum` instance with a value of `0`.
    pub const ZERO: ByteNum = ByteNum(0);

    /// Creates a new `ByteNum` instance.
    ///
    /// # Examples
    /// ```
    /// use idencomp::progress::ByteNum;
    ///
    /// let bytes = ByteNum::new(123);
    /// assert_eq!(bytes.get(), 123);
    /// ```
    #[inline]
    #[must_use]
    pub const fn new(bytes: usize) -> Self {
        Self(bytes)
    }

    /// Returns the value of this `ByteNum` instance.
    ///
    /// # Examples
    /// ```
    /// use idencomp::progress::ByteNum;
    ///
    /// let bytes = ByteNum::new(123);
    /// assert_eq!(bytes.get(), 123);
    /// ```
    #[inline]
    #[must_use]
    pub const fn get(&self) -> usize {
        self.0
    }
}

/// An object that can track progress of any long-running operation. This can be
/// implemented e.g. as a progress bar in CLI/GUI.
pub trait ProgressNotifier: Debug + Send + Sync {
    /// Indicates that the specified number of bytes has been processed.
    fn processed_bytes(&self, bytes: ByteNum);

    /// Indicates that there will be specified number of items to be processed
    /// (so that the maximum value for a progress bar can be set).
    fn set_iter_num(&self, num_iter: u64);

    /// Indicates that a single item has been processed. This is usually used
    /// after calling [`Self::set_iter_num()`].
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

/// A no-operation implementation of [`ProgressNotifier`].
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
