use std::borrow::Cow;

use automerge as am;

use crate::reconcile::{
    CounterReconciler, LoadKey, MapReconciler, Reconciler, SeqReconciler, StaleHeads,
    TextReconciler,
};
use crate::Reconcile;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum HydrateReconcileError {
    #[error(transparent)]
    StaleHeads(#[from] StaleHeads),
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Convert a reference to a `hydrate::Value` into an owned `am::ValueRef<'static>`.
///
/// For scalars the inner `ScalarValue` is cloned; for objects only the type tag
/// is propagated.
fn hydrate_value_to_value_ref(value: &am::hydrate::Value) -> am::ValueRef<'static> {
    match value {
        am::hydrate::Value::Scalar(s) => am::ValueRef::from(s.clone()),
        am::hydrate::Value::Map(_) => am::ValueRef::Object(am::ObjType::Map),
        am::hydrate::Value::List(_) => am::ValueRef::Object(am::ObjType::List),
        am::hydrate::Value::Text(_) => am::ValueRef::Object(am::ObjType::Text),
    }
}

/// Convert a reference to a `hydrate::Value` into an `am::Value<'static>`.
fn hydrate_value_to_value(value: &am::hydrate::Value) -> am::Value<'static> {
    am::Value::from(value)
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Reconcile a value into an [`am::hydrate::Value`] without going through an
/// Automerge document.
pub fn reconcile_to_am_hydrate<R: Reconcile>(val: R) -> Option<am::hydrate::Value> {
    let mut result: Option<am::hydrate::Value> = None;
    let reconciler = ValueReconciler { value: &mut result };
    val.reconcile(reconciler)
        .expect("reconcile into hydrate::Value should not fail");
    result
}

/// Internal helper – reconcile a nested value and return the resulting
/// [`am::hydrate::Value`].
fn reconcile_inner<R: Reconcile>(value: R) -> Result<Option<am::hydrate::Value>, HydrateReconcileError> {
    let mut result = None;
    let reconciler = ValueReconciler { value: &mut result };
    value.reconcile(reconciler)?;
    Ok(result)
}

// ---------------------------------------------------------------------------
// ValueReconciler – implements Reconciler, writes into &mut Option<Value>
// ---------------------------------------------------------------------------

struct ValueReconciler<'a> {
    value: &'a mut Option<am::hydrate::Value>,
}

impl<'a> Reconciler for ValueReconciler<'a> {
    type Error = HydrateReconcileError;

    type Map<'m>
        = HydrateMapReconciler<'m>
    where
        Self: 'm;

    type Seq<'s>
        = HydrateSeqReconciler<'s>
    where
        Self: 's;

    type Text<'t>
        = HydrateTextReconciler<'t>
    where
        Self: 't;

    type Counter<'c>
        = HydrateCounterReconciler<'c>
    where
        Self: 'c;

    fn none(&mut self) -> Result<(), Self::Error> {
        *self.value = Some(am::hydrate::Value::Scalar(am::ScalarValue::Null));
        Ok(())
    }

    fn bytes<B: AsRef<[u8]>>(&mut self, value: B) -> Result<(), Self::Error> {
        *self.value = Some(am::hydrate::Value::Scalar(am::ScalarValue::Bytes(
            value.as_ref().to_vec(),
        )));
        Ok(())
    }

    fn timestamp(&mut self, value: i64) -> Result<(), Self::Error> {
        *self.value = Some(am::hydrate::Value::Scalar(am::ScalarValue::Timestamp(
            value,
        )));
        Ok(())
    }

    fn boolean(&mut self, value: bool) -> Result<(), Self::Error> {
        *self.value = Some(am::hydrate::Value::Scalar(am::ScalarValue::Boolean(value)));
        Ok(())
    }

    fn str<S: AsRef<str>>(&mut self, value: S) -> Result<(), Self::Error> {
        *self.value = Some(am::hydrate::Value::Scalar(am::ScalarValue::Str(
            value.as_ref().into(),
        )));
        Ok(())
    }

    fn u64(&mut self, value: u64) -> Result<(), Self::Error> {
        *self.value = Some(am::hydrate::Value::Scalar(am::ScalarValue::Uint(value)));
        Ok(())
    }

    fn i64(&mut self, value: i64) -> Result<(), Self::Error> {
        *self.value = Some(am::hydrate::Value::Scalar(am::ScalarValue::Int(value)));
        Ok(())
    }

    fn f64(&mut self, value: f64) -> Result<(), Self::Error> {
        *self.value = Some(am::hydrate::Value::Scalar(am::ScalarValue::F64(value)));
        Ok(())
    }

    fn map(&mut self) -> Result<Self::Map<'_>, Self::Error> {
        *self.value = Some(am::hydrate::Value::Map(am::hydrate::Map::default()));
        match self.value.as_mut() {
            Some(am::hydrate::Value::Map(m)) => Ok(HydrateMapReconciler { map: m }),
            _ => unreachable!(),
        }
    }

    fn seq(&mut self) -> Result<Self::Seq<'_>, Self::Error> {
        Ok(HydrateSeqReconciler {
            items: Vec::new(),
            target: self.value,
        })
    }

    fn text(&mut self) -> Result<Self::Text<'_>, Self::Error> {
        Ok(HydrateTextReconciler {
            text: String::new(),
            target: self.value,
        })
    }

    fn counter(&mut self) -> Result<Self::Counter<'_>, Self::Error> {
        Ok(HydrateCounterReconciler {
            value: 0,
            target: self.value,
        })
    }

    fn heads(&self) -> &[am::ChangeHash] {
        // There is no backing document, so there are no heads.
        &[]
    }
}

// ---------------------------------------------------------------------------
// HydrateMapReconciler
// ---------------------------------------------------------------------------

struct HydrateMapReconciler<'a> {
    map: &'a mut am::hydrate::Map,
}

impl MapReconciler for HydrateMapReconciler<'_> {
    type Error = HydrateReconcileError;

    type EntriesIter<'e>
        = HydrateMapEntriesIter<'e>
    where
        Self: 'e;

    fn entries(&self) -> Self::EntriesIter<'_> {
        let entries: Vec<_> = self
            .map
            .iter()
            .map(|(k, v)| {
                let key = Cow::Borrowed(k.as_str());
                let val = hydrate_value_to_value_ref(&v.value);
                (key, val)
            })
            .collect();
        HydrateMapEntriesIter {
            inner: entries.into_iter(),
        }
    }

    fn entry<P: AsRef<str>>(&self, prop: P) -> Option<am::Value<'_>> {
        self.map
            .get(prop.as_ref())
            .map(hydrate_value_to_value)
    }

    fn put<R: Reconcile, P: AsRef<str>>(&mut self, prop: P, value: R) -> Result<(), Self::Error> {
        let Some(hydrated) = reconcile_inner(value)? else {
            return Ok(());
        };
        // Map derefs to HashMap<String, MapValue> via DerefMut, so we can
        // insert directly.
        self.map.insert(
            prop.as_ref().to_string(),
            am::hydrate::MapValue {
                value: hydrated,
                conflict: false,
            },
        );
        Ok(())
    }

    fn delete<P: AsRef<str>>(&mut self, prop: P) -> Result<(), Self::Error> {
        self.map.remove(prop.as_ref());
        Ok(())
    }

    fn hydrate_entry_key<'k, R: Reconcile, P: AsRef<str>>(
        &self,
        _prop: P,
    ) -> Result<LoadKey<R::Key<'k>>, Self::Error> {
        // No underlying document to hydrate keys from.
        Ok(LoadKey::KeyNotFound)
    }
}

// Iterator adapter that converts map entries into `(Cow<str>, ValueRef)`.
//
// `Map::iter()` returns an opaque `impl Iterator`, so we collect into a `Vec`
// eagerly and hand out an `IntoIter`.
struct HydrateMapEntriesIter<'a> {
    inner: std::vec::IntoIter<(Cow<'a, str>, am::ValueRef<'a>)>,
}

impl<'a> Iterator for HydrateMapEntriesIter<'a> {
    type Item = (Cow<'a, str>, am::ValueRef<'a>);

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

// ---------------------------------------------------------------------------
// HydrateSeqReconciler
//
// `am::hydrate::List` does not expose public insertion methods, so we
// accumulate items in a `Vec` and convert to `List` via `From<Vec<Value>>`
// when the reconciler is dropped.
// ---------------------------------------------------------------------------

struct HydrateSeqReconciler<'a> {
    items: Vec<am::hydrate::Value>,
    target: &'a mut Option<am::hydrate::Value>,
}

impl Drop for HydrateSeqReconciler<'_> {
    fn drop(&mut self) {
        let items = std::mem::take(&mut self.items);
        *self.target = Some(am::hydrate::Value::List(am::hydrate::List::from(items)));
    }
}

impl SeqReconciler for HydrateSeqReconciler<'_> {
    type Error = HydrateReconcileError;

    type ItemIter<'i>
        = HydrateSeqItemIter<'i>
    where
        Self: 'i;

    fn items(&self) -> Self::ItemIter<'_> {
        HydrateSeqItemIter {
            inner: self.items.iter(),
        }
    }

    fn get(&self, index: usize) -> Result<Option<am::Value<'_>>, Self::Error> {
        Ok(self.items.get(index).map(|v| hydrate_value_to_value(v)))
    }

    fn hydrate_item_key<'k, R: Reconcile>(
        &self,
        _index: usize,
    ) -> Result<LoadKey<R::Key<'k>>, Self::Error> {
        // No underlying document to hydrate keys from.
        Ok(LoadKey::KeyNotFound)
    }

    fn insert<R: Reconcile>(&mut self, index: usize, value: R) -> Result<(), Self::Error> {
        if let Some(hydrated) = reconcile_inner(value)? {
            self.items.insert(index, hydrated);
        }
        Ok(())
    }

    fn set<R: Reconcile>(&mut self, index: usize, value: R) -> Result<(), Self::Error> {
        let Some(hydrated) = reconcile_inner(value)? else {
            return Ok(());
        };
        if index < self.items.len() {
            self.items[index] = hydrated;
        }
        Ok(())
    }

    fn delete(&mut self, index: usize) -> Result<(), Self::Error> {
        if index < self.items.len() {
            self.items.remove(index);
        }
        Ok(())
    }

    fn len(&self) -> Result<usize, Self::Error> {
        Ok(self.items.len())
    }
}

// Iterator adapter that converts list items into `ValueRef`.
struct HydrateSeqItemIter<'a> {
    inner: std::slice::Iter<'a, am::hydrate::Value>,
}

impl<'a> Iterator for HydrateSeqItemIter<'a> {
    type Item = am::ValueRef<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|v| hydrate_value_to_value_ref(v))
    }
}

// ---------------------------------------------------------------------------
// HydrateTextReconciler
//
// Accumulates the text content in a `String` and writes it as
// `am::hydrate::Text` on drop.
// ---------------------------------------------------------------------------

struct HydrateTextReconciler<'a> {
    text: String,
    target: &'a mut Option<am::hydrate::Value>,
}

impl Drop for HydrateTextReconciler<'_> {
    fn drop(&mut self) {
        let text = std::mem::take(&mut self.text);
        *self.target = Some(am::hydrate::Value::Text(am::hydrate::Text::new(
            am::TextEncoding::platform_default(),
            &text,
        )));
    }
}

impl TextReconciler for HydrateTextReconciler<'_> {
    type Error = HydrateReconcileError;

    fn splice<S: AsRef<str>>(
        &mut self,
        pos: usize,
        delete: isize,
        insert: S,
    ) -> Result<(), Self::Error> {
        let delete_count = delete.max(0) as usize;
        let byte_start = self.char_to_byte_offset(pos);
        let byte_end = self.char_to_byte_offset(pos + delete_count);
        self.text
            .replace_range(byte_start..byte_end, insert.as_ref());
        Ok(())
    }

    fn heads(&self) -> &[am::ChangeHash] {
        &[]
    }

    fn update<S: AsRef<str>>(&mut self, new_text: S) -> Result<(), Self::Error> {
        self.text = new_text.as_ref().to_string();
        Ok(())
    }
}

impl HydrateTextReconciler<'_> {
    /// Convert a char-based index into a byte offset suitable for
    /// `String::replace_range`.
    fn char_to_byte_offset(&self, char_index: usize) -> usize {
        self.text
            .char_indices()
            .nth(char_index)
            .map(|(byte_pos, _)| byte_pos)
            .unwrap_or(self.text.len())
    }
}

// ---------------------------------------------------------------------------
// HydrateCounterReconciler
// ---------------------------------------------------------------------------

struct HydrateCounterReconciler<'a> {
    value: i64,
    target: &'a mut Option<am::hydrate::Value>,
}

impl Drop for HydrateCounterReconciler<'_> {
    fn drop(&mut self) {
        *self.target = Some(am::hydrate::Value::Scalar(am::ScalarValue::Counter(
            self.value.into(),
        )));
    }
}

impl CounterReconciler for HydrateCounterReconciler<'_> {
    type Error = HydrateReconcileError;

    fn increment(&mut self, by: i64) -> Result<(), Self::Error> {
        self.value += by;
        Ok(())
    }

    fn set(&mut self, value: i64) -> Result<(), Self::Error> {
        self.value = value;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reconcile::{NoKey, Reconciler};

    #[test]
    fn scalar_string() {
        let val = reconcile_to_am_hydrate(String::from("hello")).unwrap();
        assert_eq!(
            val,
            am::hydrate::Value::Scalar(am::ScalarValue::Str("hello".into()))
        );
    }

    #[test]
    fn scalar_u64() {
        let val = reconcile_to_am_hydrate(42u64).unwrap();
        assert_eq!(val, am::hydrate::Value::Scalar(am::ScalarValue::Uint(42)));
    }

    #[test]
    fn scalar_i64() {
        let val = reconcile_to_am_hydrate(-7i64).unwrap();
        assert_eq!(val, am::hydrate::Value::Scalar(am::ScalarValue::Int(-7)));
    }

    #[test]
    fn scalar_bool() {
        let val = reconcile_to_am_hydrate(true).unwrap();
        assert_eq!(
            val,
            am::hydrate::Value::Scalar(am::ScalarValue::Boolean(true))
        );
    }

    #[test]
    fn scalar_f64() {
        let val = reconcile_to_am_hydrate(3.456).unwrap();
        assert_eq!(val, am::hydrate::Value::Scalar(am::ScalarValue::F64(3.456)));
    }

    #[test]
    fn option_none() {
        let val = reconcile_to_am_hydrate(None::<String>).unwrap();
        assert_eq!(val, am::hydrate::Value::Scalar(am::ScalarValue::Null));
    }

    #[test]
    fn option_some() {
        let val = reconcile_to_am_hydrate(Some(String::from("hi"))).unwrap();
        assert_eq!(
            val,
            am::hydrate::Value::Scalar(am::ScalarValue::Str("hi".into()))
        );
    }

    #[test]
    fn simple_map() {
        struct Pair {
            key: String,
            value: i64,
        }

        impl Reconcile for Pair {
            type Key<'a> = NoKey;

            fn reconcile<R: Reconciler>(&self, mut reconciler: R) -> Result<(), R::Error> {
                let mut map = reconciler.map()?;
                map.put("key", &self.key)?;
                map.put("value", self.value)?;
                Ok(())
            }
        }

        let val = reconcile_to_am_hydrate(Pair {
            key: "abc".into(),
            value: 99,
        }).unwrap();

        match &val {
            am::hydrate::Value::Map(m) => {
                assert_eq!(
                    *m.get("key").unwrap(),
                    am::hydrate::Value::Scalar(am::ScalarValue::Str("abc".into()))
                );
                assert_eq!(
                    *m.get("value").unwrap(),
                    am::hydrate::Value::Scalar(am::ScalarValue::Int(99))
                );
            }
            other => panic!("expected Map, got {:?}", other),
        }
    }

    #[test]
    fn simple_list() {
        let items: Vec<i64> = vec![1, 2, 3];
        let val = reconcile_to_am_hydrate(&items).unwrap();

        match &val {
            am::hydrate::Value::List(l) => {
                let values: Vec<_> = l.iter().map(|lv| &lv.value).collect();
                assert_eq!(values.len(), 3);
                assert_eq!(
                    *values[0],
                    am::hydrate::Value::Scalar(am::ScalarValue::Int(1))
                );
                assert_eq!(
                    *values[1],
                    am::hydrate::Value::Scalar(am::ScalarValue::Int(2))
                );
                assert_eq!(
                    *values[2],
                    am::hydrate::Value::Scalar(am::ScalarValue::Int(3))
                );
            }
            other => panic!("expected List, got {:?}", other),
        }
    }

    #[test]
    fn nested_map_with_list() {
        struct Outer {
            name: String,
            numbers: Vec<u64>,
        }

        impl Reconcile for Outer {
            type Key<'a> = NoKey;

            fn reconcile<R: Reconciler>(&self, mut reconciler: R) -> Result<(), R::Error> {
                let mut map = reconciler.map()?;
                map.put("name", &self.name)?;
                map.put("numbers", &self.numbers)?;
                Ok(())
            }
        }

        let val = reconcile_to_am_hydrate(Outer {
            name: "test".into(),
            numbers: vec![10, 20],
        }).unwrap();

        match &val {
            am::hydrate::Value::Map(m) => {
                assert_eq!(
                    *m.get("name").unwrap(),
                    am::hydrate::Value::Scalar(am::ScalarValue::Str("test".into()))
                );
                match m.get("numbers").unwrap() {
                    am::hydrate::Value::List(l) => {
                        assert_eq!(l.len(), 2);
                        let values: Vec<_> = l.iter().map(|lv| &lv.value).collect();
                        assert_eq!(
                            *values[0],
                            am::hydrate::Value::Scalar(am::ScalarValue::Uint(10))
                        );
                        assert_eq!(
                            *values[1],
                            am::hydrate::Value::Scalar(am::ScalarValue::Uint(20))
                        );
                    }
                    other => panic!("expected List for numbers, got {:?}", other),
                }
            }
            other => panic!("expected Map, got {:?}", other),
        }
    }

    #[test]
    fn empty_list() {
        let items: Vec<String> = vec![];
        let val = reconcile_to_am_hydrate(&items).unwrap();
        match &val {
            am::hydrate::Value::List(l) => assert_eq!(l.len(), 0),
            other => panic!("expected empty List, got {:?}", other),
        }
    }

    #[test]
    fn list_of_maps() {
        #[derive(Clone)]
        struct Item {
            label: String,
        }

        impl Reconcile for Item {
            type Key<'a> = NoKey;

            fn reconcile<R: Reconciler>(&self, mut reconciler: R) -> Result<(), R::Error> {
                let mut map = reconciler.map()?;
                map.put("label", &self.label)?;
                Ok(())
            }
        }

        let items = vec![Item { label: "a".into() }, Item { label: "b".into() }];
        let val = reconcile_to_am_hydrate(&items).unwrap();

        match &val {
            am::hydrate::Value::List(l) => {
                assert_eq!(l.len(), 2);
                for (i, lv) in l.iter().enumerate() {
                    match &lv.value {
                        am::hydrate::Value::Map(m) => {
                            let expected = if i == 0 { "a" } else { "b" };
                            assert_eq!(
                                *m.get("label").unwrap(),
                                am::hydrate::Value::Scalar(am::ScalarValue::Str(expected.into()))
                            );
                        }
                        other => panic!("expected Map at index {}, got {:?}", i, other),
                    }
                }
            }
            other => panic!("expected List, got {:?}", other),
        }
    }
}
