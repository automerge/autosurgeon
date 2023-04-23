use std::mem;

use automerge::{ScalarValue, Value};
use uuid::Uuid;

use crate::{bytes::ByteArray, reconcile::LoadKey, Hydrate, HydrateError, ReadDoc, Reconcile};

impl Reconcile for Uuid {
    type Key<'a> = Uuid;

    fn reconcile<R: crate::Reconciler>(&self, reconciler: R) -> Result<(), R::Error> {
        ByteArray::from(*self.as_bytes()).reconcile(reconciler)
    }

    fn key(&self) -> LoadKey<Self::Key<'_>> {
        LoadKey::Found(*self)
    }

    fn hydrate_key<'a, D: ReadDoc>(
        doc: &D,
        obj: &automerge::ObjId,
        prop: crate::Prop<'_>,
    ) -> Result<LoadKey<Self::Key<'a>>, crate::ReconcileError> {
        Ok(match doc.get(obj, &prop)? {
            Some((Value::Scalar(s), _)) => {
                if let ScalarValue::Bytes(b) = s.as_ref() {
                    match Uuid::from_slice(b) {
                        Ok(u) => LoadKey::Found(u),
                        Err(_) => LoadKey::KeyNotFound,
                    }
                } else {
                    LoadKey::KeyNotFound
                }
            }
            _ => LoadKey::KeyNotFound,
        })
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

    #[test]
    fn uuid_keys() {
        let uuid0 = Uuid::new_v4();
        let uuid1 = Uuid::new_v4();

        // Vec with two UUIDs
        let mut doc1 = automerge::AutoCommit::new();
        reconcile_prop(&mut doc1, ObjId::Root, "vec", vec![uuid0, uuid1]).unwrap();

        // Fork document, remove first UUID in doc1, and second UUID in doc2
        let mut doc2 = doc1.fork();
        reconcile_prop(&mut doc1, ObjId::Root, "vec", vec![uuid1]).unwrap();
        reconcile_prop(&mut doc2, ObjId::Root, "vec", vec![uuid0]).unwrap();

        // Merge documents together: both UUIDs should be gone
        doc1.merge(&mut doc2).unwrap();
        let hydrated_vec: Vec<Uuid> = hydrate_prop(&doc1, ObjId::Root, "vec").unwrap();
        assert_eq!(Vec::<Uuid>::new(), hydrated_vec);
    }
}
