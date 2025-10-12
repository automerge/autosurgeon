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

/*
struct NewElemSimple<K> {
    key: K,
    index: usize,
}

// `similar::algorithms::lcs` requires that the new sequence elements implement `PartialEqual` with
// the old elements. By implementing this in terms of the key on the old and new elements we can
// get `similar` to do what we want
impl<'a: 'b, 'b, K> PartialEq<OldElem<LoadKey<K>>> for NewElemSimple<LoadKey<K>>
where
    K: PartialEq,
{
    fn eq(&self, other: &OldElem<LoadKey<K>>) -> bool {
        match (&self.key, &other.key) {
            // Both elements had a key, just compare the keys
            (LoadKey::Found(k1), LoadKey::Found(k2)) => k1 == k2,

            // One of the elements had a key, but the other didn't, they are not eqeual
            (LoadKey::Found(_), _) => false,
            (_, LoadKey::Found(_)) => false,

            // Neither element had a key, in this case we want to set both of them and diff
            // structurally
            (_, _) => self.index == other.index,
        }
    }
}
*/

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

macro_rules! tuple_impl {
    ($hook_ar:ident $(, $idx:tt $ty:tt)*) => {
        // we assume that it starts at N1 and use N0 directly
        impl<'k, N0, $($ty,)*> Reconcile for (N0, $($ty,)*)
        where
            N0: Reconcile,
            // we force all the remainig types to have the same key
            $($ty: Reconcile<Key<'k> = <N0 as Reconcile>::Key<'k>>,)*
        {
            // constrain the key to that of N0
            type Key<'a> = <N0 as Reconcile>::Key<'a>;

            fn reconcile<R: Reconciler>(&self, mut reconciler: R) -> Result<(), R::Error>
            {
                let mut seq = reconciler.seq()?;
                let old_len = seq.len()?;
                let arity = 0;
                $(
                    let arity = arity.max($idx);
                )*
                for ii in (arity + 1)..old_len {
                    seq.delete(ii)?;
                }
                if 0 < old_len {
                    seq.set(0, &self.0)?;
                } else {
                    seq.insert(0, &self.0)?;
                }
                $(
                    if $idx < old_len {
                        seq.set($idx, &self.$idx)?;
                    } else {
                        seq.insert($idx, &self.$idx)?;
                    }
                )*

                /*
                let mut seq = reconciler.seq()?;

                let old_len = seq.len()?;
                let arity = 0;
                $(
                    let arity = arity.max($idx);
                )*
                let old_keys = (0..old_len).try_fold::<_, _, Result<_, R::Error>>(
                    Vec::with_capacity(old_len),
                    |mut items, ii| {
                        $(
                            if ii == 0 {
                                items.push(OldElem {
                                    key: seq.hydrate_item_key::<N0>(0)?,
                                    index: 0,
                                });
                            }
                            if ii == $idx {
                                items.push(OldElem {
                                    key: seq.hydrate_item_key::<$ty>($idx)?,
                                    index: $idx,
                                });
                            }
                        )*;
                        Ok(items)
                    },
                )?;

                let new = vec![
                    NewElemSimple { key: self.0.key(), index: 0 },
                    $(NewElemSimple { key: self.$idx.key(), index: $idx },)*
                ];


                let mut hook = pastey::paste! {[<$hook_ar  Hook>]} {
                    idx: 0,
                    seq: &mut seq,
                    tuple: self,
                };

                similar::algorithms::lcs::diff(&mut hook, &old_keys, 0..old_len, &new, 0..arity)?;
                */
                Ok(())
            }
        }

        /*
        pastey::paste! {
            struct [<$hook_ar  Hook>]<'a, $($ty, )+ S> {
                idx: usize,
                seq: &'a mut S,
                tuple: &'a($($ty, )+),
            }

            impl<'a, $($ty, )+ S> similar::algorithms::DiffHook for [<$hook_ar  Hook>]<'a, $($ty, )+ S>
            where
                $($ty: Reconcile,)+
                S: SeqReconciler,
            {
                type Error = S::Error;
                fn equal(
                    &mut self,
                    _old_index: usize,
                    new_index: usize,
                    len: usize,
                ) -> Result<(), Self::Error> {
                    $(if $idx >= new_index && $idx < (new_index + len){
                        self.seq.set(self.idx, &self.tuple.$idx)?;
                        self.idx += 1;
                    })+;
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
                    $(if $idx >= new_index && $idx < (new_index + new_len){
                        self.seq.insert(self.idx, &self.tuple.$idx)?;
                        self.idx += 1;
                    })+;
                    Ok(())
                }
            }
        }
        */
    }
}

tuple_impl!(I);
tuple_impl!(II, 1 N1);
tuple_impl!(III, 1 N1, 2 N2);
tuple_impl!(IV, 1 N1, 2 N2, 3 N3);
tuple_impl!(V, 1 N1, 2 N2, 3 N3, 4 N4);
tuple_impl!(VI, 1 N1, 2 N2, 3 N3, 4 N4, 5 N5);
tuple_impl!(VII, 1 N1, 2 N2, 3 N3, 4 N4, 5 N5, 6 N6);
tuple_impl!(VIII, 1 N1, 2 N2, 3 N3, 4 N4, 5 N5, 6 N6, 7 N7);
tuple_impl!(IX, 1 N1, 2 N2, 3 N3, 4 N4, 5 N5, 6 N6, 7 N7, 8 N8);
tuple_impl!(X, 1 N1, 2 N2, 3 N3, 4 N4, 5 N5, 6 N6, 7 N7, 8 N8, 9 N9);
tuple_impl!(XI, 1 N1, 2 N2, 3 N3, 4 N4, 5 N5, 6 N6, 7 N7, 8 N8, 9 N9, 10 N10);
tuple_impl!(XII, 1 N1, 2 N2, 3 N3, 4 N4, 5 N5, 6 N6, 7 N7, 8 N8, 9 N9, 10 N10, 11 N11);
tuple_impl!(XIII, 1 N1, 2 N2, 3 N3, 4 N4, 5 N5, 6 N6, 7 N7, 8 N8, 9 N9, 10 N10, 11 N11, 12 N12);
tuple_impl!(XIV, 1 N1, 2 N2, 3 N3, 4 N4, 5 N5, 6 N6, 7 N7, 8 N8, 9 N9, 10 N10, 11 N11, 12 N12, 13 N13);
tuple_impl!(XV, 1 N1, 2 N2, 3 N3, 4 N4, 5 N5, 6 N6, 7 N7, 8 N8, 9 N9, 10 N10, 11 N11, 12 N12, 13 N13, 14 N14);
tuple_impl!(XVI, 1 N1, 2 N2, 3 N3, 4 N4, 5 N5, 6 N6, 7 N7, 8 N8, 9 N9, 10 N10, 11 N11, 12 N12, 13 N13, 14 N14, 15 N15);
tuple_impl!(XVII, 1 N1, 2 N2, 3 N3, 4 N4, 5 N5, 6 N6, 7 N7, 8 N8, 9 N9, 10 N10, 11 N11, 12 N12, 13 N13, 14 N14, 15 N15, 16 N16);
tuple_impl!(XVIII, 1 N1, 2 N2, 3 N3, 4 N4, 5 N5, 6 N6, 7 N7, 8 N8, 9 N9, 10 N10, 11 N11, 12 N12, 13 N13, 14 N14, 15 N15, 16 N16, 17 N17);
tuple_impl!(XIX, 1 N1, 2 N2, 3 N3, 4 N4, 5 N5, 6 N6, 7 N7, 8 N8, 9 N9, 10 N10, 11 N11, 12 N12, 13 N13, 14 N14, 15 N15, 16 N16, 17 N17, 18 N18);
tuple_impl!(XX, 1 N1, 2 N2, 3 N3, 4 N4, 5 N5, 6 N6, 7 N7, 8 N8, 9 N9, 10 N10, 11 N11, 12 N12, 13 N13, 14 N14, 15 N15, 16 N16, 17 N17, 18 N18, 19 N19);
tuple_impl!(XXI, 1 N1, 2 N2, 3 N3, 4 N4, 5 N5, 6 N6, 7 N7, 8 N8, 9 N9, 10 N10, 11 N11, 12 N12, 13 N13, 14 N14, 15 N15, 16 N16, 17 N17, 18 N18, 19 N19, 20 N20);
tuple_impl!(XXII, 1 N1, 2 N2, 3 N3, 4 N4, 5 N5, 6 N6, 7 N7, 8 N8, 9 N9, 10 N10, 11 N11, 12 N12, 13 N13, 14 N14, 15 N15, 16 N16, 17 N17, 18 N18, 19 N19, 20 N20, 21 N21);
tuple_impl!(XXIII, 1 N1, 2 N2, 3 N3, 4 N4, 5 N5, 6 N6, 7 N7, 8 N8, 9 N9, 10 N10, 11 N11, 12 N12, 13 N13, 14 N14, 15 N15, 16 N16, 17 N17, 18 N18, 19 N19, 20 N20, 21 N21, 22 N22);
tuple_impl!(XXIV, 1 N1, 2 N2, 3 N3, 4 N4, 5 N5, 6 N6, 7 N7, 8 N8, 9 N9, 10 N10, 11 N11, 12 N12, 13 N13, 14 N14, 15 N15, 16 N16, 17 N17, 18 N18, 19 N19, 20 N20, 21 N21, 22 N22, 23 N23);
tuple_impl!(XXV, 1 N1, 2 N2, 3 N3, 4 N4, 5 N5, 6 N6, 7 N7, 8 N8, 9 N9, 10 N10, 11 N11, 12 N12, 13 N13, 14 N14, 15 N15, 16 N16, 17 N17, 18 N18, 19 N19, 20 N20, 21 N21, 22 N22, 23 N23, 24 N24);

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

    #[test]
    fn reconcile_tuple_2_round_trip() {
        let mut doc = automerge::AutoCommit::new();
        crate::reconcile_prop(&mut doc, automerge::ROOT, "t", &(1u64, 2u64)).unwrap();
        // Update tuple
        crate::reconcile_prop(&mut doc, automerge::ROOT, "t", &(10u64, 20u64)).unwrap();
        use automerge_test::{assert_doc, list, map};
        assert_doc!(
            doc.document(),
            map! { "t" => { list! { { 10_u64 }, { 20_u64 } } } }
        );
    }

    #[test]
    fn reconcile_tuple_3_insert_delete() {
        let mut doc = automerge::AutoCommit::new();
        crate::reconcile_prop(&mut doc, automerge::ROOT, "t", &(1u64, 2u64, 3u64)).unwrap();
        // Shorten to 2
        crate::reconcile_prop(&mut doc, automerge::ROOT, "t", &(7u64, 8u64)).unwrap();
        use automerge_test::{assert_doc, list, map};
        assert_doc!(
            doc.document(),
            map! { "t" => { list! { { 7_u64 }, { 8_u64 } } } }
        );
    }
}
