use std::collections::{BTreeMap, HashMap};

use crate::Reconcile;

use super::{LoadKey, MapReconciler};

impl<K, V> Reconcile for HashMap<K, V>
where
    K: AsRef<str>,
    V: Reconcile,
{
    type Key<'a> = super::NoKey;

    fn reconcile<R: crate::Reconciler>(&self, reconciler: R) -> Result<(), R::Error> {
        reconcile_map_impl(self.iter(), reconciler)
    }
}

impl<K, V> Reconcile for BTreeMap<K, V>
where
    K: AsRef<str>,
    V: Reconcile,
{
    type Key<'a> = super::NoKey;

    fn reconcile<R: crate::Reconciler>(&self, reconciler: R) -> Result<(), R::Error> {
        reconcile_map_impl(self.iter(), reconciler)
    }
}

fn reconcile_map_impl<
    'a,
    K: AsRef<str> + 'a,
    V: Reconcile + 'a,
    I: Iterator<Item = (&'a K, &'a V)>,
    R: crate::Reconciler,
>(
    items: I,
    mut reconciler: R,
) -> Result<(), R::Error> {
    let mut m = reconciler.map()?;
    for (k, val) in items {
        if let LoadKey::Found(new_key) = val.key() {
            if let LoadKey::Found(existing_key) = m.hydrate_entry_key::<V, _>(k)? {
                if existing_key != new_key {
                    m.replace(k, val)?;
                    continue;
                }
            }
        }
        m.put(k.as_ref(), val)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use automerge::ActorId;
    use automerge_test::{assert_doc, list, map};

    use crate::{
        reconcile,
        reconcile::{hydrate_key, LoadKey, MapReconciler},
        ReadDoc, Reconcile,
    };

    #[test]
    fn reconcile_map() {
        let mut map = HashMap::new();
        map.insert("key1", vec!["one", "two"]);
        map.insert("key2", vec!["three"]);
        let mut doc = automerge::AutoCommit::new();
        reconcile(&mut doc, &map).unwrap();
        assert_doc!(
            doc.document(),
            map! {
                "key1" => { list! { {"one"}, {"two"} }},
                "key2" => { list! { {"three"} }},
            }
        );
    }

    #[derive(Clone)]
    struct User {
        id: u64,
        name: &'static str,
    }

    impl Reconcile for User {
        type Key<'a> = u64;

        fn reconcile<R: crate::Reconciler>(&self, mut reconciler: R) -> Result<(), R::Error> {
            let mut m = reconciler.map()?;
            m.put("id", self.id)?;
            m.put("name", self.name)?;
            Ok(())
        }

        fn hydrate_key<'a, D: ReadDoc>(
            doc: &D,
            obj: &automerge::ObjId,
            prop: crate::Prop<'_>,
        ) -> Result<reconcile::LoadKey<Self::Key<'a>>, crate::ReconcileError> {
            hydrate_key::<_, u64>(doc, obj, prop, "id".into())
        }

        fn key(&self) -> LoadKey<Self::Key<'_>> {
            LoadKey::Found(self.id)
        }
    }

    #[test]
    fn reconcile_map_with_key() {
        let mut map = HashMap::new();
        map.insert("user", User { id: 1, name: "one" });
        let mut doc = automerge::AutoCommit::new();
        reconcile(&mut doc, &map).unwrap();

        let mut doc2 = doc.fork().with_actor(ActorId::random());
        let mut map2 = map.clone();
        map2.insert("user", User { id: 2, name: "two" });
        reconcile(&mut doc2, &map2).unwrap();

        map.insert(
            "user",
            User {
                id: 3,
                name: "three",
            },
        );
        reconcile(&mut doc, &map).unwrap();

        doc.merge(&mut doc2).unwrap();

        assert_doc!(
            doc.document(),
            map! {
                "user" => {
                    map! {
                        "id" => { 2_u64 },
                        "name" => { "two" },
                    },
                    map! {
                        "id" => { 3_u64 },
                        "name" => { "three" },
                    }
                }
            }
        );
    }
}
