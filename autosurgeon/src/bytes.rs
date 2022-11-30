//! Newtypes for `[u8;N]` and `Vec<u8>` which encode to [`automerge::ScalarValue::Bytes`]
//!
//! This is necessary because otherwise we get conflicting implementations of `Reconcile` and
//! `Hydrate` when we implement these traits for `u8` and `Vec<u8>`.

use std::ops::Deref;

use crate::{Hydrate, HydrateError, Reconcile};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct ByteArray<const N: usize>([u8; N]);

impl<const N: usize> From<[u8; N]> for ByteArray<N> {
    fn from(b: [u8; N]) -> Self {
        Self(b)
    }
}

impl<const N: usize> From<ByteArray<N>> for [u8; N] {
    fn from(b: ByteArray<N>) -> Self {
        b.0
    }
}

impl<const N: usize> ByteArray<N> {
    fn expected() -> String {
        format!("a byte array of length {}", N)
    }
}

impl<const N: usize> Reconcile for ByteArray<N> {
    type Key<'a> = crate::reconcile::NoKey;

    fn reconcile<R: crate::Reconciler>(&self, mut reconciler: R) -> Result<(), R::Error> {
        reconciler.bytes(self.0)
    }
}

impl<const N: usize> Hydrate for ByteArray<N> {
    fn hydrate_bytes(bytes: &[u8]) -> Result<Self, HydrateError> {
        let raw = bytes.to_vec();
        let inner = raw.try_into().map_err(|e: Vec<u8>| {
            HydrateError::unexpected(Self::expected(), format!("an array of length {}", e.len()))
        })?;
        Ok(Self(inner))
    }
}

impl<const N: usize> Deref for ByteArray<N> {
    type Target = [u8; N];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<const N: usize> AsMut<[u8; N]> for ByteArray<N> {
    fn as_mut(&mut self) -> &mut [u8; N] {
        &mut self.0
    }
}

impl<const N: usize> AsRef<[u8; N]> for ByteArray<N> {
    fn as_ref(&self) -> &[u8; N] {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct ByteVec(Vec<u8>);

impl From<Vec<u8>> for ByteVec {
    fn from(b: Vec<u8>) -> Self {
        Self(b)
    }
}

impl From<ByteVec> for Vec<u8> {
    fn from(s: ByteVec) -> Self {
        s.0
    }
}

impl Reconcile for ByteVec {
    type Key<'a> = crate::reconcile::NoKey;

    fn reconcile<R: crate::Reconciler>(&self, mut reconciler: R) -> Result<(), R::Error> {
        reconciler.bytes(&self.0)
    }
}

impl Hydrate for ByteVec {
    fn hydrate_bytes(bytes: &[u8]) -> Result<Self, HydrateError> {
        Ok(Self(bytes.to_vec()))
    }
}

impl Deref for ByteVec {
    type Target = Vec<u8>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsMut<Vec<u8>> for ByteVec {
    fn as_mut(&mut self) -> &mut Vec<u8> {
        &mut self.0
    }
}

impl AsRef<Vec<u8>> for ByteVec {
    fn as_ref(&self) -> &Vec<u8> {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::{ByteArray, ByteVec};
    use automerge as am;

    use crate::{hydrate_prop, reconcile_prop};

    #[test]
    fn round_trip_array() {
        let mut doc = am::AutoCommit::new();
        let value: ByteArray<4> = [1_u8, 2, 3, 4].into();
        reconcile_prop(&mut doc, am::ROOT, "values", value).unwrap();

        let result: ByteArray<4> = hydrate_prop(&doc, am::ROOT, "values").unwrap();
        assert_eq!(result, value);
    }

    #[test]
    fn round_trip_vec() {
        let mut doc = am::AutoCommit::new();
        let value: ByteVec = vec![1_u8, 2, 3, 4].into();
        reconcile_prop(&mut doc, am::ROOT, "values", &value).unwrap();

        let result: ByteVec = hydrate_prop(&doc, am::ROOT, "values").unwrap();
        assert_eq!(result, value);
    }
}
