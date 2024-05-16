use crate::{
    reconcile::{NoKey, TextReconciler},
    Hydrate, ReadDoc, Reconcile,
};

/// A type which reconciles to an [`automerge::ObjType::Text`]
///
/// The intended way to use this, as with [`crate::Counter`], is as a field of a struct which implements
/// [`Reconcile`]. Each time you wish to make a change to the text object you hydrate the struct,
/// make mutating calls to [`Text::splice`], and then once you're done [`crate::reconcile()`] the struct
/// with the document.
///
/// **important** Attempting to reconcile this struct with a document whose heads have changed
/// since the struct was rehydrated will throw a [`crate::reconcile::ReconcileError::StaleHeads`] error.
///
/// # Example
///
/// ```rust
/// # use automerge::ActorId;
/// # use autosurgeon::{reconcile, hydrate, Text, Reconcile, Hydrate};
/// #[derive(Debug, Reconcile, Hydrate)]
/// struct Quote {
///     text: Text,
/// }
/// let mut doc = automerge::AutoCommit::new();
/// let quote = Quote {
///     text: "glimmers".into()
/// };
/// reconcile(&mut doc, &quote).unwrap();
///
/// // Fork and make changes to the text
/// let mut doc2 = doc.fork().with_actor(ActorId::random());
/// let mut quote2: Quote = hydrate(&doc2).unwrap();
/// quote2.text.splice(0, 0, "All that ");
/// let end_index = quote2.text.as_str().char_indices().last().unwrap().0;
/// quote2.text.splice(end_index + 1, 0, " is not gold");
/// reconcile(&mut doc2, &quote2).unwrap();
///
/// // Concurrently modify the text in the original doc
/// let mut quote: Quote = hydrate(&doc).unwrap();
/// let m_index = quote.text.as_str().char_indices().nth(3).unwrap().0;
/// quote.text.splice(m_index, 2, "tt");
/// reconcile(&mut doc, quote).unwrap();
///
/// // Merge the changes
/// doc.merge(&mut doc2).unwrap();
///
/// let quote: Quote = hydrate(&doc).unwrap();
/// assert_eq!(quote.text.as_str(), "All that glitters is not gold");
/// ```
#[derive(Clone)]
pub struct Text(State);

impl std::default::Default for Text {
    fn default() -> Self {
        Text::with_value("")
    }
}

impl std::fmt::Debug for Text {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Text")
            .field("value", &self.as_str())
            .finish()
    }
}

impl Text {
    pub fn with_value<S: AsRef<str>>(value: S) -> Text {
        Self(State::Fresh(value.as_ref().to_string()))
    }

    /// Update the value of the `Text`
    ///
    /// # Arguments
    ///
    /// * pos - The index to start the splice at
    /// * del - The number of characters to delete. This can be negative to indicate deleting `del`
    ///         characters preceding `pos`
    /// * insert - The characters to insert
    ///
    /// The `pos` index uses the same logic as [`String::replace_range`]. This means
    /// that the same caveats apply with regards to the indices you can use. To find the correct
    /// index to start a splice at you use logic such as `String::char_indices`.
    ///
    /// # Panics
    ///
    /// Panics if the starting point or end point do not lie on a char boundary, or if they’re out
    /// of bounds.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use autosurgeon::Text;
    /// let mut value = Text::with_value("some value");
    /// // Get char index of the "v"
    /// let i = value.as_str().char_indices().nth(5).unwrap().0;
    /// value.splice(i, 0, "amazing ");
    /// assert_eq!(value.as_str(), "some amazing value");
    /// ```
    pub fn splice<S: AsRef<str>>(&mut self, pos: usize, del: isize, insert: S) {
        let start = if del < 0 {
            pos.saturating_sub(del.unsigned_abs())
        } else {
            pos
        };
        match &mut self.0 {
            State::Fresh(v) => {
                v.replace_range(start..(start + del.unsigned_abs()), insert.as_ref())
            }
            State::Rehydrated { value, edits, .. } => {
                value.replace_range(start..(start + del.unsigned_abs()), insert.as_ref());
                edits.push(Splice {
                    pos,
                    delete: del,
                    insert: insert.as_ref().to_string(),
                });
            }
        }
    }

    /// Update the value of the text field by diffing it with a new string
    ///
    /// This is useful if you can't capture the edits to a text field as they happen (i.e. the
    /// insertion and deletion events) but instead you just get given the new value of the field.
    /// This method will diff the new value with the current value and convert the diff into a set
    /// of edits which are applied to the text field. This will produce more confusing merge
    /// results than capturing the edits directly, but sometimes it's all you can do.
    ///
    /// ## Example
    ///
    /// ```rust
    /// # use autosurgeon::{Hydrate, Reconcile, Text};
    /// #[derive(Hydrate, Reconcile)]
    /// struct TextDoc {
    ///     content: Text,
    /// }
    ///
    /// let start = TextDoc {
    ///     content: Text::with_value("some value"),
    /// };
    ///
    /// // Create the initial document
    /// let mut doc = automerge::AutoCommit::new();
    /// autosurgeon::reconcile(&mut doc, &start).unwrap();
    ///
    /// // Fork the document so we can make concurrent changes
    /// let mut doc2 = doc.fork();
    ///
    /// // On one fork replace 'value' with 'day'
    /// let mut start2 = autosurgeon::hydrate::<_, TextDoc>(&doc).unwrap();
    /// // Note the use of `update` to replace the entire content instead of `splice`
    /// start2.content.update("some day");
    /// autosurgeon::reconcile(&mut doc, &start2).unwrap();
    ///
    /// // On the other fork replace 'some' with 'another'
    /// let mut start3 = autosurgeon::hydrate::<_, TextDoc>(&doc2).unwrap();
    /// start3.content.update("another value");
    /// autosurgeon::reconcile(&mut doc2, &start3).unwrap();
    ///
    /// // Merge the two forks
    /// doc.merge(&mut doc2).unwrap();
    ///
    /// // The result is 'another day'
    /// let start3 = autosurgeon::hydrate::<_, TextDoc>(&doc).unwrap();
    /// assert_eq!(start3.content.as_str(), "another day");
    /// ```
    pub fn update<S: AsRef<str>>(&mut self, new_value: S) {
        match &mut self.0 {
            State::Fresh(v) => *v = new_value.as_ref().to_string(),
            State::Rehydrated { value, .. } => {
                let mut idx = 0;
                let old = value.clone();
                for change in similar::TextDiff::from_graphemes(old.as_str(), new_value.as_ref())
                    .iter_all_changes()
                {
                    match change.tag() {
                        similar::ChangeTag::Delete => {
                            let len = change.value().len();
                            self.splice(idx, len as isize, "");
                        }
                        similar::ChangeTag::Insert => {
                            self.splice(idx, 0, change.value());
                            idx += change.value().len();
                        }
                        similar::ChangeTag::Equal => {
                            idx += change.value().len();
                        }
                    }
                }
            }
        }
    }

    pub fn as_str(&self) -> &str {
        match &self.0 {
            State::Fresh(v) => v,
            State::Rehydrated { value, .. } => value,
        }
    }
}

impl<S: AsRef<str>> From<S> for Text {
    fn from(s: S) -> Self {
        Text::with_value(s)
    }
}

impl std::cmp::PartialEq for Text {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl std::cmp::Eq for Text {}

#[derive(Clone)]
enum State {
    Fresh(String),
    Rehydrated {
        value: String,
        edits: Vec<Splice>,
        from_heads: Vec<automerge::ChangeHash>,
    },
}

#[derive(Clone)]
struct Splice {
    pos: usize,
    delete: isize,
    insert: String,
}

impl Reconcile for Text {
    type Key<'a> = NoKey;

    fn reconcile<R: crate::Reconciler>(&self, mut reconciler: R) -> Result<(), R::Error> {
        let mut t = reconciler.text()?;
        match &self.0 {
            State::Fresh(v) => {
                t.splice(0, 0, v)?;
            }
            State::Rehydrated {
                edits, from_heads, ..
            } => {
                let to_heads = t.heads();
                if to_heads != from_heads {
                    return Err(crate::reconcile::StaleHeads {
                        expected: from_heads.to_vec(),
                        found: to_heads.to_vec(),
                    }
                    .into());
                } else {
                    for edit in edits {
                        t.splice(edit.pos, edit.delete, &edit.insert)?;
                    }
                }
            }
        }
        Ok(())
    }
}

impl Hydrate for Text {
    fn hydrate_text<D: ReadDoc>(
        doc: &D,
        obj: &automerge::ObjId,
    ) -> Result<Self, crate::HydrateError> {
        let value = doc.text(obj)?;
        Ok(Text(State::Rehydrated {
            value,
            edits: Vec::new(),
            from_heads: doc.get_heads(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use automerge::ActorId;

    use crate::{hydrate_prop, reconcile_prop};

    use super::Text;

    #[test]
    fn merge_text() {
        let mut doc1 = automerge::AutoCommit::new();
        let text = Text::with_value("glitters");
        reconcile_prop(&mut doc1, automerge::ROOT, "text", &text).unwrap();
        let mut doc2 = doc1.fork().with_actor(ActorId::random());

        let mut text1: Text = hydrate_prop(&doc1, &automerge::ROOT, "text").unwrap();
        let mut text2: Text = hydrate_prop(&doc1, &automerge::ROOT, "text").unwrap();

        text1.splice(0, 0, "all that ");
        reconcile_prop(&mut doc1, automerge::ROOT, "text", &text1).unwrap();

        let offset = text2.as_str().char_indices().last().unwrap().0;
        text2.splice(offset + 1, 0, " is not gold");
        reconcile_prop(&mut doc2, automerge::ROOT, "text", &text2).unwrap();

        doc1.merge(&mut doc2).unwrap();

        let result: Text = hydrate_prop(&doc1, &automerge::ROOT, "text").unwrap();
        assert_eq!(result.as_str(), "all that glitters is not gold");
    }

    #[test]
    fn test_partial_eq() {
        let text = Text::with_value("hello");
        assert_eq!(text, Text::with_value("hello"));
        assert_ne!(text, Text::with_value("world"));
    }

    #[test]
    fn test_eq() {
        let text: Text = Text::with_value("hello");
        assert_eq!(text, text);
    }
}
