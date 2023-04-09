use std::{collections::HashMap, error, hash::Hash, str::FromStr};

use crate::{Hydrate, HydrateError, Prop, ReadDoc, Reconcile, Reconciler};
use automerge::ObjId;

pub fn reconcile<R, K, V>(items: &HashMap<K, V>, reconciler: R) -> Result<(), R::Error>
where
    R: Reconciler,
    K: ToString,
    V: Reconcile,
{
    super::reconcile_map_impl(items.iter(), reconciler)
}

pub fn hydrate<D, K, V>(doc: &D, obj: &ObjId, prop: Prop<'_>) -> Result<HashMap<K, V>, HydrateError>
where
    D: ReadDoc,
    K: FromStr + Eq + Hash,
    K::Err: error::Error + 'static,
    V: Hydrate,
{
    super::hydrate_map_impl(doc, obj, prop)?.collect()
}
