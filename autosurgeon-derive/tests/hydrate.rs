use automerge::{transaction::Transactable, ObjType};
use autosurgeon::{hydrate, hydrate_prop, Hydrate};

#[derive(Debug, Hydrate, PartialEq)]
struct Company {
    employees: Vec<Employee>,
    name: String,
}

#[derive(Debug, Hydrate, PartialEq)]
struct Employee {
    name: String,
    number: u64,
}

#[test]
fn hydrate_struct() {
    let mut doc = automerge::AutoCommit::new();
    let microsoft = doc
        .put_object(automerge::ROOT, "microsoft", ObjType::Map)
        .unwrap();
    doc.put(&microsoft, "name", "Microsoft").unwrap();
    let emps = doc
        .put_object(&microsoft, "employees", ObjType::List)
        .unwrap();
    let satya = doc.insert_object(&emps, 0, ObjType::Map).unwrap();
    doc.put(&satya, "name", "Satya Nadella").unwrap();
    doc.put(&satya, "number", 1_u64).unwrap();

    let result: Company = hydrate_prop(&doc, &automerge::ROOT, "microsoft").unwrap();
    assert_eq!(
        result,
        Company {
            name: "Microsoft".to_string(),
            employees: vec![Employee {
                name: "Satya Nadella".to_string(),
                number: 1_u64,
            }]
        }
    );
}

#[derive(Debug, Hydrate, PartialEq)]
struct SpecialString(String);

#[test]
fn hydrate_newtype_struct() {
    let mut doc = automerge::AutoCommit::new();
    doc.put(&automerge::ROOT, "key", "value").unwrap();
    let result: SpecialString = hydrate_prop(&doc, &automerge::ROOT, "key").unwrap();
    assert_eq!(result, SpecialString("value".to_string()));
}

// Just here to check that generics are propagated correctly
#[derive(Hydrate)]
#[allow(dead_code)]
struct Wrapped<T>(T);

#[derive(Debug, Hydrate, PartialEq)]
struct LatLng(f64, f64);

#[test]
fn hydrate_tuple_struct() {
    let mut doc = automerge::AutoCommit::new();
    let coord = doc
        .put_object(&automerge::ROOT, "coord", ObjType::List)
        .unwrap();
    doc.insert(&coord, 0, 1.2).unwrap();
    doc.insert(&coord, 1, 2.3).unwrap();
    let coordinate: LatLng = hydrate_prop(&doc, &automerge::ROOT, "coord").unwrap();
    assert_eq!(coordinate, LatLng(1.2, 2.3));
}

#[derive(Debug, Hydrate, PartialEq)]
enum Color {
    Red,
    Green,
}

#[test]
fn hydrate_unit_enum() {
    let mut doc = automerge::AutoCommit::new();
    doc.put(&automerge::ROOT, "color", "Red").unwrap();
    let red: Color = hydrate_prop(&doc, &automerge::ROOT, "color").unwrap();
    assert_eq!(red, Color::Red);
}

#[derive(Debug, Hydrate, PartialEq)]
enum Coordinate {
    LatLng { lat: f64, lng: f64 },
}

#[test]
fn hydrate_named_field_enum() {
    let mut doc = automerge::AutoCommit::new();
    let latlng = doc
        .put_object(&automerge::ROOT, "LatLng", ObjType::Map)
        .unwrap();
    doc.put(&latlng, "lat", 1.2).unwrap();
    doc.put(&latlng, "lng", 2.3).unwrap();
    let coordinate: Coordinate = hydrate(&doc).unwrap();
    assert_eq!(coordinate, Coordinate::LatLng { lat: 1.2, lng: 2.3 });
}

#[derive(Debug, Hydrate, PartialEq)]
enum ValueHolder {
    Int(u32),
}

#[test]
fn hydrate_single_value_tuple_enum_variant() {
    let mut doc = automerge::AutoCommit::new();
    doc.put(&automerge::ROOT, "Int", 234_u64).unwrap();
    let holder: ValueHolder = hydrate(&doc).unwrap();
    assert_eq!(holder, ValueHolder::Int(234));
}

#[derive(Debug, Hydrate, PartialEq)]
enum Vector {
    ThreeD(f64, f64, f64),
}

#[test]
fn hydrate_multi_value_tuple_enum_variant() {
    let mut doc = automerge::AutoCommit::new();
    let three = doc
        .put_object(&automerge::ROOT, "ThreeD", ObjType::List)
        .unwrap();
    doc.insert(&three, 0, 1.2).unwrap();
    doc.insert(&three, 1, 3.4).unwrap();
    doc.insert(&three, 2, 5.6).unwrap();
    let vec: Vector = hydrate(&doc).unwrap();
    assert_eq!(vec, Vector::ThreeD(1.2, 3.4, 5.6));
}
