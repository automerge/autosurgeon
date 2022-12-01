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

#[cfg(test)]
mod tests {
    use automerge::ObjId;
    use uuid::Uuid;

    use crate::{hydrate_prop, reconcile_prop};

    #[test]
    fn round_trip_uuids() {
        let mut doc = automerge::AutoCommit::new();

        let uuid = Uuid::new_v4();
        reconcile_prop(&mut doc, ObjId::Root, "secret", uuid).unwrap();

        let hydrated_uuid = hydrate_prop(&doc, ObjId::Root, "secret").unwrap();

        assert_eq!(uuid, hydrated_uuid);
    }
}
