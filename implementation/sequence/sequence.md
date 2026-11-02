# State over a sequence

```turtle folio:document
implementation:sequence\/sequence a x0k:Implementation ;
    x0k:status "draft" ;
    x0k:summary "The parts that all sequence stores share: the scene of a moment, the address of a moment, the set-scene op and the sequence store. One check holds each sequence store to the same contract." ;
    x0k:concerns "sequence", "scene", "moment", "arrangement", "figures" ;
    x0k:cites architecture:malleable-ui-runtime,
        implementation:folio\/inline-entities,
        implementation:document-editing\/tree-page ;
    x0k:implements architecture:arrangement-document ;
    folio:tangleCrate "crates/x0k-sequence" ;
    folio:tangleRoot "src/lib.rs" .
```

A **sequence** is an ordered set of moments. A tour, a telling and the
steps of a figure are sequences. Each **moment** can have a **scene**. A
scene is the state that a view shows at that moment.

A reader makes a scene with one action. The reader sets up a figure,
highlights the important parts and moves the figure to the correct step.
Then the reader selects "Show it here". The section keeps these settings as
its scene. The next reader who gets to the section sees the figure in the
same state.

A **sequence store** keeps the moments of a sequence and writes their
scenes through its own write path. Different kinds of sequence store write
a scene in different ways (`x0k:architecture/arrangement-document` §"One op
vocabulary, one transport"):

- A document edits the scene block of the section.
- A fact store asserts facts about the moment.
- A genome records a genome change.

The code that captures a scene does not know which sequence store it writes
to. This crate contains the parts that all sequence stores share:

- The **scene**: the state of a view at one moment.
- The **address** of a moment.
- The **`set-scene` op**, which writes the scene of a moment.
- The **`SequenceStore` trait**, which reads the scene of a moment and
  applies the op through the write path of the store.

The crate does not know about documents, fact stores or genomes. A document
is the first sequence store (`x0k:implementation/document-editing/tree-page`,
"The page as a sequence store"). The page core captures the view of a
figure. The page core also moves a figure between its positions. That
movement stays in the page core until a second host moves figures.

## A scene

A scene is the state of a view at one moment
(`x0k:architecture/arrangement-document` §"Arrangements in time"). A scene
has two parts:

- A value for each param that the scene sets. A **param** is a named input
  that the view declares.
- The private data of the view.

The scene identifies each param by its local name. The local name is the
name that the declaration of the view gives to the param. The value of a
param is the value that the host sends in `SetParams`. A value is a string,
a number, a boolean or a list of these.

A scene states all that it shows. If a scene does not set a param, the view
uses the value that its author gave. The param does not get its value from
the previous moment.

A scene keeps its params in the order that its author wrote them. A reader
reads them in this order.

The data is one JSON text. The view reads it. Each host passes it on and
does not read it. If a scene has no data, the view gets no data.

A scene can also state a **loop**. A loop operates while the reader stays
at the moment. The host moves one param of the view in a straight line from
`from` to `to`. One pass takes `period` milliseconds. Then the param starts
again at `from`. The loop makes a stated number of passes, or it does not
stop. A sequence on a duration axis can loop over a range
(`x0k:architecture/arrangement-document` §"Arrangements in time").

The host operates the loop, not the view. In each frame, the host gives the
param to the view, as it gives all other values. The view does not know
that a loop moves the param. Thus each frame depends only on its params.

<a name="chunk-scene"></a><sub>[`src/lib.rs`](../../crates/x0k-sequence/src/lib.rs) · `#scene`</sub>

```rust {#scene}
/// One moment's scene: the params it sets, by local name, in the order it
/// states them, the loop its host runs while it rests, and the view's
/// private data.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scene {
    /// Each param the scene sets, by local name, in the order it names them.
    pub params: Vec<(String, serde_json::Value)>,
    /// The loop its host runs while the moment rests, when it states one.
    pub looping: Option<Loop>,
    /// The view's private data, one JSON text passed through unread, when
    /// the scene states it.
    pub data: Option<String>,
}

/// A loop a host runs while a moment rests: `param` carried linearly from
/// `from` to `to` over `period` milliseconds, then from `from` again,
/// `count` times or without end.
#[derive(Clone, Debug, PartialEq)]
pub struct Loop {
    /// The view's param it carries, by local name.
    pub param: String,
    pub from: f64,
    pub to: f64,
    /// Milliseconds one pass takes.
    pub period: f64,
    /// How many passes it makes before it stops at `to`; `None` runs
    /// without end.
    pub count: Option<u32>,
}
```

A document states a scene as a graph block in its section
(`x0k:implementation/folio/inline-entities` §"A section's scene"). Folio
reads and writes that block as this `Scene`. Thus the sequence store owns
the format, and all sequence stores share the value.

## A moment's address

A sequence store contains sequences, and a sequence contains moments. The
address of a moment identifies the sequence and the moment. Each of the two
is a key that the sequence store gives. Only that store reads these keys.

In a document, the key of the sequence is the name that a figure uses for
its blocks. The key of the moment is the heading of the section.

No code outside the sequence store reads the keys. Thus a sequence store
can use any keys that stay stable.

<a name="chunk-address"></a><sub>[`src/lib.rs`](../../crates/x0k-sequence/src/lib.rs) · `#address`</sub>

```rust {#address}
/// Where a moment is: the sequence it is a moment of and the moment, each
/// by the key its sequence store gave it. Only the store reads either key.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MomentRef {
    /// The sequence, by its store's key.
    pub sequence: String,
    /// The moment, by its store's key.
    pub moment: String,
}
```

## The op

An **op** is one change that a sequence store applies. `set-scene` is one
op in the closed set of arrangement ops. The key of the moment addresses
the op.

The op writes the full scene of the moment. A scene states all that it
shows. Thus the op replaces the scene that the moment had. It does not
merge the two scenes.

<a name="chunk-op"></a><sub>[`src/lib.rs`](../../crates/x0k-sequence/src/lib.rs) · `#op`</sub>

```rust {#op}
/// `set-scene`: write `scene` as `moment`'s scene, replacing whatever scene
/// it held.
#[derive(Clone, Debug, PartialEq)]
pub struct SetScene {
    pub moment: MomentRef,
    pub scene: Scene,
}
```

## The sequence store

A sequence store reads the scene of a moment. It also applies `set-scene`
through its own write path. That path gives the store its undo, its
attribution and its save. All sequence stores obey the same three rules:

- **A sequence store reads back what it wrote.** After `set-scene`, the
  scene of the moment is exactly the scene in the op. This includes the
  params, their order, the loop and the data.
- **A sequence store replaces.** After a second `set-scene` on the same
  moment, the moment has the second scene. It does not have a merge of the
  two scenes.
- **A sequence store writes one moment.** All other moments keep the
  scenes that they had. A scene does not get values from the moments near
  it. Thus a write to one moment cannot change a neighbour.

<a name="chunk-store"></a><sub>[`src/lib.rs`](../../crates/x0k-sequence/src/lib.rs) · `#store`</sub>

```rust {#store}
/// Where a sequence's moments live: a sequence store reads a moment's scene
/// and applies `set-scene` through its own write path.
pub trait SequenceStore {
    /// Why the store refused a read or an op.
    type Error;

    /// The scene `moment` holds of its own; `None` when it holds none.
    fn scene(&self, moment: &MomentRef) -> Result<Option<Scene>, Self::Error>;

    /// Write `op.scene` as `op.moment`'s scene: afterwards the moment holds
    /// exactly that scene, and every other moment what it held before.
    fn set_scene(&mut self, op: &SetScene) -> Result<(), Self::Error>;
}
```

One function, `check_store`, checks the three rules for every sequence
store. It uses two moments of the store: the moment it writes, and a second
moment. It does these steps:

1. It reads the scene of the second moment.
2. It writes a scene at the first moment and reads it back.
3. It writes the empty scene at the first moment and reads it back.
4. It writes the first scene again and reads it back.
5. It reads the scene of the second moment again. This scene must be the
   same as the scene from step 1.

The tests of each sequence store call `check_store` with two moments of
that store. Thus a document, a fact store and a genome obey one contract.

When a sequence store breaks a rule, `check_store` returns a **breach**. A
breach contains the scenes that the check found. Thus a failed test shows
what the check wrote and what it read back. A breach is large. But
`check_store` returns a breach only when a store breaks the contract. Thus
its size has no cost when the check passes.

<a name="chunk-check"></a><sub>[`src/lib.rs`](../../crates/x0k-sequence/src/lib.rs) · `#check`</sub>

```rust {#check}
/// How a sequence store broke its contract, as [`check_store`] finds it.
#[derive(Debug, PartialEq)]
pub enum Breach<E> {
    /// The store refused a read or an op.
    Refused(E),
    /// A moment did not read back the scene last written to it.
    Unread { wrote: Scene, read: Option<Scene> },
    /// Writing one moment changed another's scene.
    Disturbed { before: Option<Scene>, after: Option<Scene> },
}

/// Hold `store` to its contract with two of its moments: `scene` written at
/// `here` reads back, the empty scene written over it replaces it, `scene`
/// written again reads back, and `there` holds what it held before.
#[allow(
    clippy::result_large_err,
    reason = "a breach carries the scenes it found and is returned only when a store breaks its contract"
)]
pub fn check_store<S: SequenceStore + ?Sized>(store: &mut S, here: &MomentRef, there: &MomentRef, scene: &Scene) -> Result<(), Breach<S::Error>> {
    let before = store.scene(there).map_err(Breach::Refused)?;
    for wrote in [scene.clone(), Scene::default(), scene.clone()] {
        store.set_scene(&SetScene { moment: here.clone(), scene: wrote.clone() }).map_err(Breach::Refused)?;
        let read = store.scene(here).map_err(Breach::Refused)?;
        if read.as_ref() != Some(&wrote) {
            return Err(Breach::Unread { wrote, read });
        }
    }
    let after = store.scene(there).map_err(Breach::Refused)?;
    if after != before {
        return Err(Breach::Disturbed { before, after });
    }
    Ok(())
}
```

## The crate

The crate has one dependency, `serde_json`, for the values of params. The
crate builds for all targets that a host operates on. This includes a
browser.

<a name="chunk-lib"></a><sub>[`src/lib.rs`](../../crates/x0k-sequence/src/lib.rs) · `#lib` · assembles [scene](#chunk-scene) · [address](#chunk-address) · [op](#chunk-op) · [store](#chunk-store) · [check](#chunk-check)</sub>

```rust {#lib file="src/lib.rs"}
//! State over a sequence: the scene of a moment, the address of a moment,
//! the `set-scene` op and the sequence store that applies it
//! (`x0k:implementation/sequence/sequence`). The crate knows no document,
//! fact store or genome.

<<scene>>

<<address>>

<<op>>

<<store>>

<<check>>

#[cfg(test)]
mod tests;
```

<a name="chunk-manifest"></a><sub>[`Cargo.toml`](../../crates/x0k-sequence/Cargo.toml) · `#manifest`</sub>

```toml {#manifest file="Cargo.toml"}
[package]
name = "x0k-sequence"
version = "0.2.0"
edition = { workspace = true }
license = "MIT"
description = "State over a sequence: the scene of a moment, the address of a moment, the set-scene op and the sequence store that applies it"
rust-version = { workspace = true }
repository = "https://github.com/0k-dot-computer/x0k-folio"
readme = "../../README.md"
keywords = ["literate-programming", "tangle", "markdown", "documentation"]

[lib]
path = "src/lib.rs"

[dependencies]
serde_json = { workspace = true }
```

## What the tests pin

The tests check the contract without a document. They use a sequence store
that keeps its scenes in a map. That store obeys the contract.

Two other stores each break the contract. The check refuses both:

- One store merges a new scene into the old scene. It breaks the rule that
  a sequence store replaces.
- One store writes all scenes to the first moment that it got. It breaks
  the rule that a sequence store reads back what it wrote.

<a name="chunk-tests"></a><sub>[`src/tests.rs`](../../crates/x0k-sequence/src/tests.rs) · `#tests`</sub>

```rust {#tests file="src/tests.rs"}
use super::*;
use std::collections::BTreeMap;

/// A sequence store that keeps each moment's scene in a map.
#[derive(Default)]
struct MemoryStore {
    scenes: BTreeMap<MomentRef, Scene>,
}

impl SequenceStore for MemoryStore {
    type Error = std::convert::Infallible;

    fn scene(&self, moment: &MomentRef) -> Result<Option<Scene>, Self::Error> {
        Ok(self.scenes.get(moment).cloned())
    }

    fn set_scene(&mut self, op: &SetScene) -> Result<(), Self::Error> {
        self.scenes.insert(op.moment.clone(), op.scene.clone());
        Ok(())
    }
}

fn moment(key: &str) -> MomentRef {
    MomentRef { sequence: "run".into(), moment: key.into() }
}

/// A slide's scene: the record lit, the camera on it, the run's data.
fn slide() -> Scene {
    Scene {
        params: vec![
            ("letters".into(), serde_json::json!("NA")),
            ("camera".into(), serde_json::json!("1")),
            ("emphasis".into(), serde_json::json!(["1", "1.0"])),
        ],
        looping: None,
        data: Some(r#"{"value": "(<66> ('Add' 21 21))"}"#.into()),
    }
}

#[test]
fn a_store_that_keeps_its_scenes_keeps_the_contract() {
    let mut store = MemoryStore::default();
    let neighbour = Scene { params: vec![("letters".into(), serde_json::json!("N"))], looping: None, data: None };
    store.set_scene(&SetScene { moment: moment("17"), scene: neighbour.clone() }).unwrap();
    assert_eq!(check_store(&mut store, &moment("18"), &moment("17"), &slide()), Ok(()));
    assert_eq!(store.scene(&moment("18")).unwrap(), Some(slide()));
    assert_eq!(store.scene(&moment("17")).unwrap(), Some(neighbour));
    assert_eq!(store.scene(&moment("19")).unwrap(), None, "a moment never written holds no scene");
}

/// A sequence store that merges a new scene's params into the old one's.
#[derive(Default)]
struct MergingStore(MemoryStore);

impl SequenceStore for MergingStore {
    type Error = std::convert::Infallible;

    fn scene(&self, moment: &MomentRef) -> Result<Option<Scene>, Self::Error> {
        self.0.scene(moment)
    }

    fn set_scene(&mut self, op: &SetScene) -> Result<(), Self::Error> {
        let held = self.0.scenes.entry(op.moment.clone()).or_default();
        for (name, value) in &op.scene.params {
            held.params.retain(|(held, _)| held != name);
            held.params.push((name.clone(), value.clone()));
        }
        held.data = op.scene.data.clone().or(held.data.take());
        Ok(())
    }
}

/// A sequence store that writes every scene to the first moment it was given.
#[derive(Default)]
struct StuckStore(MemoryStore, Option<MomentRef>);

impl SequenceStore for StuckStore {
    type Error = std::convert::Infallible;

    fn scene(&self, moment: &MomentRef) -> Result<Option<Scene>, Self::Error> {
        self.0.scene(moment)
    }

    fn set_scene(&mut self, op: &SetScene) -> Result<(), Self::Error> {
        let at = self.1.get_or_insert_with(|| op.moment.clone()).clone();
        self.0.set_scene(&SetScene { moment: at, scene: op.scene.clone() })
    }
}

#[test]
fn a_store_that_merges_or_writes_elsewhere_breaks_the_contract() {
    let mut merging = MergingStore::default();
    let refused = check_store(&mut merging, &moment("18"), &moment("17"), &slide());
    assert_eq!(refused, Err(Breach::Unread { wrote: Scene::default(), read: Some(slide()) }), "the empty scene must replace");
    let mut stuck = StuckStore::default();
    stuck.set_scene(&SetScene { moment: moment("17"), scene: Scene::default() }).unwrap();
    assert!(matches!(check_store(&mut stuck, &moment("18"), &moment("17"), &slide()), Err(Breach::Unread { .. })), "it never reads back at 18");
}
```

A sequence store can write the correct moment and also change a different
moment. The check refuses this store as `Disturbed`. The test uses a store
that clears all other moments when it writes one.

<a name="chunk-tests-2"></a><sub>[`src/lib.rs`](../../crates/x0k-sequence/src/lib.rs) · `#tests` · continues</sub>

```rust {#tests}

/// A sequence store that forgets every other moment's scene when it writes one.
#[derive(Default)]
struct SweepingStore(MemoryStore);

impl SequenceStore for SweepingStore {
    type Error = std::convert::Infallible;

    fn scene(&self, moment: &MomentRef) -> Result<Option<Scene>, Self::Error> {
        self.0.scene(moment)
    }

    fn set_scene(&mut self, op: &SetScene) -> Result<(), Self::Error> {
        self.0.scenes.clear();
        self.0.set_scene(op)
    }
}

#[test]
fn a_store_that_changes_another_moment_breaks_the_contract() {
    let mut store = SweepingStore::default();
    let neighbour = Scene { params: vec![("letters".into(), serde_json::json!("N"))], looping: None, data: None };
    store.set_scene(&SetScene { moment: moment("17"), scene: neighbour.clone() }).unwrap();
    let refused = check_store(&mut store, &moment("18"), &moment("17"), &slide());
    assert_eq!(refused, Err(Breach::Disturbed { before: Some(neighbour), after: None }));
}
```

These three tests pin the three rules. A new sequence store uses the same
check in its own tests. It needs only two of its own moments.
