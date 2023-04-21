use crate::{reconcile::CounterReconciler, Hydrate, Reconcile};

/// A type which reconciles to an [`automerge::ScalarValue::Counter`]
///
/// # Example
///
/// ```rust
/// # use autosurgeon::{Counter, reconcile, hydrate, Reconcile, Hydrate};
/// # use automerge::ActorId;
/// #[derive(Debug, Reconcile, Hydrate)]
/// struct Stats {
///     num_clicks: Counter,
/// }
/// let mut doc = automerge::AutoCommit::new();
/// let mut stats = Stats {num_clicks: Counter::default() };
/// reconcile(&mut doc, &stats).unwrap();
///
/// // Fork the doc and increment the counter
/// let mut doc2 = doc.fork().with_actor(ActorId::random());
/// let mut stats2: Stats = hydrate(&doc).unwrap();
/// stats2.num_clicks.increment(5);
/// reconcile(&mut doc2, &stats2).unwrap();
///
/// // Concurrently increment in the original doc
/// let mut stats: Stats = hydrate(&doc).unwrap();
/// stats.num_clicks.increment(3);
/// reconcile(&mut doc, &stats).unwrap();
///
/// // Merge the two docs
/// doc.merge(&mut doc2).unwrap();
///
/// // Observe that `num_clicks` is the sum of the concurrent increments
/// let stats: Stats = hydrate(&doc).unwrap();
/// assert_eq!(stats.num_clicks.value(), 8);
/// ```
#[derive(Clone)]
pub struct Counter(State);

impl std::fmt::Debug for Counter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Counter")
            .field("value", &self.value())
            .finish()
    }
}

impl std::default::Default for Counter {
    fn default() -> Self {
        Self::with_value(0)
    }
}

#[derive(Clone)]
enum State {
    Fresh(i64),
    Rehydrated { original: i64, increment: i64 },
}

impl Counter {
    pub fn with_value(value: i64) -> Self {
        Self(State::Fresh(value))
    }

    pub fn increment(&mut self, by: i64) {
        match &mut self.0 {
            State::Fresh(v) => *v += by,
            State::Rehydrated { increment, .. } => *increment += by,
        }
    }

    pub fn value(&self) -> i64 {
        match self.0 {
            State::Fresh(v) => v,
            State::Rehydrated {
                original,
                increment,
            } => original + increment,
        }
    }
}

impl Reconcile for Counter {
    type Key<'a> = crate::reconcile::NoKey;

    fn reconcile<R: crate::Reconciler>(&self, mut reconciler: R) -> Result<(), R::Error> {
        let mut c = reconciler.counter()?;
        match self.0 {
            State::Fresh(v) => c.set(v)?,
            State::Rehydrated { increment, .. } => c.increment(increment)?,
        };
        Ok(())
    }
}

impl Hydrate for Counter {
    fn hydrate_counter(c: i64) -> Result<Self, crate::HydrateError> {
        Ok(Counter(State::Rehydrated {
            original: c,
            increment: 0,
        }))
    }
}

#[cfg(test)]
mod tests {
    use automerge::ActorId;

    use super::Counter;
    use crate::{hydrate_prop, reconcile_prop};

    #[test]
    fn simple_increment() {
        let mut doc = automerge::AutoCommit::new();
        let counter = Counter::default();
        reconcile_prop(&mut doc, automerge::ROOT, "counter", &counter).unwrap();

        let mut counter2: Counter = hydrate_prop(&doc, &automerge::ROOT, "counter").unwrap();
        let mut doc2 = doc.fork().with_actor(ActorId::random());
        counter2.increment(5);
        reconcile_prop(&mut doc2, automerge::ROOT, "counter", &counter2).unwrap();

        let mut counter3: Counter = hydrate_prop(&doc, &automerge::ROOT, "counter").unwrap();
        counter3.increment(3);
        reconcile_prop(&mut doc, automerge::ROOT, "counter", &counter3).unwrap();

        doc.merge(&mut doc2).unwrap();

        let counter: Counter = hydrate_prop(&doc, &automerge::ROOT, "counter").unwrap();
        assert_eq!(counter.value(), 8);
    }
}
