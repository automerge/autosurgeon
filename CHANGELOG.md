# `0.9.0`

* Update to `automerge` 0.7.0

The update to `automerge` results in a few breaking changes we have to pass on:

* The `MapReonciler::EntriesIter` type should now be an iterator over (Cow<'a,
  str>, automerge::ValueRef<'a>). This is the item for automerge's
  `MapRangeIter` so this should require no syntactic changes
* The `SeqReconciler::ItemIter` should now be an iterator over
  `automerge::ValueRef<'a>`. As above this is the item for automerge's
  `ListRangeIter` so this should require no syntactic changes


## `0.8.7`

* Generalize the type of the doc argument to `reconcile_insert` to allow
  anything which implements `Doc` and not just `AutoCommit`

## `0.8.6

* Update to `automerge` 0.6.0

## `0.8.5`

* Implement `PartialEq` and `Eq` for `Text`

## `0.8.4`

* Implement `Reconcile` for `MaybeMissing` so that `derive(Reconcile)` works
  for types containing a `MaybeMissing` field.


## `0.8.3`

* Add `Text::update` which allows you to specify changes to text by just
  passing the latest version of the text rather than as individual splice
  calls.

## `0.8.2`

* (@teohanhui) Add the `missing=` annotation which allows the user to specify a
  function to call to construct a value if no value was found in the document
* (@teohanhui) Add the `MaybeMissing` which tracks whether a value was present
  in the document at all

## `0.8.1`

* Improvements to macro hygiene courtesy of @teohhanhui

## `0.8.0`

* Upgrade to Automerge 0.5.0
* Allow `TextReconciler::splice_text` to take a negative `del`

## `0.7.1`

No changes, this release is the same as 0.7.0, but I (Alex) published the wrong
code to crates.io for 0.7.0 due to being quite sleepy so I had to yank and
publish 0.7.1

## `0.7.0`

* BREAKING: The `Reconcile` implementation for maps now removes keys from the
  document which are not part of the incoming data

## `0.6.0`

* BREAKING: Add a `Reconcile::Key` to the `Reconcile` implementation for
  `Uuid`.
* Update `autosurgeon-derive` to `syn` 2.0

## `0.5.1`

* Add `Clone` for `Text` and `Counter`

## `0.5.0`

* Add a `with=` adapter for maps which have keys that implement `FromStr` and `ToString`
* Update to `automerge` 4.0

## `0.4.0`

* Delete old keys when reconciling a new enum variant which has different keys
  to the previous variant

## `0.3.2`

* Fix a bug where the code generated for the `Reconcile` implementation for
  enum variants didn't include the full crate path for `autosurgeon`

## `0.3.1`

* Implement `Hydrate` for `Box<T> where T: Hydrate`

## `0.3`

* Update to `automerge` 3.0

## `0.2.2

* Fixed a bug where the wrong key type was generated for enums with a variant
  with one field and a variant with multiple fields

## `0.2.1`

* Add `Hydrate` for HashMap and BTreeMap
* Fix hydrate_path failing to hydrate some items correctly
* Add implementations of Reconcile and Hydrate for Uuid behind the `uuid` feature flag

## `0.2.0`

* **BREAKING** Remove implementation of `Hydrate` for `Vec<u8>`
* Add `ByteArray` and `ByteVec` wrappers for `[u8; N]` and `Vec<u8`>
* Add an implementation of `Hdyrate` for `u8`
* Accept any `Doc` in `reconcile_prop`

## `0.1.1`

* Fix visibility of key types for derived `Reconcile` on enum types not
  matching the visibility of the target enum type

## `0.1.0`

Initial release
