//! A bunch of tests for the behaviour of reconcile when keys of incoming
//! items match or don't match existing items

use super::*;
use automerge::ReadDoc as AmReadDoc;
use std::borrow::Cow;

/// A type with a key (the `id` field) that reconciles to a map
struct KeyedItem {
    id: String,
    value: String,
}

impl Reconcile for KeyedItem {
    type Key<'a> = Cow<'a, String>;

    fn reconcile<R: Reconciler>(&self, mut reconciler: R) -> Result<(), R::Error> {
        let mut m = reconciler.map()?;
        m.put("id", &self.id)?;
        m.put("value", &self.value)?;
        Ok(())
    }

    fn hydrate_key<'a, D: crate::ReadDoc>(
        doc: &D,
        obj: &automerge::ObjId,
        prop: crate::Prop<'_>,
    ) -> Result<LoadKey<Self::Key<'a>>, ReconcileError> {
        crate::hydrate_key(doc, obj, prop, "id".into())
    }

    fn key(&self) -> LoadKey<Self::Key<'_>> {
        LoadKey::Found(Cow::Borrowed(&self.id))
    }
}

impl crate::Hydrate for KeyedItem {
    fn hydrate_map<D: crate::ReadDoc>(
        doc: &D,
        obj: &automerge::ObjId,
    ) -> Result<Self, crate::HydrateError> {
        let id: String = crate::hydrate_prop(doc, obj, "id")?;
        let value: String = crate::hydrate_prop(doc, obj, "value")?;
        Ok(KeyedItem { id, value })
    }
}

/// A "fresh" item that has no key (returns KeyNotFound)
struct FreshItem {
    id: String,
    value: String,
}

impl Reconcile for FreshItem {
    type Key<'a> = Cow<'a, String>;

    fn reconcile<R: Reconciler>(&self, mut reconciler: R) -> Result<(), R::Error> {
        let mut m = reconciler.map()?;
        m.put("id", &self.id)?;
        m.put("value", &self.value)?;
        Ok(())
    }

    fn hydrate_key<'a, D: crate::ReadDoc>(
        doc: &D,
        obj: &automerge::ObjId,
        prop: crate::Prop<'_>,
    ) -> Result<LoadKey<Self::Key<'a>>, ReconcileError> {
        // Same hydrate_key as KeyedItem - can load keys from doc
        crate::hydrate_key(doc, obj, prop, "id".into())
    }

    fn key(&self) -> LoadKey<Self::Key<'_>> {
        // Always returns KeyNotFound, simulating a "fresh" instance
        LoadKey::KeyNotFound
    }
}

/// Container with a single keyed item in a map property
struct MapContainer<T> {
    item: T,
}

impl<T: Reconcile> Reconcile for MapContainer<T> {
    type Key<'a> = NoKey;

    fn reconcile<R: Reconciler>(&self, mut reconciler: R) -> Result<(), R::Error> {
        let mut m = reconciler.map()?;
        m.put("item", &self.item)?;
        Ok(())
    }
}

/// Container with keyed items in a sequence
struct SeqContainer {
    items: Vec<KeyedItem>,
}

impl Reconcile for SeqContainer {
    type Key<'a> = NoKey;

    fn reconcile<R: Reconciler>(&self, mut reconciler: R) -> Result<(), R::Error> {
        let mut m = reconciler.map()?;
        m.put("items", &self.items)?;
        Ok(())
    }
}

/// Container with fresh items in a sequence
struct FreshSeqContainer {
    items: Vec<FreshItem>,
}

impl Reconcile for FreshSeqContainer {
    type Key<'a> = NoKey;

    fn reconcile<R: Reconciler>(&self, mut reconciler: R) -> Result<(), R::Error> {
        let mut m = reconciler.map()?;
        m.put("items", &self.items)?;
        Ok(())
    }
}

// Test: MapReconciler::put with matching keys should update in place
#[test]
fn map_put_matching_keys_updates_in_place() {
    let mut doc = automerge::AutoCommit::new();
    let container = MapContainer {
        item: KeyedItem {
            id: "item1".to_string(),
            value: "original".to_string(),
        },
    };
    reconcile(&mut doc, &container).unwrap();

    // Fork and make a concurrent change to the value
    let mut doc2 = doc.fork().with_actor("actor2".as_bytes().into());
    let item: KeyedItem = crate::hydrate_prop(&doc2, &automerge::ROOT, "item").unwrap();

    // Update with same id (key matches) on original doc
    let container2 = MapContainer {
        item: KeyedItem {
            id: "item1".to_string(), // Same ID
            value: "updated".to_string(),
        },
    };
    reconcile(&mut doc, &container2).unwrap();

    // On fork, modify the value field directly
    reconcile_prop(
        &mut doc2,
        automerge::ROOT,
        "item",
        &KeyedItem {
            id: item.id,
            value: "concurrent".to_string(),
        },
    )
    .unwrap();

    // Merge - since keys matched, both changes should be to the same object
    // and we should see a conflict on the value field
    doc.merge(&mut doc2).unwrap();

    let (val, item_id) = AmReadDoc::get(&doc, &automerge::ROOT, "item")
        .unwrap()
        .unwrap();
    assert!(matches!(
        val,
        automerge::Value::Object(automerge::ObjType::Map)
    ));

    // Check that there's a conflict on the value field (both writes went to same object)
    let values = AmReadDoc::get_all(&doc, &item_id, "value").unwrap();
    assert_eq!(values.len(), 2, "Expected conflict with 2 values");
}

// Test: MapReconciler::put with different keys should create new object
#[test]
fn map_put_different_keys_creates_new_object() {
    let mut doc = automerge::AutoCommit::new();
    let container = MapContainer {
        item: KeyedItem {
            id: "item1".to_string(),
            value: "original".to_string(),
        },
    };
    reconcile(&mut doc, &container).unwrap();

    // Get the ObjId of the original item
    let (_, original_obj_id) = AmReadDoc::get(&doc, &automerge::ROOT, "item")
        .unwrap()
        .unwrap();

    // Update with different id (key doesn't match)
    let container2 = MapContainer {
        item: KeyedItem {
            id: "item2".to_string(), // Different ID!
            value: "new_item".to_string(),
        },
    };
    reconcile(&mut doc, &container2).unwrap();

    // Get the ObjId after update
    let (_, new_obj_id) = AmReadDoc::get(&doc, &automerge::ROOT, "item")
        .unwrap()
        .unwrap();

    // The object IDs should be different - a new object was created
    assert_ne!(
        original_obj_id, new_obj_id,
        "Expected a new object to be created when keys don't match"
    );

    // The new object should have the new ID
    let hydrated: KeyedItem = crate::hydrate_prop(&doc, &automerge::ROOT, "item").unwrap();
    assert_eq!(hydrated.id, "item2");
    assert_eq!(hydrated.value, "new_item");
}

// Test: MapReconciler::put with fresh item (KeyNotFound) over existing keyed item
#[test]
fn map_put_fresh_over_keyed_creates_new_object() {
    let mut doc = automerge::AutoCommit::new();
    let container = MapContainer {
        item: KeyedItem {
            id: "item1".to_string(),
            value: "original".to_string(),
        },
    };
    reconcile(&mut doc, &container).unwrap();

    // Get the ObjId of the original item
    let (_, original_obj_id) = AmReadDoc::get(&doc, &automerge::ROOT, "item")
        .unwrap()
        .unwrap();

    // Update with a "fresh" item (returns KeyNotFound from key())
    let container2 = MapContainer {
        item: FreshItem {
            id: "item1".to_string(), // Same ID value but fresh (no key)
            value: "fresh_value".to_string(),
        },
    };
    reconcile(&mut doc, &container2).unwrap();

    // Get the ObjId after update
    let (_, new_obj_id) = AmReadDoc::get(&doc, &automerge::ROOT, "item")
        .unwrap()
        .unwrap();

    // The object IDs should be different - a new object was created
    // because FreshItem::key() returns KeyNotFound
    assert_ne!(
        original_obj_id, new_obj_id,
        "Expected a new object to be created when incoming item has no key"
    );
}

// Test: SeqReconciler::set with matching keys should update in place
#[test]
fn seq_set_matching_keys_updates_in_place() {
    let mut doc = automerge::AutoCommit::new();
    let container = SeqContainer {
        items: vec![KeyedItem {
            id: "item1".to_string(),
            value: "original".to_string(),
        }],
    };
    reconcile(&mut doc, &container).unwrap();

    // Fork
    let mut doc2 = doc.fork().with_actor("actor2".as_bytes().into());

    // Update with same id on original doc
    let container2 = SeqContainer {
        items: vec![KeyedItem {
            id: "item1".to_string(), // Same ID
            value: "updated".to_string(),
        }],
    };
    reconcile(&mut doc, &container2).unwrap();

    // On fork, also update the item
    let container3 = SeqContainer {
        items: vec![KeyedItem {
            id: "item1".to_string(),
            value: "concurrent".to_string(),
        }],
    };
    reconcile(&mut doc2, &container3).unwrap();

    // Merge
    doc.merge(&mut doc2).unwrap();

    // Get the items list
    let (_, items_id) = AmReadDoc::get(&doc, &automerge::ROOT, "items")
        .unwrap()
        .unwrap();
    let (_, item_id) = AmReadDoc::get(&doc, &items_id, 0_usize).unwrap().unwrap();

    // Check for conflict on value field (both writes went to same object)
    let values = AmReadDoc::get_all(&doc, &item_id, "value").unwrap();
    assert_eq!(
        values.len(),
        2,
        "Expected conflict with 2 values on same object"
    );
}

// Test: SeqReconciler::set with different keys should create new object
#[test]
fn seq_set_different_keys_creates_new_object() {
    let mut doc = automerge::AutoCommit::new();
    let container = SeqContainer {
        items: vec![KeyedItem {
            id: "item1".to_string(),
            value: "original".to_string(),
        }],
    };
    reconcile(&mut doc, &container).unwrap();

    // Get the ObjId of the original item
    let (_, items_id) = AmReadDoc::get(&doc, &automerge::ROOT, "items")
        .unwrap()
        .unwrap();
    let (_, original_obj_id) = AmReadDoc::get(&doc, &items_id, 0_usize).unwrap().unwrap();

    // Update with different id
    let container2 = SeqContainer {
        items: vec![KeyedItem {
            id: "item2".to_string(), // Different ID!
            value: "new_item".to_string(),
        }],
    };
    reconcile(&mut doc, &container2).unwrap();

    // Get the ObjId after update
    let (_, new_obj_id) = AmReadDoc::get(&doc, &items_id, 0_usize).unwrap().unwrap();

    // The object IDs should be different - a new object was created
    assert_ne!(
        original_obj_id, new_obj_id,
        "Expected a new object to be created when keys don't match"
    );

    // The new object should have the new ID
    let items: Vec<KeyedItem> = crate::hydrate_prop(&doc, &automerge::ROOT, "items").unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].id, "item2");
    assert_eq!(items[0].value, "new_item");
}

// Test: SeqReconciler::set with fresh item over existing keyed item
#[test]
fn seq_set_fresh_over_keyed_creates_new_object() {
    let mut doc = automerge::AutoCommit::new();
    let container = SeqContainer {
        items: vec![KeyedItem {
            id: "item1".to_string(),
            value: "original".to_string(),
        }],
    };
    reconcile(&mut doc, &container).unwrap();

    // Get the ObjId of the original item
    let (_, items_id) = AmReadDoc::get(&doc, &automerge::ROOT, "items")
        .unwrap()
        .unwrap();
    let (_, original_obj_id) = AmReadDoc::get(&doc, &items_id, 0_usize).unwrap().unwrap();

    // Update with fresh item
    let container2 = FreshSeqContainer {
        items: vec![FreshItem {
            id: "item1".to_string(),
            value: "fresh_value".to_string(),
        }],
    };
    reconcile(&mut doc, &container2).unwrap();

    // Get the ObjId after update
    let (_, new_obj_id) = AmReadDoc::get(&doc, &items_id, 0_usize).unwrap().unwrap();

    // The object IDs should be different - a new object was created
    assert_ne!(
        original_obj_id, new_obj_id,
        "Expected a new object to be created when incoming item has no key"
    );
}

// Test: reconcile_prop with matching keys should update in place
#[test]
fn reconcile_prop_matching_keys_updates_in_place() {
    let mut doc = automerge::AutoCommit::new();
    let item = KeyedItem {
        id: "item1".to_string(),
        value: "original".to_string(),
    };
    reconcile_prop(&mut doc, automerge::ROOT, "item", &item).unwrap();

    // Fork
    let mut doc2 = doc.fork().with_actor("actor2".as_bytes().into());

    // Update with same id on original doc
    let item2 = KeyedItem {
        id: "item1".to_string(),
        value: "updated".to_string(),
    };
    reconcile_prop(&mut doc, automerge::ROOT, "item", &item2).unwrap();

    // On fork, also update
    let item3 = KeyedItem {
        id: "item1".to_string(),
        value: "concurrent".to_string(),
    };
    reconcile_prop(&mut doc2, automerge::ROOT, "item", &item3).unwrap();

    // Merge
    doc.merge(&mut doc2).unwrap();

    let (_, item_id) = AmReadDoc::get(&doc, &automerge::ROOT, "item")
        .unwrap()
        .unwrap();
    let values = AmReadDoc::get_all(&doc, &item_id, "value").unwrap();
    assert_eq!(
        values.len(),
        2,
        "Expected conflict with 2 values on same object"
    );
}

// Test: reconcile_prop with different keys should create new object
#[test]
fn reconcile_prop_different_keys_creates_new_object() {
    let mut doc = automerge::AutoCommit::new();
    let item = KeyedItem {
        id: "item1".to_string(),
        value: "original".to_string(),
    };
    reconcile_prop(&mut doc, automerge::ROOT, "item", &item).unwrap();

    // Get the ObjId of the original item
    let (_, original_obj_id) = AmReadDoc::get(&doc, &automerge::ROOT, "item")
        .unwrap()
        .unwrap();

    // Update with different id
    let item2 = KeyedItem {
        id: "item2".to_string(),
        value: "new_item".to_string(),
    };
    reconcile_prop(&mut doc, automerge::ROOT, "item", &item2).unwrap();

    // Get the ObjId after update
    let (_, new_obj_id) = AmReadDoc::get(&doc, &automerge::ROOT, "item")
        .unwrap()
        .unwrap();

    // The object IDs should be different - a new object was created
    assert_ne!(
        original_obj_id, new_obj_id,
        "Expected a new object to be created when keys don't match"
    );

    // The new object should have the new ID
    let hydrated: KeyedItem = crate::hydrate_prop(&doc, &automerge::ROOT, "item").unwrap();
    assert_eq!(hydrated.id, "item2");
    assert_eq!(hydrated.value, "new_item");
}

// Test: reconcile_prop with fresh item over existing keyed item
#[test]
fn reconcile_prop_fresh_over_keyed_creates_new_object() {
    let mut doc = automerge::AutoCommit::new();
    let item = KeyedItem {
        id: "item1".to_string(),
        value: "original".to_string(),
    };
    reconcile_prop(&mut doc, automerge::ROOT, "item", &item).unwrap();

    // Get the ObjId of the original item
    let (_, original_obj_id) = AmReadDoc::get(&doc, &automerge::ROOT, "item")
        .unwrap()
        .unwrap();

    // Update with a fresh item (no key)
    let item2 = FreshItem {
        id: "item1".to_string(),
        value: "fresh_value".to_string(),
    };
    reconcile_prop(&mut doc, automerge::ROOT, "item", &item2).unwrap();

    // Get the ObjId after update
    let (_, new_obj_id) = AmReadDoc::get(&doc, &automerge::ROOT, "item")
        .unwrap()
        .unwrap();

    // The object IDs should be different - a new object was created
    assert_ne!(
        original_obj_id, new_obj_id,
        "Expected a new object to be created when incoming item has no key"
    );
}
