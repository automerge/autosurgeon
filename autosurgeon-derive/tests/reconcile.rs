use automerge::transaction::Transactable;
use automerge_test::{assert_doc, list, map};
use autosurgeon::{reconcile, reconcile::reconcile_insert, reconcile::reconcile_prop, Reconcile};

#[derive(Reconcile)]
struct Company {
    employees: Vec<Employee>,
    name: String,
}

#[derive(Reconcile)]
struct Employee {
    name: String,
    number: u64,
}

#[test]
fn basic_struct_reconcile() {
    let mut doc = automerge::AutoCommit::new();
    let facebook = Company {
        name: "Meta".to_string(),
        employees: vec![Employee {
            name: "Yann LeCun".to_string(),
            number: 8,
        }],
    };
    reconcile(&mut doc, facebook).unwrap();
    assert_doc!(
        doc.document(),
        map! {
            "name" => { "Meta" },
            "employees" => { list! {
                { map! {
                    "name" => { "Yann LeCun" },
                    "number" => { 8_u64 },
               }}
            }}
        }
    )
}

#[derive(Reconcile)]
struct SpecialString(String);

#[test]
fn test_newtype_struct_reconcile() {
    let mut doc = automerge::AutoCommit::new();
    reconcile_prop(
        &mut doc,
        automerge::ROOT,
        "special",
        SpecialString("special".to_string()),
    )
    .unwrap();
    assert_doc!(
        doc.document(),
        map! {
            "special" => { "special" }
        }
    );
}

#[derive(Reconcile)]
struct CartesianCoordinate(f64, f64);

#[test]
fn test_unnamed_struct_variant_reconcile() {
    let mut doc = automerge::AutoCommit::new();
    reconcile_prop(
        &mut doc,
        automerge::ROOT,
        "coordinate",
        CartesianCoordinate(5.4, 3.2),
    )
    .unwrap();
    assert_doc!(
        doc.document(),
        map! {
            "coordinate" => { list! { { 5.4 }, { 3.2 }}}
        }
    );
}

#[derive(Reconcile)]
enum Color {
    Red,
}

#[test]
fn enum_no_variant_reconcile() {
    let mut doc = automerge::AutoCommit::new();
    let colors = doc
        .put_object(automerge::ROOT, "colors", automerge::ObjType::List)
        .unwrap();
    reconcile_insert(&mut doc, colors, 0, Color::Red).unwrap();
    assert_doc!(
        doc.document(),
        map! {
            "colors" => { list! {
                { "Red" },
            }}
        }
    );
}

#[derive(Reconcile)]
enum TvCommand {
    VolumeUp { amount: f64 },
}

#[test]
fn enum_named_field_variant_reconcile() {
    let mut doc = automerge::AutoCommit::new();
    reconcile(&mut doc, TvCommand::VolumeUp { amount: 10.0 }).unwrap();
    assert_doc!(
        doc.document(),
        map! {
            "VolumeUp" => { map! {
                "amount" => { 10.0 }
            }}
        }
    );
}

#[derive(Reconcile)]
enum RefString<'a> {
    Ref { theref: &'a str },
}

#[test]
fn enum_namedfield_with_refs() {
    let mut doc = automerge::AutoCommit::new();
    reconcile(&mut doc, RefString::Ref { theref: "somestr" }).unwrap();
    assert_doc!(
        doc.document(),
        map! {
            "Ref" => { map! {
                "theref" => {{ "somestr"}}
            }}
        }
    );
}

#[derive(Reconcile)]
enum Measurement {
    Amount(f64),
}

#[test]
fn enum_tuple_variant_single_field_reconcile() {
    let mut doc = automerge::AutoCommit::new();
    reconcile(&mut doc, Measurement::Amount(1.2)).unwrap();
    assert_doc!(
        doc.document(),
        map! {
            "Amount" => { 1.2 }
        }
    );
}

#[derive(Reconcile)]
enum Coordinate {
    LatLng(f64, f64),
}

#[test]
fn enum_tuple_variant_multi_field_reconcile() {
    let mut doc = automerge::AutoCommit::new();
    reconcile(&mut doc, Coordinate::LatLng(1.2, 3.4)).unwrap();
    assert_doc!(
        doc.document(),
        map! {
            "LatLng" => { list! { { 1.2 }, { 3.4 }}}
        }
    );
}

#[derive(Reconcile)]
enum CoordinateRef<'b> {
    LatLng(&'b f64, &'b f64),
}

#[test]
fn enum_tuple_variant_with_refs() {
    let mut doc = automerge::AutoCommit::new();
    reconcile(&mut doc, CoordinateRef::LatLng(&1.2, &3.4)).unwrap();
    assert_doc!(
        doc.document(),
        map! {
            "LatLng" => { list! { { 1.2 }, { 3.4 }}}
        }
    );
}

#[derive(Clone, Reconcile)]
struct Cereal {
    name: String,
    #[key]
    id: u64,
}

#[test]
fn reconcile_with_key() {
    let mut doc = automerge::AutoCommit::new();
    let mut cereals = vec![
        Cereal {
            name: "Weetabix".to_string(),
            id: 1,
        },
        Cereal {
            name: "Quavars".to_string(),
            id: 2,
        },
    ];
    reconcile_prop(&mut doc, automerge::ROOT, "cereals", &cereals).unwrap();

    let mut doc2 = doc.fork().with_actor(automerge::ActorId::random());
    let mut cereals2 = cereals.clone();
    cereals2.insert(
        0,
        Cereal {
            name: "Oats".to_string(),
            id: 3,
        },
    );
    reconcile_prop(&mut doc2, automerge::ROOT, "cereals", cereals2).unwrap();

    cereals.remove(0);
    reconcile_prop(&mut doc, automerge::ROOT, "cereals", &cereals).unwrap();

    doc.merge(&mut doc2).unwrap();

    assert_doc!(
        doc.document(),
        map! {
            "cereals" => { list! {
                { map! {
                    "name" => { "Oats" },
                    "id" => { 3_u64 },
                }},
                { map! {
                    "name" => { "Quavars" },
                    "id" => { 2_u64 },
                }},
            }}
        }
    );
}

#[derive(Clone, Reconcile)]
struct SpecialCereal(Cereal);

#[test]
fn reconcile_with_key_newtype_struct() {
    let mut doc = automerge::AutoCommit::new();
    let mut cereals = vec![
        SpecialCereal(Cereal {
            name: "Weetabix".to_string(),
            id: 1,
        }),
        SpecialCereal(Cereal {
            name: "Quavars".to_string(),
            id: 2,
        }),
    ];
    reconcile_prop(&mut doc, automerge::ROOT, "cereals", &cereals).unwrap();

    let mut doc2 = doc.fork().with_actor(automerge::ActorId::random());
    let mut cereals2 = cereals.clone();
    cereals2.insert(
        0,
        SpecialCereal(Cereal {
            name: "Oats".to_string(),
            id: 3,
        }),
    );
    reconcile_prop(&mut doc2, automerge::ROOT, "cereals", cereals2).unwrap();

    cereals.remove(0);
    reconcile_prop(&mut doc, automerge::ROOT, "cereals", &cereals).unwrap();

    doc.merge(&mut doc2).unwrap();

    assert_doc!(
        doc.document(),
        map! {
            "cereals" => { list! {
                { map! {
                    "name" => { "Oats" },
                    "id" => { 3_u64 },
                }},
                { map! {
                    "name" => { "Quavars" },
                    "id" => { 2_u64 },
                }},
            }}
        }
    );
}

#[derive(Clone, Reconcile)]
struct NameWithIndex(String, #[key] u64);

#[test]
fn reconcile_tuple_struct_with_key() {
    let mut doc = automerge::AutoCommit::new();
    let mut names = vec![
        NameWithIndex("one".to_string(), 1),
        NameWithIndex("two".to_string(), 2),
    ];
    reconcile_prop(&mut doc, automerge::ROOT, "names", &names).unwrap();

    let mut doc2 = doc.fork().with_actor(automerge::ActorId::random());
    let mut names2 = names.clone();
    names2.insert(0, NameWithIndex("three".to_string(), 3));
    reconcile_prop(&mut doc2, automerge::ROOT, "names", names2).unwrap();

    names.remove(0);
    reconcile_prop(&mut doc, automerge::ROOT, "names", &names).unwrap();

    doc.merge(&mut doc2).unwrap();

    assert_doc!(
        doc.document(),
        map! {
            "names" => { list! {
                { list! { { "three" }, { 3_u64 } } },
                { list! { { "two" }, { 2_u64 } } },
            }}
        }
    );
}

#[derive(Clone, Reconcile)]
enum Fruit {
    Orange,
    Banana,
    Kiwi,
}

#[test]
fn reconcile_unit_enum_key() {
    let mut doc = automerge::AutoCommit::new();
    let mut fruits = vec![Fruit::Orange, Fruit::Banana];
    reconcile_prop(&mut doc, automerge::ROOT, "fruits", &fruits).unwrap();

    let mut doc2 = doc.fork().with_actor(automerge::ActorId::random());
    let mut fruits2 = fruits.clone();
    fruits2.remove(0);
    reconcile_prop(&mut doc2, automerge::ROOT, "fruits", &fruits2).unwrap();

    fruits.insert(0, Fruit::Kiwi);
    reconcile_prop(&mut doc, automerge::ROOT, "fruits", &fruits).unwrap();

    doc.merge(&mut doc2).unwrap();

    assert_doc!(
        doc.document(),
        map! {
            "fruits" => { list! {
                { "Kiwi" },
                { "Banana" },
            } }
        }
    );
}

#[derive(Clone, Reconcile)]
enum Vehicle {
    Car {
        #[key]
        id: String,
        manufacturer: String,
    },
    Truck {
        #[key]
        id: String,
        num_wheels: u64,
    },
}

#[test]
fn reconcile_struct_enum_key() {
    let mut doc = automerge::AutoCommit::new();
    let mut vehicles = vec![
        Vehicle::Car {
            id: "one".to_string(),
            manufacturer: "ford".to_string(),
        },
        Vehicle::Truck {
            id: "two".to_string(),
            num_wheels: 18,
        },
    ];
    reconcile_prop(&mut doc, automerge::ROOT, "vehicles", &vehicles).unwrap();

    let mut doc2 = doc.fork().with_actor(automerge::ActorId::random());
    let mut vehicles2 = vehicles.clone();
    vehicles2.remove(0);
    reconcile_prop(&mut doc2, automerge::ROOT, "vehicles", &vehicles2).unwrap();

    vehicles.insert(
        0,
        Vehicle::Car {
            id: "three".to_string(),
            manufacturer: "Audi".to_string(),
        },
    );
    let Vehicle::Truck{num_wheels, ..} = &mut vehicles[2] else {
        panic!("should be a truck");
    };
    *num_wheels = 20;
    reconcile_prop(&mut doc, automerge::ROOT, "vehicles", &vehicles).unwrap();

    doc.merge(&mut doc2).unwrap();

    assert_doc!(
        doc.document(),
        map! {
                "vehicles" => { list! {
                    { map! {
                        "Car" => { map! {
                            "id" => { "three" },
                            "manufacturer" => { "Audi" },
                        } }
                    } } ,
                    { map! {
                        "Truck" => { map!{
                            "id" => { "two" },
                            "num_wheels" => { 20_u64 },
                        } }
                    }
                }  }
            }
        }
    );
}

#[derive(Clone, Reconcile)]
enum TempReading {
    Celsius(#[key] String, f64),
    Fahrenheit(#[key] String, f64),
}

#[test]
fn reconcile_tuple_enum_key() {
    let mut doc = automerge::AutoCommit::new();
    let mut temps = vec![
        TempReading::Celsius("one".to_string(), 1.2),
        TempReading::Fahrenheit("two".to_string(), 3.4),
    ];
    reconcile_prop(&mut doc, automerge::ROOT, "temps", &temps).unwrap();

    let mut doc2 = doc.fork().with_actor(automerge::ActorId::random());
    let mut temps2 = temps.clone();
    temps2.remove(0);
    reconcile_prop(&mut doc2, automerge::ROOT, "temps", &temps2).unwrap();

    temps.insert(0, TempReading::Celsius("three".to_string(), 5.6));
    let TempReading::Fahrenheit(_, temp) = &mut temps[2] else {
        panic!("should be a fahrenheit");
    };
    *temp = 7.8;
    reconcile_prop(&mut doc, automerge::ROOT, "temps", &temps).unwrap();

    doc.merge(&mut doc2).unwrap();

    assert_doc!(
        doc.document(),
        map! {
                "temps" => { list! {
                    { map! {
                        "Celsius" => { list! {
                            { "three" },
                            { 5.6_f64 },
                        } }
                    } } ,
                    { map! {
                        "Fahrenheit" => { list!{
                            { "two" },
                            { 7.8_f64 },
                        } }
                    }
                }}
            }
        }
    );
}

mod enumkeyvisibility {
    use autosurgeon::Reconcile;

    // Check that the key type derived by `Reconcile` has the correct visibility
    #[derive(Reconcile)]
    #[allow(dead_code)]
    pub enum Thing {
        ThatThing,
        TheOtherThing,
    }
}

// Reproduce https://github.com/alexjg/autosurgeon/issues/9
#[derive(Reconcile)]
pub enum Ports {
    Range(u16, u16),
    Collection(Vec<u16>),
}
