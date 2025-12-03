use automerge::transaction::Transactable;
use automerge_test::{assert_doc, list, map};
use autosurgeon::{hydrate, reconcile, Hydrate, Reconcile};

#[derive(Debug, Clone, PartialEq, Reconcile, Hydrate)]
struct Document {
    #[autosurgeon(rename = "doc-title")]
    title: String,

    #[autosurgeon(rename = "created_at")]
    created: String,
}

#[test]
fn struct_field_rename_reconcile() {
    let mut doc = automerge::AutoCommit::new();
    let document = Document {
        title: "My Document".to_string(),
        created: "2024-01-01".to_string(),
    };
    reconcile(&mut doc, &document).unwrap();

    // Verify the renamed keys are used in the automerge document
    assert_doc!(
        doc.document(),
        map! {
            "doc-title" => { "My Document" },
            "created_at" => { "2024-01-01" },
        }
    );
}

#[test]
fn struct_field_rename_hydrate() {
    let mut doc = automerge::AutoCommit::new();
    // Manually create the document with renamed keys
    doc.put(automerge::ROOT, "doc-title", "Hydrated Doc")
        .unwrap();
    doc.put(automerge::ROOT, "created_at", "2024-06-15")
        .unwrap();

    let hydrated: Document = hydrate(&doc).unwrap();
    assert_eq!(hydrated.title, "Hydrated Doc");
    assert_eq!(hydrated.created, "2024-06-15");
}

#[test]
fn struct_field_rename_roundtrip() {
    let mut doc = automerge::AutoCommit::new();
    let original = Document {
        title: "Roundtrip Test".to_string(),
        created: "2024-03-20".to_string(),
    };
    reconcile(&mut doc, &original).unwrap();

    let hydrated: Document = hydrate(&doc).unwrap();
    assert_eq!(original, hydrated);
}

// Test rename with non-Rust-identifier name (hyphens, etc.)
#[derive(Debug, Clone, PartialEq, Reconcile, Hydrate)]
struct Config {
    #[autosurgeon(rename = "api-key")]
    api_key: String,

    #[autosurgeon(rename = "max-retries")]
    max_retries: u64,

    #[autosurgeon(rename = "enable-feature-x")]
    enable_feature_x: bool,
}

#[test]
fn struct_field_rename_with_hyphens() {
    let mut doc = automerge::AutoCommit::new();
    let config = Config {
        api_key: "secret123".to_string(),
        max_retries: 5,
        enable_feature_x: true,
    };
    reconcile(&mut doc, &config).unwrap();

    assert_doc!(
        doc.document(),
        map! {
            "api-key" => { "secret123" },
            "max-retries" => { 5_u64 },
            "enable-feature-x" => { true },
        }
    );

    let hydrated: Config = hydrate(&doc).unwrap();
    assert_eq!(config, hydrated);
}

// Test rename combined with #[key] attribute
#[derive(Debug, Clone, PartialEq, Reconcile, Hydrate)]
struct Item {
    #[key]
    #[autosurgeon(rename = "item-id")]
    id: String,

    #[autosurgeon(rename = "item-name")]
    name: String,
}

#[test]
fn struct_field_rename_with_key() {
    use autosurgeon::reconcile::reconcile_prop;

    let mut doc = automerge::AutoCommit::new();
    let items = vec![
        Item {
            id: "1".to_string(),
            name: "First".to_string(),
        },
        Item {
            id: "2".to_string(),
            name: "Second".to_string(),
        },
    ];
    reconcile_prop(&mut doc, automerge::ROOT, "items", &items).unwrap();

    assert_doc!(
        doc.document(),
        map! {
            "items" => { list! {
                { map! {
                    "item-id" => { "1" },
                    "item-name" => { "First" },
                }},
                { map! {
                    "item-id" => { "2" },
                    "item-name" => { "Second" },
                }},
            }}
        }
    );

    let hydrated: Vec<Item> = autosurgeon::hydrate_prop(&doc, automerge::ROOT, "items").unwrap();
    assert_eq!(items, hydrated);
}

#[derive(Debug, Clone, PartialEq, Reconcile, Hydrate)]
enum Status {
    #[autosurgeon(rename = "in-progress")]
    InProgress,

    #[autosurgeon(rename = "done")]
    Completed,

    Active,
}

#[test]
fn enum_unit_variant_rename_reconcile() {
    use autosurgeon::reconcile::reconcile_prop;

    let mut doc = automerge::AutoCommit::new();
    reconcile_prop(&mut doc, automerge::ROOT, "status", Status::InProgress).unwrap();

    assert_doc!(
        doc.document(),
        map! {
            "status" => { "in-progress" },
        }
    );

    let mut doc2 = automerge::AutoCommit::new();
    reconcile_prop(&mut doc2, automerge::ROOT, "status", Status::Completed).unwrap();
    assert_doc!(
        doc2.document(),
        map! {
            "status" => { "done" },
        }
    );

    // Non-renamed variant
    let mut doc3 = automerge::AutoCommit::new();
    reconcile_prop(&mut doc3, automerge::ROOT, "status", Status::Active).unwrap();
    assert_doc!(
        doc3.document(),
        map! {
            "status" => { "Active" },
        }
    );
}

#[test]
fn enum_unit_variant_rename_hydrate() {
    let mut doc = automerge::AutoCommit::new();
    doc.put(automerge::ROOT, "status", "in-progress").unwrap();

    let status: Status = autosurgeon::hydrate_prop(&doc, automerge::ROOT, "status").unwrap();
    assert_eq!(status, Status::InProgress);

    let mut doc2 = automerge::AutoCommit::new();
    doc2.put(automerge::ROOT, "status", "done").unwrap();

    let status2: Status = autosurgeon::hydrate_prop(&doc2, automerge::ROOT, "status").unwrap();
    assert_eq!(status2, Status::Completed);
}

#[derive(Debug, Clone, PartialEq, Reconcile, Hydrate)]
enum Measurement {
    #[autosurgeon(rename = "temp-celsius")]
    Celsius(f64),

    #[autosurgeon(rename = "temp-fahrenheit")]
    Fahrenheit(f64),
}

#[test]
fn enum_newtype_variant_rename() {
    let mut doc = automerge::AutoCommit::new();
    reconcile(&mut doc, Measurement::Celsius(25.5)).unwrap();

    assert_doc!(
        doc.document(),
        map! {
            "temp-celsius" => { 25.5 },
        }
    );

    let hydrated: Measurement = hydrate(&doc).unwrap();
    assert_eq!(hydrated, Measurement::Celsius(25.5));
}

#[derive(Debug, Clone, PartialEq, Reconcile, Hydrate)]
enum Command {
    #[autosurgeon(rename = "set-volume")]
    SetVolume { level: u64 },

    #[autosurgeon(rename = "change-channel")]
    ChangeChannel { channel: String },
}

#[test]
fn enum_struct_variant_rename() {
    let mut doc = automerge::AutoCommit::new();
    reconcile(&mut doc, Command::SetVolume { level: 50 }).unwrap();

    assert_doc!(
        doc.document(),
        map! {
            "set-volume" => { map! {
                "level" => { 50_u64 },
            }},
        }
    );

    let hydrated: Command = hydrate(&doc).unwrap();
    assert_eq!(hydrated, Command::SetVolume { level: 50 });
}

#[derive(Debug, Clone, PartialEq, Reconcile, Hydrate)]
enum Coordinate {
    #[autosurgeon(rename = "lat-lng")]
    LatLng(f64, f64),

    #[autosurgeon(rename = "utm-coord")]
    Utm(String, f64, f64),
}

#[test]
fn enum_tuple_variant_rename() {
    let mut doc = automerge::AutoCommit::new();
    reconcile(&mut doc, Coordinate::LatLng(51.5074, -0.1278)).unwrap();

    assert_doc!(
        doc.document(),
        map! {
            "lat-lng" => { list! { { 51.5074 }, { -0.1278 } }},
        }
    );

    let hydrated: Coordinate = hydrate(&doc).unwrap();
    assert_eq!(hydrated, Coordinate::LatLng(51.5074, -0.1278));
}

// Test combining field rename and variant rename in same enum
#[derive(Debug, Clone, PartialEq, Reconcile, Hydrate)]
enum Event {
    #[autosurgeon(rename = "user-login")]
    UserLogin {
        #[autosurgeon(rename = "user-id")]
        user_id: String,
        timestamp: u64,
    },

    #[autosurgeon(rename = "item-purchase")]
    ItemPurchase {
        #[autosurgeon(rename = "item-sku")]
        item_sku: String,
        #[autosurgeon(rename = "price-cents")]
        price_cents: u64,
    },
}

#[test]
fn enum_variant_and_field_rename_combined() {
    let mut doc = automerge::AutoCommit::new();
    reconcile(
        &mut doc,
        Event::UserLogin {
            user_id: "user123".to_string(),
            timestamp: 1234567890,
        },
    )
    .unwrap();

    assert_doc!(
        doc.document(),
        map! {
            "user-login" => { map! {
                "user-id" => { "user123" },
                "timestamp" => { 1234567890_u64 },
            }},
        }
    );

    let hydrated: Event = hydrate(&doc).unwrap();
    assert_eq!(
        hydrated,
        Event::UserLogin {
            user_id: "user123".to_string(),
            timestamp: 1234567890,
        }
    );
}

// Test rename with generic types
#[derive(Debug, Clone, PartialEq, Reconcile, Hydrate)]
struct Container<T: Reconcile + Hydrate + Clone> {
    #[autosurgeon(rename = "inner-value")]
    value: T,

    #[autosurgeon(rename = "meta-data")]
    metadata: String,
}

#[test]
fn struct_field_rename_with_generics() {
    let mut doc = automerge::AutoCommit::new();
    let container = Container {
        value: 42_u64,
        metadata: "test".to_string(),
    };
    reconcile(&mut doc, &container).unwrap();

    assert_doc!(
        doc.document(),
        map! {
            "inner-value" => { 42_u64 },
            "meta-data" => { "test" },
        }
    );

    let hydrated: Container<u64> = hydrate(&doc).unwrap();
    assert_eq!(container, hydrated);
}
