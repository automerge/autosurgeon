use std::mem;

use uuid::Uuid;

use crate::{bytes::ByteArray, Hydrate, HydrateError, Reconcile};

impl Reconcile for Uuid {
    type Key<'a> = <ByteArray<{ mem::size_of::<Uuid>() }> as Reconcile>::Key<'a>;

    fn reconcile<R: crate::Reconciler>(&self, reconciler: R) -> Result<(), R::Error> {
        ByteArray::from(*self.as_bytes()).reconcile(reconciler)
    }
}

impl Hydrate for Uuid {
    fn hydrate_bytes(bytes: &[u8]) -> Result<Self, HydrateError> {
        let array = ByteArray::<{ mem::size_of::<Uuid>() }>::hydrate_bytes(bytes)?;
        Ok(Uuid::from_bytes(*array))
    }
}
