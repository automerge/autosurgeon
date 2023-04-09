use std::ops::RangeFull;

use automerge::ScalarValue;

use crate::{Doc, Prop, ReadDoc};

mod impls;
mod map;
mod seq;

/// A node in the document we are reconciling with.
///
/// The methods on reconciler modify the underlying document if the document does not match the
/// structure implied by the method. For example, calling `Reconciler::boolean` will first check if
/// the current value is a [`automerge::ScalarValue::Boolean`] and whether the value of the boolean
/// is the same as the value passed to `Reconciler::boolean`, if either of these checks fails then
/// the document will updated to the new value, otherwise nothing will be done.
///
/// Methods which create composite structures (such as a map or sequence) will return an
/// implementation of a trait representing that structure after first updating the document to
/// match the implied structure.
pub trait Reconciler {
    type Error: std::error::Error + From<StaleHeads>;

    /// The type returned from [`Self::map`]
    type Map<'a>: MapReconciler<Error = Self::Error>
    where
        Self: 'a;

    /// The type returned from [`Self::seq`]
    type Seq<'a>: SeqReconciler<Error = Self::Error>
    where
        Self: 'a;

    /// The type returned from [`Self::text`]
    type Text<'a>: TextReconciler<Error = Self::Error>
    where
        Self: 'a;

    /// The type returned from [`Self::counter`]
    type Counter<'a>: CounterReconciler<Error = Self::Error>
    where
        Self: 'a;

    /// Set the current node to a [`automerge::ScalarValue::Null`]
    fn none(&mut self) -> Result<(), Self::Error>;

    /// Set the current node to a [`automerge::ScalarValue::Bytes`]
    fn bytes<B: AsRef<[u8]>>(&mut self, value: B) -> Result<(), Self::Error>;

    /// Set the current node to a [`automerge::ScalarValue::Timestamp`]
    fn timestamp(&mut self, value: i64) -> Result<(), Self::Error>;

    /// Set the current node to a [`automerge::ScalarValue::Boolean`]
    fn boolean(&mut self, value: bool) -> Result<(), Self::Error>;

    /// Set the current node to a [`automerge::ScalarValue::Str`]
    fn str<S: AsRef<str>>(&mut self, value: S) -> Result<(), Self::Error>;

    /// Set the current node to a [`automerge::ScalarValue::Uint`]
    fn u64(&mut self, value: u64) -> Result<(), Self::Error>;

    /// Set the current node to a [`automerge::ScalarValue::Int`]
    fn i64(&mut self, value: i64) -> Result<(), Self::Error>;

    /// Set the current node to a [`automerge::ScalarValue::F64`]
    fn f64(&mut self, value: f64) -> Result<(), Self::Error>;

    /// Set the current node to a [`automerge::ObjType::Map`]
    ///
    /// The [`MapReconciler`] which is returned can be used to reconcile the state of the resulting
    /// map
    fn map(&mut self) -> Result<Self::Map<'_>, Self::Error>;

    /// Set the current node to a [`automerge::ObjType::List`]
    ///
    /// The [`SeqReconciler`] which is returned can be used to reconcile the state of the resulting
    /// sequence
    fn seq(&mut self) -> Result<Self::Seq<'_>, Self::Error>;

    /// Set the current node to a [`automerge::ObjType::Text`]
    ///
    /// The [`TextReconciler`] which is returned can be used to reconcile the state of the resulting
    /// text
    fn text(&mut self) -> Result<Self::Text<'_>, Self::Error>;

    /// Set the current node to a [`automerge::ScalarValue::Counter`]
    ///
    /// The [`CounterReconciler`] which is returned can be used to reconcile the state of the resulting
    /// text
    fn counter(&mut self) -> Result<Self::Counter<'_>, Self::Error>;

    /// Get the heads of the document this reconciler is pointing at
    fn heads(&self) -> &[automerge::ChangeHash];
}

/// A node in the document which is a map.
///
/// A `MapReconciler` is obtained from [`Reconciler::map`] so once you have a `MapReconciler` it is
/// pointing at an already existing map in the underlying document or transaction.
pub trait MapReconciler {
    type Error: std::error::Error + From<StaleHeads>;
    type EntriesIter<'a>: Iterator<Item = (&'a str, automerge::Value<'a>)>
    where
        Self: 'a;

    /// An iterator over the entries of the map as `(&str, automerge::Value)`
    fn entries(&self) -> Self::EntriesIter<'_>;

    /// Get the value of a single entry in the document, if it exists
    fn entry<P: AsRef<str>>(&self, prop: P) -> Option<automerge::Value<'_>>;

    /// Set a key in the map to a given value.
    fn put<R: Reconcile, P: AsRef<str>>(&mut self, prop: P, value: R) -> Result<(), Self::Error>;

    /// Delete a key in the map
    fn delete<P: AsRef<str>>(&mut self, prop: P) -> Result<(), Self::Error>;

    /// For some `R`, hydrate the key at the given property
    ///
    /// This is used to determine if some data in the document matches the data we are reconciling.
    /// Suppose we have some type `R: Reconcile`, we pass `R` as a type parameter to
    /// `hydrate_entry_key` and we will get back a `LoadKey<R::Key>`, we can then compare this to
    /// the key of the data we are reconciling using `R::key` to determine if this entry represents
    /// the same identity as the item we are reconciling.
    fn hydrate_entry_key<'a, R: Reconcile, P: AsRef<str>>(
        &self,
        prop: P,
    ) -> Result<LoadKey<R::Key<'a>>, Self::Error>;

    /// First delete, then put to a key in the map
    fn replace<R: Reconcile, P: AsRef<str>>(
        &mut self,
        prop: P,
        value: R,
    ) -> Result<(), Self::Error> {
        self.delete(&prop)?;
        self.put(prop, value)?;
        Ok(())
    }

    /// Remove any entries that do not satisfy the given predicate.
    fn retain<F: FnMut(&str, automerge::Value) -> bool>(
        &mut self,
        mut pred: F,
    ) -> Result<(), Self::Error> {
        // TODO: a more efficient implementation might be possible with
        // an addition to Automerge
        let mut delenda = Vec::new();
        for (k, v) in self.entries() {
            if !pred(k, v) {
                delenda.push(k.to_string());
            }
        }
        for k in &delenda {
            self.delete(k)?;
        }
        Ok(())
    }
}

/// A node in the document which is an `automerge::List`
pub trait SeqReconciler {
    type Error: std::error::Error + From<StaleHeads>;
    type ItemIter<'a>: Iterator<Item = automerge::Value<'a>>
    where
        Self: 'a;

    /// An iterator over the items currently in this node in the document
    fn items(&self) -> Self::ItemIter<'_>;

    /// Get a single item from the document
    fn get(&self, index: usize) -> Result<Option<automerge::Value<'_>>, Self::Error>;

    /// For some `R`, hydrate the key at the given index
    ///
    /// This is used to determine if some data in the document matches the data we are reconciling.
    /// Suppose we have some type `R: Reconcile`, we pass `R` as a type parameter to
    /// `hydrate_item_key` and we will get back a `LoadKey<R::Key>`, we can then compare this to
    /// the key of the data we are reconciling using `R::key` to determine if this index represents
    /// the same identity as the item we are reconciling.
    fn hydrate_item_key<'a, R: Reconcile>(
        &self,
        index: usize,
    ) -> Result<LoadKey<R::Key<'a>>, Self::Error>;

    /// Insert the given value at the given index in the document
    fn insert<R: Reconcile>(&mut self, index: usize, value: R) -> Result<(), Self::Error>;

    /// Reconcile the value of an index with some `R`
    fn set<R: Reconcile>(&mut self, index: usize, value: R) -> Result<(), Self::Error>;

    /// Delete an index from the sequence
    fn delete(&mut self, index: usize) -> Result<(), Self::Error>;

    /// Get the current length of the sequence
    fn len(&self) -> Result<usize, Self::Error>;

    fn is_empty(&self) -> Result<bool, Self::Error> {
        Ok(self.len()? == 0)
    }
}

/// A node in the document which is an `automerge::ScalarValue::Counter`
pub trait CounterReconciler {
    type Error: std::error::Error + From<StaleHeads>;

    fn increment(&mut self, by: i64) -> Result<(), Self::Error>;
    fn set(&mut self, value: i64) -> Result<(), Self::Error>;
}

/// A node in the document which is an `automerge::ObjType::Text`
pub trait TextReconciler {
    type Error: std::error::Error + From<StaleHeads>;
    fn splice<S: AsRef<str>>(
        &mut self,
        pos: usize,
        delete: usize,
        insert: S,
    ) -> Result<(), Self::Error>;
    fn heads(&self) -> &[automerge::ChangeHash];
}

/// Placeholder type to be used for types which do not have a key
#[derive(Clone, PartialEq, Eq)]
pub struct NoKey;

/// The result of either loading a key from the document or from a `R: Reconcile`
#[derive(Debug)]
pub enum LoadKey<K> {
    /// This data type does not have a key
    NoKey,
    /// This data type has a key but we couldn't load it
    KeyNotFound,
    /// We loaded the key
    Found(K),
}

impl<K> LoadKey<K> {
    /// If this is a `LoadKey::Found`, map `f` over the contents
    pub fn map<L, F: FnOnce(K) -> L>(self, f: F) -> LoadKey<L> {
        match self {
            Self::NoKey => LoadKey::NoKey,
            Self::KeyNotFound => LoadKey::KeyNotFound,
            Self::Found(k) => LoadKey::Found(f(k)),
        }
    }
}

/// A data type which can be reconciled
///
/// The required method is `reconcile`. This allows you to update the state of a document based on
/// the state of the implementor.
///
/// As well as the `reconcile` method, `Reconcile` also allows you to specify the "key" of the
/// type. This is used by autosurgeon to determine how to merge with existing data in the document.
/// Imagine that you have a list of items like this:
///
/// ```json
/// [{id: 1, name: "one"}, {id: 2, name: "two"}]
/// ```
///
/// The difference between this list and the following
///
/// ```json
/// [{id: 3, name: "three"}, {id: 1, name: "one"}, {id: 2, name: "two"}]
/// ```
///
/// Is (to us) clearly that an item has been inserted at the front. `autosurgeon` has no way to
/// know this though. By defining a "key" which points at the `id` field we tell `autosurgeon` that
/// if it finds an item in the document with the same ID as `self`, then we should update that
/// item, otherwise we should insert a new item.
///
/// # Example
///
/// ```rust
/// use std::borrow::Cow;
/// # use autosurgeon::{Reconcile, Reconciler, hydrate_key, ReconcileError, Prop, ReadDoc, reconcile::{LoadKey, MapReconciler}};
///
/// struct User {
///     id: String,
///     name: String
/// }
///
/// impl Reconcile for User {
///     type Key<'a> = Cow<'a, String>;
///
///     fn reconcile<R: Reconciler>(&self, mut reconciler: R) -> Result<(), R::Error> {
///         let mut m = reconciler.map()?;
///         m.put("id", &self.id)?;
///         m.put("name", &self.name)?;
///         Ok(())
///     }
///
///     fn hydrate_key<'a, D: ReadDoc>(
///         doc: &D,
///         obj: &automerge::ObjId,
///         prop: Prop<'_>,
///     ) -> Result<LoadKey<Self::Key<'a>>, ReconcileError> {
///         hydrate_key(doc, obj, prop, "id".into())
///     }
///
///     fn key(&self) -> LoadKey<Cow<'_, String>> {
///         LoadKey::Found(Cow::Borrowed(&self.id))
///     }
/// }
/// ```
pub trait Reconcile {
    /// The type of the key of this item.
    ///
    /// If you do not need to set a key for this type you can use [`NoKey`] as the type.
    ///
    /// If you want to return reference types from [`Reconcile::key`] then you should use
    /// `Cow<'a, T>` as the type. For example if you have an `id: String` field then this type
    /// should be `Cow<'a, String>`.
    type Key<'a>: PartialEq;

    /// Reconcile this item with the document
    ///
    /// See the documentation of [`Reconciler`] for more details. Typically though there are two
    /// cases:
    ///
    /// 1. `R` reconciles to a primitive value, in which case you directly call one of the
    ///    primitive value methods on [`Reconciler`] (e.g. [`Reconciler::str`]
    /// 2. `R` reconciles to a composite data structure - either a map, list, counter, or text in
    ///    which case you obtain the nested reconciler using [`Reconciler::map`],
    ///    [`Reconciler::seq`], [`Reconciler::counter`] or [`Reconciler::text`] respectively and
    ///    then proceed with reconciliation using then nested reconciler
    fn reconcile<R: Reconciler>(&self, reconciler: R) -> Result<(), R::Error>;

    /// Hydrate the key for this Object
    ///
    /// This will be called by the reconciliation infrastructure to determine if some data in the
    /// document matches an instance of `Self`. The `obj` and `prop` arguments will be the
    /// arguments corresponding the item that _contains_ `Self`. Consider the example from the
    /// trait level documentation but now imagine the `User` structs are within a `"users"`
    /// property in the document. An example document might look like:
    ///
    /// ```json
    /// {
    ///     "users": [
    ///         {id: "one", name: "userone"},
    ///         {id: "two", name: "usertwo"},
    ///     ]
    /// }
    /// ```
    ///
    /// Recall that the definition of `hydrate_key` was this:
    ///
    /// ```rust
    /// # use std::borrow::Cow;
    /// # use autosurgeon::{
    /// #   Reconcile, Reconciler, reconcile::{MapReconciler, LoadKey}, Prop, hydrate_key, ReconcileError,
    /// #   ReadDoc
    /// # };
    /// # struct User {
    /// #     id: String,
    /// #     name: String
    /// # }
    /// # impl Reconcile for User {
    /// #     type Key<'a> = Cow<'a, String>;
    /// #     fn reconcile<R: Reconciler>(&self, mut reconciler: R) -> Result<(), R::Error> {
    /// #         todo!()
    /// #     }
    ///     fn hydrate_key<'a, D: ReadDoc>(
    ///         doc: &D,
    ///         obj: &automerge::ObjId,
    ///         prop: Prop<'_>,
    ///     ) -> Result<LoadKey<Self::Key<'a>>, ReconcileError> {
    ///         hydrate_key(doc, obj, prop, "id".into())
    ///     }
    /// #     fn key(&self) -> LoadKey<Cow<'_, String>> {
    /// #         LoadKey::Found(Cow::Borrowed(&self.id))
    /// #     }
    /// # }
    /// ```
    ///
    /// Here the value of `obj` and `doc` passed to `hydrate_key` will be the ID of the `"users"`
    /// list and `idx` respectively, where `idx` is the index of the user in the `"users"` array.
    fn hydrate_key<'a, D: ReadDoc>(
        #[allow(unused_variables)] doc: &D,
        #[allow(unused_variables)] obj: &automerge::ObjId,
        #[allow(unused_variables)] prop: Prop<'_>,
    ) -> Result<LoadKey<Self::Key<'a>>, ReconcileError> {
        Ok(LoadKey::NoKey)
    }

    /// Get the key from an instance of Self
    fn key(&self) -> LoadKey<Self::Key<'_>> {
        LoadKey::NoKey
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ReconcileError {
    #[error(transparent)]
    Automerge(#[from] automerge::AutomergeError),
    #[error("the top level object must reconcile to a map")]
    TopLevelNotMap,
    #[error(transparent)]
    StaleHeads(#[from] StaleHeads),
}

#[derive(Debug, thiserror::Error)]
#[error("the data to be reconciled is stale, expected heads were {expected:?} but found {found:?}")]
pub struct StaleHeads {
    pub expected: Vec<automerge::ChangeHash>,
    pub found: Vec<automerge::ChangeHash>,
}

struct RootReconciler<'a, D> {
    heads: Vec<automerge::ChangeHash>,
    doc: &'a mut D,
}

impl<'a, D: Doc> Reconciler for RootReconciler<'a, D> {
    type Error = ReconcileError;
    type Map<'b> = InMap<'b, D>
        where Self: 'b;
    type Seq<'b> = InSeq<'b, D>
        where Self: 'b;
    type Text<'b> = InText<'b, D>
        where Self: 'b;
    type Counter<'b> = AtCounter<'b, D>
        where Self: 'b;

    fn none(&mut self) -> Result<(), Self::Error> {
        Err(ReconcileError::TopLevelNotMap)
    }

    fn bytes<B: AsRef<[u8]>>(&mut self, _value: B) -> Result<(), Self::Error> {
        Err(ReconcileError::TopLevelNotMap)
    }

    fn boolean(&mut self, _value: bool) -> Result<(), Self::Error> {
        Err(ReconcileError::TopLevelNotMap)
    }

    fn timestamp(&mut self, _value: i64) -> Result<(), Self::Error> {
        Err(ReconcileError::TopLevelNotMap)
    }

    fn str<S: AsRef<str>>(&mut self, _value: S) -> Result<(), Self::Error> {
        Err(ReconcileError::TopLevelNotMap)
    }

    fn u64(&mut self, _value: u64) -> Result<(), Self::Error> {
        Err(ReconcileError::TopLevelNotMap)
    }

    fn i64(&mut self, _value: i64) -> Result<(), Self::Error> {
        Err(ReconcileError::TopLevelNotMap)
    }

    fn f64(&mut self, _value: f64) -> Result<(), Self::Error> {
        Err(ReconcileError::TopLevelNotMap)
    }

    fn map(&mut self) -> Result<InMap<'_, D>, Self::Error> {
        Ok(InMap {
            heads: &self.heads,
            current_obj: automerge::ROOT,
            doc: self.doc,
        })
    }

    fn seq(&mut self) -> Result<InSeq<'_, D>, Self::Error> {
        Err(ReconcileError::TopLevelNotMap)
    }

    fn text(&mut self) -> Result<Self::Text<'_>, Self::Error> {
        Err(ReconcileError::TopLevelNotMap)
    }

    fn counter(&mut self) -> Result<Self::Counter<'a>, Self::Error> {
        Err(ReconcileError::TopLevelNotMap)
    }

    fn heads(&self) -> &[automerge::ChangeHash] {
        &self.heads
    }
}

enum PropAction<'a> {
    Put(Prop<'a>),
    Insert(u32),
}

impl<'a> PropAction<'a> {
    fn get_target<'b, D: Doc>(
        &self,
        doc: &'b D,
        obj: &automerge::ObjId,
    ) -> Result<Option<(automerge::Value<'b>, automerge::ObjId)>, automerge::AutomergeError> {
        match self {
            Self::Put(prop) => doc.get(obj, prop),
            Self::Insert(_idx) => Ok(None),
        }
    }

    fn create_target_obj<D: Doc>(
        &self,
        doc: &mut D,
        obj: &automerge::ObjId,
        objtype: automerge::ObjType,
    ) -> Result<automerge::ObjId, automerge::AutomergeError> {
        match self {
            Self::Put(prop) => doc.put_object(obj, prop, objtype),
            Self::Insert(idx) => doc.insert_object(obj, (*idx) as usize, objtype),
        }
    }

    fn create_primitive<V: Into<automerge::ScalarValue>, D: Doc>(
        &self,
        doc: &mut D,
        obj: &automerge::ObjId,
        value: V,
    ) -> Result<(), automerge::AutomergeError> {
        match self {
            Self::Put(p) => doc.put(obj, p, value),
            Self::Insert(idx) => doc.insert(obj, (*idx) as usize, value),
        }
    }
}

struct PropReconciler<'a, D> {
    heads: &'a [automerge::ChangeHash],
    doc: &'a mut D,
    current_obj: automerge::ObjId,
    action: PropAction<'a>,
}

impl<'a, D: Doc> Reconciler for PropReconciler<'a, D> {
    type Error = ReconcileError;
    type Map<'b> = InMap<'b, D>
        where Self: 'b;
    type Seq<'b> = InSeq<'b, D>
        where Self: 'b;
    type Text<'b> = InText<'b, D>
        where Self: 'b;
    type Counter<'b> = AtCounter<'b, D>
        where Self: 'b;

    fn none(&mut self) -> Result<(), Self::Error> {
        self.action
            .create_primitive(self.doc, &self.current_obj, ScalarValue::Null)
            .map_err(ReconcileError::from)
    }

    fn bytes<B: AsRef<[u8]>>(&mut self, value: B) -> Result<(), Self::Error> {
        self.action
            .create_primitive(self.doc, &self.current_obj, value.as_ref().to_vec())
            .map_err(ReconcileError::from)
    }

    fn boolean(&mut self, value: bool) -> Result<(), Self::Error> {
        self.action
            .create_primitive(self.doc, &self.current_obj, value)
            .map_err(ReconcileError::from)
    }

    fn timestamp(&mut self, value: i64) -> Result<(), Self::Error> {
        self.action
            .create_primitive(self.doc, &self.current_obj, ScalarValue::Timestamp(value))
            .map_err(ReconcileError::from)
    }

    fn str<S: AsRef<str>>(&mut self, value: S) -> Result<(), Self::Error> {
        self.action
            .create_primitive(self.doc, &self.current_obj, value.as_ref())
            .map_err(ReconcileError::from)
    }

    fn u64(&mut self, value: u64) -> Result<(), Self::Error> {
        self.action
            .create_primitive(self.doc, &self.current_obj, value)
            .map_err(ReconcileError::from)
    }

    fn i64(&mut self, value: i64) -> Result<(), Self::Error> {
        self.action
            .create_primitive(self.doc, &self.current_obj, value)
            .map_err(ReconcileError::from)
    }

    fn f64(&mut self, value: f64) -> Result<(), Self::Error> {
        self.action
            .create_primitive(self.doc, &self.current_obj, value)
            .map_err(ReconcileError::from)
    }

    fn map(&mut self) -> Result<InMap<'_, D>, Self::Error> {
        use automerge::{ObjType, Value};
        let map_id = if let Some((Value::Object(ObjType::Map), id)) =
            self.action.get_target(self.doc, &self.current_obj)?
        {
            id
        } else {
            self.action
                .create_target_obj(self.doc, &self.current_obj, ObjType::Map)?
        };
        Ok(InMap {
            heads: self.heads,
            current_obj: map_id,
            doc: self.doc,
        })
    }

    fn seq(&mut self) -> Result<InSeq<'_, D>, Self::Error> {
        use automerge::{ObjType, Value};
        let seq_id = if let Some((Value::Object(ObjType::List), id)) =
            self.action.get_target(self.doc, &self.current_obj)?
        {
            id
        } else {
            self.action
                .create_target_obj(self.doc, &self.current_obj, ObjType::List)?
        };
        Ok(InSeq {
            heads: self.heads,
            obj: seq_id,
            doc: self.doc,
        })
    }

    fn text(&mut self) -> Result<Self::Text<'_>, Self::Error> {
        use automerge::{ObjType, Value};
        let text_id = if let Some((Value::Object(ObjType::Text), id)) =
            self.action.get_target(self.doc, &self.current_obj)?
        {
            id
        } else {
            self.action
                .create_target_obj(self.doc, &self.current_obj, ObjType::Text)?
        };
        Ok(InText {
            heads: self.heads,
            obj: text_id,
            doc: self.doc,
        })
    }

    fn counter(&mut self) -> Result<Self::Counter<'_>, Self::Error> {
        Ok(AtCounter {
            doc: self.doc,
            current_obj: &self.current_obj,
            action: &self.action,
        })
    }

    fn heads(&self) -> &[automerge::ChangeHash] {
        self.heads
    }
}

struct AtCounter<'a, D> {
    doc: &'a mut D,
    current_obj: &'a automerge::ObjId,
    action: &'a PropAction<'a>,
}

impl<'a, D: Doc> CounterReconciler for AtCounter<'a, D> {
    type Error = ReconcileError;

    fn increment(&mut self, by: i64) -> Result<(), Self::Error> {
        use automerge::Value;
        match &self.action {
            PropAction::Put(prop) => {
                if let Some((Value::Scalar(s), _)) = self.doc.get(self.current_obj, prop)? {
                    if let ScalarValue::Counter(_) = s.as_ref() {
                        self.doc.increment(self.current_obj, prop, by)?;
                        return Ok(());
                    }
                }
                self.doc
                    .put(self.current_obj, prop, ScalarValue::Counter(by.into()))?;
                Ok(())
            }
            PropAction::Insert(idx) => {
                self.doc.insert(
                    self.current_obj,
                    (*idx) as usize,
                    ScalarValue::Counter(by.into()),
                )?;
                Ok(())
            }
        }
    }

    fn set(&mut self, value: i64) -> Result<(), Self::Error> {
        self.action.create_primitive(
            self.doc,
            self.current_obj,
            automerge::ScalarValue::Counter(value.into()),
        )?;
        Ok(())
    }
}

struct InMap<'a, D> {
    heads: &'a [automerge::ChangeHash],
    doc: &'a mut D,
    current_obj: automerge::ObjId,
}

impl<'a, D: Doc> MapReconciler for InMap<'a, D> {
    type Error = ReconcileError;
    type EntriesIter<'b> = InMapEntries<'b>
        where Self: 'b;

    fn entries(&self) -> Self::EntriesIter<'_> {
        InMapEntries {
            map_range: self.doc.map_range(&self.current_obj, ..),
        }
    }

    fn entry<P: AsRef<str>>(&self, prop: P) -> Option<automerge::Value<'_>> {
        self.doc
            .get(&self.current_obj, prop.as_ref())
            .ok()
            .flatten()
            .map(|v| v.0)
    }

    fn put<R: Reconcile, P: AsRef<str>>(&mut self, prop: P, value: R) -> Result<(), Self::Error> {
        let reconciler = PropReconciler {
            heads: self.heads,
            current_obj: self.current_obj.clone(),
            doc: self.doc,
            action: PropAction::Put(prop.as_ref().into()),
        };
        value.reconcile(reconciler)?;
        Ok(())
    }

    fn delete<P: AsRef<str>>(&mut self, prop: P) -> Result<(), Self::Error> {
        self.doc
            .delete(&self.current_obj, prop.as_ref())
            .map_err(ReconcileError::from)
    }

    fn hydrate_entry_key<'b, R: Reconcile, P: AsRef<str>>(
        &self,
        prop: P,
    ) -> Result<LoadKey<R::Key<'b>>, Self::Error> {
        R::hydrate_key(self.doc, &self.current_obj, prop.as_ref().into())
    }
}

struct InMapEntries<'a> {
    map_range: automerge::MapRange<'a, RangeFull>,
}

impl<'a> Iterator for InMapEntries<'a> {
    type Item = (&'a str, automerge::Value<'a>);

    fn next(&mut self) -> Option<Self::Item> {
        self.map_range.next().map(|(key, val, _)| (key, val))
    }
}

struct InSeq<'a, D> {
    heads: &'a [automerge::ChangeHash],
    doc: &'a mut D,
    obj: automerge::ObjId,
}

struct ItemsInSeq<'a> {
    list_range: automerge::ListRange<'a, RangeFull>,
}

impl<'a> Iterator for ItemsInSeq<'a> {
    type Item = automerge::Value<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.list_range.next().map(|i| i.1)
    }
}

impl<'a, D: Doc> SeqReconciler for InSeq<'a, D> {
    type Error = ReconcileError;
    type ItemIter<'b> = ItemsInSeq<'b>
        where Self: 'b;

    fn items<'b>(&'_ self) -> Self::ItemIter<'_> {
        ItemsInSeq {
            list_range: self.doc.list_range(&self.obj, ..),
        }
    }

    fn get(&'_ self, index: usize) -> Result<Option<automerge::Value<'_>>, Self::Error> {
        Ok(self.doc.get(&self.obj, index)?.map(|(v, _)| v))
    }

    fn insert<R: Reconcile>(&mut self, index: usize, value: R) -> Result<(), Self::Error> {
        let reconciler = PropReconciler {
            heads: self.heads,
            doc: self.doc,
            current_obj: self.obj.clone(),
            action: PropAction::Insert(index as u32),
        };
        value.reconcile(reconciler)?;
        Ok(())
    }

    fn set<R: Reconcile>(&mut self, index: usize, value: R) -> Result<(), Self::Error> {
        let reconciler = PropReconciler {
            heads: self.heads,
            doc: self.doc,
            current_obj: self.obj.clone(),
            action: PropAction::Put(index.into()),
        };
        value.reconcile(reconciler)?;
        Ok(())
    }

    fn delete<'b>(&mut self, index: usize) -> Result<(), Self::Error> {
        self.doc
            .delete(&self.obj, index)
            .map_err(ReconcileError::from)
    }

    fn len(&self) -> Result<usize, Self::Error> {
        Ok(self.doc.length(&self.obj))
    }

    fn hydrate_item_key<'b, R: Reconcile>(
        &self,
        index: usize,
    ) -> Result<LoadKey<R::Key<'b>>, Self::Error> {
        if self.doc.get(&self.obj, index)?.is_some() {
            R::hydrate_key(self.doc, &self.obj, index.into())
        } else {
            Ok(LoadKey::KeyNotFound)
        }
    }
}

struct InText<'a, D> {
    heads: &'a [automerge::ChangeHash],
    doc: &'a mut D,
    obj: automerge::ObjId,
}

impl<'a, D: Doc> TextReconciler for InText<'a, D> {
    type Error = ReconcileError;

    fn splice<S: AsRef<str>>(
        &mut self,
        pos: usize,
        delete: usize,
        text: S,
    ) -> Result<(), Self::Error> {
        self.doc
            .splice_text(&self.obj, pos, delete, text.as_ref())?;
        Ok(())
    }

    fn heads(&self) -> &[automerge::ChangeHash] {
        self.heads
    }
}

/// Reconcile `value` with `doc`
///
/// This will throw an error if the implementation of `Reconcile` for `R` does anything except call
/// `Reconciler::map` because only a map is a valid object for the root of an `automerge` document.
pub fn reconcile<R: Reconcile, D: Doc>(doc: &mut D, value: R) -> Result<(), ReconcileError> {
    let reconciler = RootReconciler {
        heads: doc.get_heads(),
        doc,
    };
    value.reconcile(reconciler)?;
    Ok(())
}

/// Reconcile `value` with `(obj, prop)` in `doc`
///
/// This is useful when you want to update a particular object within an `automerge` document e.g.
///
/// ```rust
/// # use automerge::{ObjType, transaction::Transactable};
/// # use autosurgeon::reconcile_prop;
/// # use automerge_test::{assert_doc, map, list};
/// let mut doc = automerge::AutoCommit::new();
/// doc.put_object(&automerge::ROOT, "numbers", ObjType::List);
/// reconcile_prop(&mut doc, automerge::ROOT, "numbers", &vec![1,2,3]).unwrap();
///
/// assert_doc!(
///     doc.document(),
///     map! {
///         "numbers" => { list! {
///             {1}, {2}, {3}
///         }}
///     }
/// );
/// ```
pub fn reconcile_prop<'a, D: Doc, R: Reconcile, O: AsRef<automerge::ObjId>, P: Into<Prop<'a>>>(
    doc: &mut D,
    obj: O,
    prop: P,
    value: R,
) -> Result<(), ReconcileError> {
    let heads = doc.get_heads();
    let reconciler = PropReconciler {
        heads: &heads,
        doc,
        action: PropAction::Put(prop.into()),
        current_obj: obj.as_ref().clone(),
    };
    value.reconcile(reconciler)?;
    Ok(())
}

/// Reconcile into a new index in a sequence
///
/// This is useful when you specifically want to insert an object which does not implement
/// `Reconcile::key` into a sequence.
pub fn reconcile_insert<R: Reconcile>(
    doc: &mut automerge::AutoCommit,
    obj: automerge::ObjId,
    idx: usize,
    value: R,
) -> Result<(), ReconcileError> {
    let heads = doc.get_heads();
    let reconciler = PropReconciler {
        heads: &heads,
        doc,
        action: PropAction::Insert(idx as u32),
        current_obj: obj,
    };
    value.reconcile(reconciler)?;
    Ok(())
}

/// Hydrate the key `inner` from inside the object `outer`
///
/// This is useful when you are attempting to hydrate the key of an object. Imagine you have a
/// structure like this,
///
/// ```json
/// {
///     "products": [
///         {id: 1, name: "one"},
///         {id: 2, name: "two"},
///     ]
/// }
/// ```
///
/// Say we define a type `Product` for the elements of the `products` list, this type will need to
/// implement [`Reconcile::hydrate_key`] such that it returns the `id` field value. However, the
/// `obj`, and `prop` arguments passed to [`Reconcile::hydrate_key`] will point at the overall
/// product map. `hydrate_key` takes an additional `inner` property which should be the property of
/// the key being hydrated from within `prop`. In the above example when hydrating a product the
/// `obj` and `prop` passed to [`Reconcile::hydrate_key`] would be the ID of the "products" list
/// and the index of the product in question. To hydrate the key of the product then you would pass
/// the object ID of the "products" list as `obj`, the index of the product as `outer`, and the
/// "id" key as `inner`.
pub fn hydrate_key<'a, D: ReadDoc, H: crate::Hydrate + Clone>(
    doc: &D,
    obj: &automerge::ObjId,
    outer: Prop<'a>,
    inner: Prop<'a>,
) -> Result<LoadKey<H>, ReconcileError> {
    use crate::hydrate::HydrateResultExt;
    Ok(
        crate::hydrate::hydrate_path(doc, obj, vec![outer, inner].into_iter())
            .strip_unexpected()?
            .map(LoadKey::Found)
            .unwrap_or(LoadKey::KeyNotFound),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use automerge_test::{assert_doc, list, map};

    struct Contact {
        name: String,
        addresses: Vec<Address>,
        id: u64,
    }

    struct Address {
        line_one: String,
        line_two: String,
    }

    impl Reconcile for Address {
        type Key<'a> = NoKey;
        fn reconcile<R: Reconciler>(&self, mut reconciler: R) -> Result<(), R::Error> {
            let mut map = reconciler.map()?;
            map.put("line_one", &self.line_one)?;
            map.put("line_two", &self.line_two)?;
            Ok(())
        }
    }

    impl Reconcile for Contact {
        type Key<'a> = NoKey;
        fn reconcile<R: Reconciler>(&self, mut reconciler: R) -> Result<(), R::Error> {
            let mut map = reconciler.map()?;
            map.put("addresses", &self.addresses)?;
            map.put("name", &self.name)?;
            map.put("id", self.id)?;
            Ok(())
        }
    }

    #[test]
    fn basic_reconciliation() {
        let mut bob = Contact {
            name: "bob".to_string(),
            id: 1,
            addresses: vec![Address {
                line_one: "line one".to_string(),
                line_two: "line two".to_string(),
            }],
        };
        let mut doc = automerge::AutoCommit::new();
        reconcile(&mut doc, &bob).unwrap();

        assert_doc!(
            &doc,
            map! {
                "name" => { "bob" },
                "id" => { 1_u64 },
                "addresses" => { list!{
                    { map! {
                        "line_one" => { "line one" },
                        "line_two" => { "line two" },
                   }}
                }
            }}
        );

        let mut doc2 = doc.clone().with_actor("actor2".as_bytes().into());
        bob.name = "Bobsson".to_string();
        reconcile(&mut doc2, &bob).unwrap();

        let mut doc3 = doc.clone().with_actor("actor3".as_bytes().into());
        bob.addresses[0].line_one = "Line one the premier".to_string();
        reconcile(&mut doc3, &bob).unwrap();

        doc.merge(&mut doc2).unwrap();
        doc.merge(&mut doc3).unwrap();

        assert_doc!(
            doc.document(),
            map! {
                "name" => { "Bobsson" },
                "id" => { 1_u64 },
                "addresses" => { list!{
                    { map! {
                        "line_one" => { "Line one the premier" },
                        "line_two" => { "line two" },
                   }}
                }
            }}
        );
    }

    #[test]
    fn test_with_transaction() {
        let mut doc = automerge::Automerge::new();
        let mut tx = doc.transaction();

        let bob = Contact {
            name: "bob".to_string(),
            id: 1,
            addresses: vec![Address {
                line_one: "line one".to_string(),
                line_two: "line two".to_string(),
            }],
        };
        reconcile(&mut tx, &bob).unwrap();
        tx.commit();

        assert_doc!(
            &doc,
            map! {
                "name" => { "bob" },
                "id" => { 1_u64 },
                "addresses" => { list!{
                    { map! {
                        "line_one" => { "line one" },
                        "line_two" => { "line two" },
                   }}
                }
            }}
        );
    }
}
