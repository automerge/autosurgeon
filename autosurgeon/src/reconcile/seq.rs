use super::{LoadKey, NoKey, Reconcile, Reconciler, SeqReconciler};

// This module implements an LCS diff for sequences when reconciling. Currently the implementation
// of the Hunt-Szymanski diff is from the `similar` crate. Consequenctly most of the types in this
// module are adapters to express the types from `reconcile` in a way which `similar` can work
// with.

impl<T: Reconcile> Reconcile for [T] {
    type Key<'a> = NoKey;
    fn reconcile<R: Reconciler>(&self, reconciler: R) -> Result<(), R::Error> {
        reconcile_seq(self, reconciler)
    }
}

impl<T: Reconcile> Reconcile for Vec<T> {
    type Key<'a> = NoKey;
    fn reconcile<R: Reconciler>(&self, reconciler: R) -> Result<(), R::Error> {
        reconcile_seq(self, reconciler)
    }
}

// Represents a key of an element in the document, we don't represent the actual element here
// because we don't want to hydrate the entire element from the document, just the key
struct OldElem<K> {
    key: K,
    index: usize,
}

// An element in the new sequence we are reconciling from
struct NewElem<'a, T> {
    elem: &'a T,
    index: usize,
}

// `similar::algorithms::lcs` requires that the new sequence elements implement `PartialEqual` with
// the old elements. By implementing this in terms of the key on the old and new elements we can
// get `similar` to do what we want
impl<'a: 'b, 'b, T: Reconcile> PartialEq<OldElem<LoadKey<T::Key<'b>>>> for NewElem<'a, T> {
    fn eq(&self, other: &OldElem<LoadKey<T::Key<'b>>>) -> bool {
        match (self.elem.key(), &other.key) {
            // Both elements had a key, just compare the keys
            (LoadKey::Found(k1), LoadKey::Found(k2)) => &k1 == k2,

            // One of the elements had a key, but the other didn't, they are not eqeual
            (LoadKey::Found(_), _) => false,
            (_, LoadKey::Found(_)) => false,

            // Neither element had a key, in this case we want to set both of them and diff
            // structurally
            (_, _) => self.index == other.index,
        }
    }
}

struct Hook<'a, T, S> {
    idx: usize,
    seq: &'a mut S,
    items: &'a [T],
}

impl<T, S> similar::algorithms::DiffHook for Hook<'_, T, S>
where
    T: Reconcile,
    S: SeqReconciler,
{
    type Error = S::Error;
    fn equal(
        &mut self,
        _old_index: usize,
        new_index: usize,
        len: usize,
    ) -> Result<(), Self::Error> {
        for elem in &self.items[new_index..(new_index + len)] {
            self.seq.set(self.idx, elem)?;
            self.idx += 1;
        }
        Ok(())
    }

    fn delete(
        &mut self,
        _old_index: usize,
        old_len: usize,
        _new_index: usize,
    ) -> Result<(), Self::Error> {
        for _ in 0..old_len {
            self.seq.delete(self.idx)?;
        }
        Ok(())
    }

    fn insert(
        &mut self,
        _old_index: usize,
        new_index: usize,
        new_len: usize,
    ) -> Result<(), Self::Error> {
        for elem in &self.items[new_index..(new_index + new_len)] {
            self.seq.insert(self.idx, elem)?;
            self.idx += 1;
        }
        Ok(())
    }
}

fn reconcile_seq<T, R>(items: &[T], mut reconciler: R) -> Result<(), R::Error>
where
    T: Reconcile,
    R: Reconciler,
    R::Error: std::fmt::Debug,
{
    let mut seq = reconciler.seq()?;

    let old_len = seq.len()?;
    let old_keys = (0..old_len).try_fold::<_, _, Result<_, R::Error>>(
        Vec::with_capacity(old_len),
        |mut items, i| {
            items.push(OldElem {
                key: seq.hydrate_item_key::<T>(i)?,
                index: i,
            });
            Ok(items)
        },
    )?;

    let new = items
        .iter()
        .enumerate()
        .map(|(i, e)| NewElem { elem: e, index: i })
        .collect::<Vec<_>>();

    let mut hook = Hook {
        idx: 0,
        items,
        seq: &mut seq,
    };

    similar::algorithms::lcs::diff(&mut hook, &old_keys, 0..old_len, &new, 0..items.len())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::{
        reconcile::{LoadKey, MapReconciler},
        reconcile_prop, ReadDoc,
    };
    use automerge_test::{assert_doc, list, map};
    use std::borrow::Cow;

    #[test]
    fn test_reconcile_slice_deletes_extra_elems() {
        let mut vals = vec![1, 2, 3];
        let mut doc = automerge::AutoCommit::new();
        reconcile_prop(&mut doc, automerge::ROOT, "vals", &vals).unwrap();
        vals.remove(1);
        reconcile_prop(&mut doc, automerge::ROOT, "vals", &vals).unwrap();
        assert_doc!(
            doc.document(),
            map! {
                "vals" => { list! {
                    { 1 },
                    { 3 }
                }}
            }
        )
    }

    #[test]
    fn test_reconcile_vec_deletes_extra_elems() {
        let mut vals = vec![1, 2, 3];
        let mut doc = automerge::AutoCommit::new();
        reconcile_prop(&mut doc, automerge::ROOT, "vals", vals.clone()).unwrap();
        vals.remove(1);
        reconcile_prop(&mut doc, automerge::ROOT, "vals", vals).unwrap();
        assert_doc!(
            doc.document(),
            map! {
                "vals" => { list! {
                    { 1 },
                    { 3 }
                }}
            }
        )
    }

    #[derive(Clone, Debug, PartialEq)]
    struct Person {
        id: String,
        name: String,
    }

    impl crate::Reconcile for Person {
        type Key<'a> = Cow<'a, String>;

        fn reconcile<R: crate::Reconciler>(&self, mut reconciler: R) -> Result<(), R::Error> {
            let mut m = reconciler.map()?;
            m.put("name", &self.name)?;
            m.put("id", &self.id)?;
            Ok(())
        }

        fn hydrate_key<'a, D: ReadDoc>(
            doc: &D,
            obj: &automerge::ObjId,
            prop: crate::prop::Prop<'_>,
        ) -> Result<crate::reconcile::LoadKey<Self::Key<'a>>, crate::ReconcileError> {
            let result = crate::reconcile::hydrate_key(doc, obj, prop, "id".into())?;
            Ok(result)
        }

        fn key(&self) -> LoadKey<Self::Key<'_>> {
            LoadKey::Found(Cow::Borrowed(&self.id))
        }
    }

    #[test]
    fn test_reconcile_vec_with_key() {
        let mut vals = vec![
            Person {
                id: "one".to_string(),
                name: "Burt".to_string(),
            },
            Person {
                id: "two".to_string(),
                name: "Winston".to_string(),
            },
        ];
        let mut doc = automerge::AutoCommit::new();
        reconcile_prop(&mut doc, automerge::ROOT, "people", &vals).unwrap();

        let mut doc2 = doc.fork().with_actor("actor2".as_bytes().into());
        let mut vals2 = vals.clone();
        vals2.insert(
            0,
            Person {
                id: "three".to_string(),
                name: "Charlotte".to_string(),
            },
        );
        reconcile_prop(&mut doc2, automerge::ROOT, "people", &vals2).unwrap();

        vals.remove(1);
        reconcile_prop(&mut doc, automerge::ROOT, "people", &vals).unwrap();

        doc.merge(&mut doc2).unwrap();

        assert_doc!(
            doc.document(),
            map! {
                "people" => { list! {
                    { map! {
                        "id" => { "three" },
                        "name" => { "Charlotte" },
                   }},
                    { map! {
                        "id" => { "one" },
                        "name" => { "Burt" },
                   }}
                }}
            }
        )
    }
}
