//! # AutoSurgeon
//!
//! `autosurgeon` is a library for interaction with [`automerge`] documents in Rust with an API
//! inspired by `serde`. The core of the library are two traits: [`Reconcile`], which describes how
//! to take a rust value and update an automerge document to match the value; and [`Hydrate`],
//! which describes how to create a rust value given an automerge document.
//!
//! Whilst you can implement [`Reconcile`] and [`Hydrate`] manually, `autosurgeon` provides derive
//! macros to do this work mechanically.
//!
//! Additionally `autosurgeon` provides the [`Counter`] and [`Text`] data types which implement
//! [`Reconcile`] and [`Hydrate`] for counters and text respectively.
//!
//! Currently this library does not handle incremental updates, that means that every time you
//! receive concurrent changes from other documents you will need to re-`hydrate` your data
//! structures from your document. This will be addressed in future versions.
//!
//! ## Example
//!
//! Imagine we are writing a program to interact with a document containing some contact details.
//! We start by writing some data types to represent the contact and deriving the [`Reconcile`] and
//! [`Hydrate`] traits.
//!
//! ```rust
//! # use autosurgeon::{Reconcile, Hydrate};
//! #[derive(Debug, Clone, Reconcile, Hydrate, PartialEq)]
//! struct Contact {
//!     name: String,
//!     address: Address,
//! }
//! #[derive(Debug, Clone, Reconcile, Hydrate, PartialEq)]
//! struct Address {
//!    line_one: String,
//!    line_two: Option<String>,
//!    city: String,
//!    postcode: String,
//! }
//! ```
//!
//! First we create a contact and put it into a document
//!
//! ```rust
//! # use autosurgeon::{Reconcile, Hydrate, reconcile};
//! # #[derive(Debug, Clone, Reconcile, Hydrate, PartialEq)]
//! # struct Contact {
//! #     name: String,
//! #     address: Address,
//! # }
//!
//! # #[derive(Debug, Clone, Reconcile, Hydrate, PartialEq)]
//! # struct Address {
//! #    line_one: String,
//! #    line_two: Option<String>,
//! #    city: String,
//! # }
//! let contact = Contact {
//!      name: "Sherlock Holmes".to_string(),
//!      address: Address{
//!          line_one: "221B Baker St".to_string(),
//!          line_two: None,
//!          city: "London".to_string(),
//!      },
//! };
//!
//! let mut doc = automerge::AutoCommit::new();
//! reconcile(&mut doc, &contact).unwrap();
//! ```
//!
//! Now we can reconstruct the contact from the document
//!
//! ```rust
//! # use autosurgeon::{Reconcile, Hydrate, reconcile, hydrate};
//! # #[derive(Debug, Clone, Reconcile, Hydrate, PartialEq)]
//! # struct Contact {
//! #     name: String,
//! #     address: Address,
//! # }
//!
//! # #[derive(Debug, Clone, Reconcile, Hydrate, PartialEq)]
//! # struct Address {
//! #    line_one: String,
//! #    line_two: Option<String>,
//! #    city: String,
//! # }
//! #
//! # let mut contact = Contact {
//! #      name: "Sherlock Holmes".to_string(),
//! #      address: Address{
//! #          line_one: "221B Baker St".to_string(),
//! #          line_two: None,
//! #          city: "London".to_string(),
//! #      },
//! # };
//! #
//! # let mut doc = automerge::AutoCommit::new();
//! # reconcile(&mut doc, &contact).unwrap();
//! let contact2: Contact = hydrate(&doc).unwrap();
//! assert_eq!(contact, contact2);
//! ```
//!
//! `reconcile` is smart though, it doesn't just update everything in the document, it figures out
//! what's changed, which means merging modified documents works as you would imagine. Let's fork
//! our document and make concurrent changes to it, then merge it and see how it looks.
//!
//! ```rust
//! # use autosurgeon::{Reconcile, Hydrate, reconcile, hydrate};
//! # #[derive(Debug, Clone, Reconcile, Hydrate, PartialEq)]
//! # struct Contact {
//! #     name: String,
//! #     address: Address,
//! # }
//! # #[derive(Debug, Clone, Reconcile, Hydrate, PartialEq)]
//! # struct Address {
//! #    line_one: String,
//! #    line_two: Option<String>,
//! #    city: String,
//! # }
//! #
//! # let mut contact = Contact {
//! #      name: "Sherlock Holmes".to_string(),
//! #      address: Address{
//! #          line_one: "221B Baker St".to_string(),
//! #          line_two: None,
//! #          city: "London".to_string(),
//! #      },
//! # };
//! #
//! # let mut doc = automerge::AutoCommit::new();
//! # reconcile(&mut doc, &contact).unwrap();
//! // Fork and make changes
//! let mut doc2 = doc.fork().with_actor(automerge::ActorId::random());
//! let mut contact2: Contact = hydrate(&doc2).unwrap();
//! contact2.name = "Dangermouse".to_string();
//! reconcile(&mut doc2, &contact2).unwrap();
//!
//! // Concurrently on doc1
//! contact.address.line_one = "221C Baker St".to_string();
//! reconcile(&mut doc, &contact).unwrap();
//!
//! // Now merge the documents
//! doc.merge(&mut doc2).unwrap();
//!
//! let merged: Contact = hydrate(&doc).unwrap();
//! assert_eq!(merged, Contact {
//!     name: "Dangermouse".to_string(), // This was updated in the first doc
//!     address: Address {
//!           line_one: "221C Baker St".to_string(), // This was concurrently updated in doc2
//!           line_two: None,
//!           city: "London".to_string(),
//!     }
//! })
//! ```
//!
//! ## Derive Macro
//!
//! ### Automerge Representation
//!
//! The derive macros map rust structs to the automerge structures in a similar manner to `serde`
//!
//! ```rust,no_run
//! struct W {
//!     a: i32,
//!     b: i32,
//! }
//! let w = W { a: 0, b: 0 }; // Represented as `{"a":0,"b":0}`
//!
//! struct X(i32, i32);
//! let x = X(0, 0); // Represented as `[0,0]`
//!
//! struct Y(i32);
//! let y = Y(0); // Represented as just the inner value `0`
//!
//! enum E {
//!     W { a: i32, b: i32 },
//!     X(i32, i32),
//!     Y(i32),
//!     Z,
//! }
//! let w = E::W { a: 0, b: 0 }; // Represented as `{"W":{"a":0,"b":0}}`
//! let x = E::X(0, 0);          // Represented as `{"X":[0,0]}`
//! let y = E::Y(0);             // Represented as `{"Y":0}`
//! let z = E::Z;                // Represented as `"Z"`
//! ```
//!
//! ### The `key` attribute
//!
//! `autosurgeon` will generally do its best to generate smart diffs. But sometimes you know
//! additional information about your data which can make merges smarter. Consider the following
//! scenario where we create a product catalog and then make concurrent changes to it.
//!
//! ```rust
//! # use automerge_test::{assert_doc, map, list};
//! # use autosurgeon::{reconcile, Reconcile, Hydrate};
//! #[derive(Reconcile, Hydrate, Clone, Debug, Eq, PartialEq)]
//! struct Product {
//!     id: u64,
//!     name: String,
//! }
//!
//! #[derive(Reconcile, Hydrate, Clone, Debug, Eq, PartialEq)]
//! struct Catalog {
//!     products: Vec<Product>,
//! }
//!
//! let mut catalog = Catalog {
//!     products: vec![
//!         Product {
//!             id: 1,
//!             name: "Lawnmower".to_string(),
//!         },
//!         Product {
//!             id: 2,
//!             name: "Strimmer".to_string(),
//!         }
//!     ]
//! };
//!
//! // Put the catalog into the document
//! let mut doc = automerge::AutoCommit::new();
//! reconcile(&mut doc, &catalog).unwrap();
//!
//! // Fork the document and insert a new product at the start of the catalog
//! let mut doc2 = doc.fork().with_actor(automerge::ActorId::random());
//! let mut catalog2 = catalog.clone();
//! catalog2.products.insert(0, Product {
//!     id: 3,
//!     name: "Leafblower".to_string(),
//! });
//! reconcile(&mut doc2, &catalog2).unwrap();
//!
//! // Concurrenctly remove a product from the catalog in the original doc
//! catalog.products.remove(0);
//! reconcile(&mut doc, &catalog).unwrap();
//!
//! // Merge the two changes
//! doc.merge(&mut doc2).unwrap();
//! assert_doc!(
//!     doc.document(),
//!     map! {
//!         "products" => { list! {
//!             // This first item is conflicted, we expected it to be the leafblower
//!             { map! {
//!                 "id" => { 2_u64, 3_u64 }, // Conflict on the ID
//!                 "name" => { "Strimmer", "Leafblower" }, // Conflict on the name
//!             }},
//!             { map! {
//!                 "id" => { 2_u64 },
//!                 "name" => { "Strimmer" },
//!             }}
//!         }}
//!     }
//! );
//! ```
//!
//! This is surprising, we have a bunch of merge conflicts on the fields of the first product in
//! the list (as signified by the multiple values in the `{..}` on the inner values of the `map!`)
//! and the second product in the list is also a strimmer. This is because `autosurgeon` has no way
//! of knowing the difference between "I inserted an item at the front of the products list" and "I
//! updated the first item in the products list".
//!
//! But we have an `id` field on the product, we can make autosurgeon aware of this.
//!
//! ```rust
//! # use automerge_test::{assert_doc, map, list};
//! # use autosurgeon::{reconcile, Reconcile, Hydrate};
//! #[derive(Reconcile, Hydrate, Clone, Debug, Eq, PartialEq)]
//! struct Product {
//!     #[key] // This is the important bit
//!     id: u64,
//!     name: String,
//! }
//! ```
//!
//! And with this our concurrent changes look like the following:
//!
//! ```rust
//! # use automerge_test::{assert_doc, map, list};
//! # use autosurgeon::{reconcile, Reconcile, Hydrate};
//! # #[derive(Reconcile, Hydrate, Clone, Debug, Eq, PartialEq)]
//! # struct Product {
//! #     #[key]
//! #     id: u64,
//! #     name: String,
//! # }
//! # #[derive(Reconcile, Hydrate, Clone, Debug, Eq, PartialEq)]
//! # struct Catalog {
//! #     products: Vec<Product>,
//! # }
//! # let mut catalog = Catalog {
//! #     products: vec![
//! #         Product {
//! #             id: 1,
//! #             name: "Lawnmower".to_string(),
//! #         },
//! #         Product {
//! #             id: 2,
//! #             name: "Strimmer".to_string(),
//! #         }
//! #     ]
//! # };
//! # let mut doc = automerge::AutoCommit::new();
//! # reconcile(&mut doc, &catalog).unwrap();
//! # let mut doc2 = doc.fork().with_actor(automerge::ActorId::random());
//! # let mut catalog2 = catalog.clone();
//! # catalog2.products.insert(0, Product {
//! #     id: 3,
//! #     name: "Leafblower".to_string(),
//! # });
//! # reconcile(&mut doc2, &catalog2).unwrap();
//! # catalog.products.remove(0);
//! # reconcile(&mut doc, &catalog).unwrap();
//! doc.merge(&mut doc2).unwrap();
//! assert_doc!(
//!     doc.document(),
//!     map! {
//!         "products" => { list! {
//!             { map! {
//!                 "id" => { 3_u64 },
//!                 "name" => { "Leafblower" },
//!             }},
//!             { map! {
//!                 "id" => { 2_u64 },
//!                 "name" => { "Strimmer" },
//!             }}
//!         }}
//!     }
//! );
//! ```
//!
//! ### Providing Implementations for foreign types
//!
//! Deriving `Hydrate` and `Reconcile` is fine for your own types, but sometimes you are using a
//! type which you did not write. For these situations there are a few attributes you can use.
//!
//! #### `reconcile=`
//!
//! The value of this attribute must be the name of a function with the same signature as
//! [`Reconcile::reconcile`]
//!
//! ```rust
//! # use autosurgeon::{Reconcile, Reconciler};
//! #[derive(Reconcile)]
//! struct File {
//!     #[autosurgeon(reconcile="reconcile_path")]
//!     path: std::path::PathBuf,
//! }
//!
//! fn reconcile_path<R: Reconciler>(
//!         path: &std::path::PathBuf, mut reconciler: R
//! ) -> Result<(), R::Error> {
//!     reconciler.str(path.display().to_string())
//! }
//! ```
//!
//! #### `hydrate=`
//!
//! The value of this attribute must be the name of a function with the same signature as
//! [`Hydrate::hydrate`]
//!
//! ```rust
//! # use autosurgeon::{Hydrate, ReadDoc, Prop, HydrateError};
//! #[derive(Hydrate)]
//! struct File {
//!     #[autosurgeon(hydrate="hydrate_path")]
//!     path: std::path::PathBuf,
//! }
//!
//! fn hydrate_path<'a, D: ReadDoc>(
//!     doc: &D,
//!     obj: &automerge::ObjId,
//!     prop: Prop<'a>,
//! ) -> Result<std::path::PathBuf, HydrateError> {
//!      let inner = String::hydrate(doc, obj, prop)?;
//!      inner.parse().map_err(|e| HydrateError::unexpected(
//!          "a valid path", format!("a path which failed to parse due to {}", e)
//!      ))
//! }
//! ```
//!
//! #### `with=`
//!
//! The value of this attribute must be the name of a module wich has both a `reconcile` function
//! and a `hydrate` function, with the same signatures as [`Reconcile::reconcile`] and
//! [`Hydrate::hydrate`] respectively.
//!
//! ```rust
//! # use autosurgeon::{Reconcile, Hydrate, ReadDoc, Prop, HydrateError};
//! #[derive(Hydrate)]
//! struct File {
//!     #[autosurgeon(with="autosurgeon_path")]
//!     path: std::path::PathBuf,
//! }
//!
//! mod autosurgeon_path {
//!     use autosurgeon::{Reconcile, Reconciler, Hydrate, ReadDoc, Prop, HydrateError};
//!     pub(super) fn hydrate<'a, D: ReadDoc>(
//!         doc: &D,
//!         obj: &automerge::ObjId,
//!         prop: Prop<'a>,
//!     ) -> Result<std::path::PathBuf, HydrateError> {
//!          let inner = String::hydrate(doc, obj, prop)?;
//!          inner.parse().map_err(|e| HydrateError::unexpected(
//!              "a valid path", format!("a path which failed to parse due to {}", e)
//!          ))
//!     }
//!
//!     pub(super) fn reconcile<R: Reconciler>(
//!             path: &std::path::PathBuf, mut reconciler: R
//!     ) -> Result<(), R::Error> {
//!         reconciler.str(path.display().to_string())
//!     }
//! }
//! ```

mod counter;
pub use counter::Counter;
pub mod bytes;
mod doc;
pub use doc::{Doc, ReadDoc};
pub mod hydrate;
#[doc(inline)]
pub use hydrate::{hydrate, hydrate_path, hydrate_prop, Hydrate, HydrateError};
pub mod reconcile;
#[doc(inline)]
pub use reconcile::{
    hydrate_key, reconcile, reconcile_insert, reconcile_prop, Reconcile, ReconcileError, Reconciler,
};
mod text;
pub use text::Text;

mod prop;
pub use prop::Prop;

pub use autosurgeon_derive::{Hydrate, Reconcile};
