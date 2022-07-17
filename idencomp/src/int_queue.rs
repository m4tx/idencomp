/// Simple queue with maximum length, represented by a single integer.
///
/// This is intended to be used in hot paths, hence all of the methods are
/// `const` and blazingly fast.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
#[repr(transparent)]
pub(crate) struct IntQueue<const MAX_SINGLE_VAL: u32, const LENGTH: usize>(u32);

impl<const MAX_SINGLE_VAL: u32, const LENGTH: usize> IntQueue<MAX_SINGLE_VAL, LENGTH> {
    #[inline(always)]
    #[must_use]
    pub const fn with_default(value: u32) -> Self {
        Self::with_state(Self::calc_default_state(0, value, LENGTH))
    }

    #[inline(always)]
    #[must_use]
    pub const fn with_state(state: u32) -> Self {
        Self(state)
    }

    #[inline(always)]
    #[must_use]
    const fn calc_default_state(cur_state: u32, value: u32, length: usize) -> u32 {
        if length == 0 {
            0
        } else {
            Self::calc_default_state(cur_state * MAX_SINGLE_VAL + value, value, length - 1)
        }
    }

    #[inline(always)]
    #[must_use]
    pub const fn get(&self) -> u32 {
        self.0
    }

    #[inline(always)]
    #[must_use]
    pub const fn num_bits() -> u32 {
        let max_val = MAX_SINGLE_VAL.pow(LENGTH as u32) - 1;
        32 - max_val.leading_zeros()
    }

    #[inline(always)]
    #[must_use]
    pub const fn mask() -> u32 {
        (1 << Self::num_bits()) - 1
    }

    #[inline(always)]
    #[must_use]
    const fn last_pow() -> u32 {
        if LENGTH == 0 {
            0
        } else {
            MAX_SINGLE_VAL.pow(LENGTH as u32 - 1)
        }
    }

    #[inline(always)]
    #[must_use]
    pub const fn with_pushed_back(&self, value: u32) -> Self {
        if LENGTH == 0 {
            return *self;
        }

        let new_value = self.0 % Self::last_pow() * MAX_SINGLE_VAL + value;
        IntQueue(new_value)
    }

    #[inline(always)]
    #[must_use]
    pub const fn with_popped_back(&self) -> Self {
        let new_value = self.0 / MAX_SINGLE_VAL;
        IntQueue(new_value)
    }

    #[inline(always)]
    #[must_use]
    pub const fn back(&self) -> u32 {
        assert!(LENGTH > 0);

        self.0 % MAX_SINGLE_VAL
    }
}
