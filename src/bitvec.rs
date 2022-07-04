//! Bit vector implementation.

use std::{fmt::Debug, ops::Index};

const BITS: usize = usize::BITS as usize;

/// Simple implementation of a bit vector on a [`Vec<usize>`].
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct BitVec {
    /// Unused bits are always zero.
    data: Vec<usize>,
    length: usize,
}

impl BitVec {
    pub(crate) fn new(length: usize) -> Self {
        let size = div_ceil(length, BITS);
        BitVec {
            data: vec![0; size],
            length,
        }
    }

    pub(crate) fn set(&mut self, index: usize, value: bool) {
        assert!(index < self.length, "index out of range");
        let mask = 1 << (index % BITS);
        let container = &mut self.data[index / BITS];
        if value {
            *container |= mask;
        } else {
            *container &= !mask;
        }
    }

    /// Copy data into self without allocating.
    ///
    /// # Panics
    /// Panics if lengths mismatch.
    pub(crate) fn copy_from_bitvec(&mut self, other: &BitVec) {
        assert_eq!(self.length, other.length, "BitVec lengths do not match");
        self.data.copy_from_slice(&other.data);
    }

    /// Fills `self` with zeros.
    pub(crate) fn reset(&mut self) {
        self.data.fill(0);
    }

    #[allow(dead_code)]
    pub(crate) fn len(&self) -> usize {
        self.length
    }

    /// Is any bit set?
    #[allow(dead_code)]
    pub(crate) fn any(&self) -> bool {
        self.data.iter().any(|&c| c != 0)
    }

    pub(crate) fn iter(&self) -> Iter<'_> {
        Iter {
            bitvec: self,
            current: 0,
        }
    }
}

impl Index<usize> for BitVec {
    type Output = bool;

    fn index(&self, index: usize) -> &Self::Output {
        assert!(index < self.length, "index out of range");
        if self.data[index / BITS] & (1 << (index % BITS)) != 0 {
            &true
        } else {
            &false
        }
    }
}

impl Debug for BitVec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.iter().map(u8::from)).finish()
    }
}

pub(crate) struct Iter<'b> {
    bitvec: &'b BitVec,
    current: usize,
}

impl<'b> Iterator for Iter<'b> {
    type Item = bool;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current >= self.bitvec.length {
            return None;
        }

        let result = self.bitvec[self.current];
        self.current += 1;
        Some(result)
    }
}

/// Ceiling division
fn div_ceil(dividend: usize, divisor: usize) -> usize {
    dividend / divisor + usize::from(dividend % divisor != 0)
}
