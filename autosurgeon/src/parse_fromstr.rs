use std::{error, str::FromStr};

use automerge::{ObjType, Value};

use crate::{
    reconcile::{LoadKey, MapReconciler},
    Hydrate, HydrateError, Prop, Reconcile, Reconciler,
};

pub mod btree_map;
pub mod hash_map;

fn reconcile_map_impl<'a, K, V, I, R>(items: I, mut reconciler: R) -> Result<(), R::Error>
where
    K: ToString + 'a,
    V: Reconcile + 'a,
    I: Iterator<Item = (&'a K, &'a V)>,
    R: Reconciler,
{
    let mut m = reconciler.map()?;
    for (k, val) in items {
        let k = k.to_string();
        if let LoadKey::Found(new_key) = val.key() {
            if let LoadKey::Found(existing_key) = m.hydrate_entry_key::<V, _>(&k)? {
                if existing_key != new_key {
                    m.replace(k, val)?;
                    continue;
                }
            }
        }
        m.put(k, val)?;
    }
    Ok(())
}

fn hydrate_map_impl<'a, D, K, V>(
    doc: &'a D,
    obj: &automerge::ObjId,
    prop: Prop<'a>,
) -> Result<impl Iterator<Item = Result<(K, V), crate::HydrateError>> + 'a, crate::HydrateError>
where
    D: crate::ReadDoc,
    K: FromStr,
    K::Err: error::Error + 'static,
    V: Hydrate,
{
    let obj = match doc.get(obj, &prop)? {
        Some((Value::Object(ObjType::Map), id)) => id,
        _ => {
            return Err(HydrateError::unexpected(
                "a map",
                "something else".to_string(),
            ))
        }
    };
    let Some(obj_type) = doc.object_type(&obj) else {
        return Err(HydrateError::unexpected("a map", "a scalar value".to_string()))
    };
    match obj_type {
        ObjType::Map | ObjType::Table => {
            Ok(doc.map_range(obj.clone(), ..).map(move |(key, _, _)| {
                let val = V::hydrate(doc, &obj, key.into())?;
                let key_parsed: K = key
                    .parse()
                    .map_err(|err: K::Err| HydrateError::Parse(err.into()))?;
                Ok((key_parsed, val))
            }))
        }
        ObjType::Text => Err(HydrateError::unexpected(
            "a map",
            "a text object".to_string(),
        )),
        ObjType::List => Err(HydrateError::unexpected(
            "a map",
            "a list object".to_string(),
        )),
    }
}
