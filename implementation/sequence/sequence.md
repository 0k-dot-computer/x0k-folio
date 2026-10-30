# A moment's scene, wherever its sequence lives

```turtle folio:document
implementation:sequence\/sequence a x0k:Implementation ;
    x0k:status "draft" ;
    x0k:summary "The vocabulary every home of a sequence shares: a scene (the params a view is shown with at a moment, the loop its host runs while the moment rests, and the view's private data), a moment's address, the set-scene op that writes a moment's scene, and the home that applies it through its own write path, held to one contract a check runs against any home." ;
    x0k:concerns "sequence", "scene", "moment", "arrangement", "figures" ;
    x0k:cites architecture:malleable-ui-runtime,
        implementation:folio\/inline-entities,
        implementation:document-editing\/tree-page ;
    x0k:implements architecture:arrangement-document ;
    folio:tangleCrate "crates/x0k-sequence" ;
    folio:tangleRoot "src/lib.rs" .
```

A reader sets a figure up the way a section should show it, lights the parts
that matter, steps it to the right moment, and presses "Show it here". What
they set becomes that section's scene: the next reader who reaches the
section sees the figure that way. The same gesture captures a tour's stop, a
telling's fragment or a cut, and those moments live in different homes: a
section of a document, facts about a moment in a store, a genome. Each home
writes a scene its own way: a document as an edit of the section's scene
block, facts as assertions about the moment, a genome as a genome change
(`x0k:architecture/arrangement-document` §"One op vocabulary, one
transport").

So nothing about capturing a scene knows which home it writes. This crate
is what the homes share: the **scene**, what a view is shown with at one
moment; a moment's **address**; the **`set-scene` op** that writes a
moment's scene; and the **home**, which reads a moment's scene and applies
the op through its own write path. It knows no document, fact store or
genome. A document is the first home
(`x0k:implementation/document-editing/tree-page`, "A section is a moment's
home"), and the page core is where a figure's view is captured; the host's
flight of a figure's position stays in the page core too, until a second
host flies one.

## A scene

A scene is the state a view holds at a moment
(`x0k:architecture/arrangement-document` §"Arrangements in time"): the
value of each param the view declares that the scene sets, and the view's
private data. A param is named by its local name, the one the view's
declaration gives it, and its value is what the host sends in `SetParams`:
a string, a number, a boolean or a list of them. A scene states everything
it shows, so a param it leaves out is one it hands back to the view's
authored value, never one it inherits from the moment before; and it keeps
the order its author stated its params in, because that is the order a
reader reads them in. The data is one JSON text the view reads and every
host passes through unread; a scene without it hands its view no data.

A scene may also state a **loop**: while the moment rests, its host carries
one param of the view linearly from `from` to `to` over `period`
milliseconds and wraps, a stated number of times or without end
(`x0k:architecture/arrangement-document` §"Arrangements in time": a sequence
on a duration axis may loop over a range). The loop is the host's, not the
view's: the view is fed the param each frame as it is fed any other value,
and never learns that a loop moves it, so a frame stays a function of its
params.

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

A document states a scene as a graph block of its section
(`x0k:implementation/folio/inline-entities` §"A section's scene"); folio
reads and writes that block as this `Scene`, so the format is the home's
and the value is everyone's.

## A moment's address

A home holds sequences, and a sequence holds moments. A moment's address
names both, each as a key the home gave it and only the home reads: in a
document, the sequence is the name a figure binds its blocks by and the
moment is the section's heading. Nothing outside the home parses either
key, so a home can key its moments however keeps them stable.

<a name="chunk-address"></a><sub>[`src/lib.rs`](../../crates/x0k-sequence/src/lib.rs) · `#address`</sub>

```rust {#address}
/// Where a moment is: the sequence it is a moment of and the moment, each
/// by the key its home gave it. Only the home reads either key.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MomentRef {
    /// The sequence, by its home's key.
    pub sequence: String,
    /// The moment, by its home's key.
    pub moment: String,
}
```

## The op

`set-scene` is one op of the closed arrangement vocabulary, addressed by
the moment's key. It writes the moment's scene whole: a scene states
everything it shows, so the op replaces what the moment held and never
merges into it.

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

## The home

A home reads a moment's scene and applies `set-scene` through its own write
path, under that path's own undo, attribution and saving. Its contract is
three laws, the same for every home:

- **It reads back what it wrote.** After `set-scene`, the moment's scene is
  exactly the op's scene: its params, their order, its loop and its data.
- **It replaces.** A second `set-scene` on the moment leaves the second
  scene, never the two merged.
- **It writes one moment.** Every other moment holds the scene it held
  before: a scene never inherits by placement, so writing one cannot change
  a neighbour.

<a name="chunk-home"></a><sub>[`src/lib.rs`](../../crates/x0k-sequence/src/lib.rs) · `#home`</sub>

```rust {#home}
/// Where a sequence's moments live: a home reads a moment's scene and
/// applies `set-scene` through its own write path.
pub trait MomentHome {
    /// Why the home refused a read or an op.
    type Error;

    /// The scene `moment` holds of its own; `None` when it holds none.
    fn scene(&self, moment: &MomentRef) -> Result<Option<Scene>, Self::Error>;

    /// Write `op.scene` as `op.moment`'s scene: afterwards the moment holds
    /// exactly that scene, and every other moment what it held before.
    fn set_scene(&mut self, op: &SetScene) -> Result<(), Self::Error>;
}
```

The laws are checked the same way for every home. `check_home` writes a
scene at one moment and reads it back, writes the empty scene over it and
reads that back, writes the scene again, and finds a second moment holding
what it held before all three. A home's tests run it with two of the
home's moments, so a document, a fact store and a genome are held to one
contract. A breach carries the scenes it found, so a failing home's test
names what it wrote and what came back; it is returned only when a home
breaks the contract, so its size costs nothing on the path that passes.

<a name="chunk-check"></a><sub>[`src/lib.rs`](../../crates/x0k-sequence/src/lib.rs) · `#check`</sub>

```rust {#check}
/// How a home broke its contract, as [`check_home`] finds it.
#[derive(Debug, PartialEq)]
pub enum Breach<E> {
    /// The home refused a read or an op.
    Refused(E),
    /// A moment did not read back the scene last written to it.
    Unread { wrote: Scene, read: Option<Scene> },
    /// Writing one moment changed another's scene.
    Disturbed { before: Option<Scene>, after: Option<Scene> },
}

/// Hold `home` to its contract with two of its moments: `scene` written at
/// `here` reads back, the empty scene written over it replaces it, `scene`
/// written again reads back, and `there` holds what it held before.
#[allow(
    clippy::result_large_err,
    reason = "a breach carries the scenes it found and is returned only when a home breaks its contract"
)]
pub fn check_home<H: MomentHome + ?Sized>(home: &mut H, here: &MomentRef, there: &MomentRef, scene: &Scene) -> Result<(), Breach<H::Error>> {
    let before = home.scene(there).map_err(Breach::Refused)?;
    for wrote in [scene.clone(), Scene::default(), scene.clone()] {
        home.set_scene(&SetScene { moment: here.clone(), scene: wrote.clone() }).map_err(Breach::Refused)?;
        let read = home.scene(here).map_err(Breach::Refused)?;
        if read.as_ref() != Some(&wrote) {
            return Err(Breach::Unread { wrote, read });
        }
    }
    let after = home.scene(there).map_err(Breach::Refused)?;
    if after != before {
        return Err(Breach::Disturbed { before, after });
    }
    Ok(())
}
```

## The crate

The crate depends on `serde_json` alone, for a param's value, and builds
for every target a host runs on, a browser's included.

<a name="chunk-lib"></a><sub>[`src/lib.rs`](../../crates/x0k-sequence/src/lib.rs) · `#lib` · assembles [scene](#chunk-scene) · [address](#chunk-address) · [op](#chunk-op) · [home](#chunk-home) · [check](#chunk-check)</sub>

```rust {#lib file="src/lib.rs"}
//! A moment's scene, wherever its sequence lives: the scene, a moment's
//! address, the `set-scene` op and the home that applies it
//! (`x0k:implementation/sequence/sequence`). Knows no document, fact store
//! or genome.

<<scene>>

<<address>>

<<op>>

<<home>>

<<check>>

#[cfg(test)]
mod tests;
```

<a name="chunk-manifest"></a><sub>[`Cargo.toml`](../../crates/x0k-sequence/Cargo.toml) · `#manifest`</sub>

```toml {#manifest file="Cargo.toml"}
[package]
name = "x0k-sequence"
version = "0.1.0"
edition = { workspace = true }
license = "MIT"
description = "A moment's scene wherever its sequence lives: the scene, a moment's address, the set-scene op and the home that applies it"
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

The contract is pinned without a document, against a home that keeps its
scenes in a map. That home keeps the contract. Two homes that each break
one law are refused by the check for that law: one that merges a new scene
into the old one, and one that writes every scene to the first moment it
was given.

<a name="chunk-tests"></a><sub>[`src/tests.rs`](../../crates/x0k-sequence/src/tests.rs) · `#tests`</sub>

```rust {#tests file="src/tests.rs"}
use super::*;
use std::collections::BTreeMap;

/// A home that keeps each moment's scene in a map.
#[derive(Default)]
struct MemoryHome {
    scenes: BTreeMap<MomentRef, Scene>,
}

impl MomentHome for MemoryHome {
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
fn a_home_that_keeps_its_scenes_keeps_the_contract() {
    let mut home = MemoryHome::default();
    let neighbour = Scene { params: vec![("letters".into(), serde_json::json!("N"))], looping: None, data: None };
    home.set_scene(&SetScene { moment: moment("17"), scene: neighbour.clone() }).unwrap();
    assert_eq!(check_home(&mut home, &moment("18"), &moment("17"), &slide()), Ok(()));
    assert_eq!(home.scene(&moment("18")).unwrap(), Some(slide()));
    assert_eq!(home.scene(&moment("17")).unwrap(), Some(neighbour));
    assert_eq!(home.scene(&moment("19")).unwrap(), None, "a moment never written holds no scene");
}

/// A home that merges a new scene's params into the old one's.
#[derive(Default)]
struct MergingHome(MemoryHome);

impl MomentHome for MergingHome {
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

/// A home that writes every scene to the first moment it was given.
#[derive(Default)]
struct StuckHome(MemoryHome, Option<MomentRef>);

impl MomentHome for StuckHome {
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
fn a_home_that_merges_or_writes_elsewhere_breaks_the_contract() {
    let mut merging = MergingHome::default();
    let refused = check_home(&mut merging, &moment("18"), &moment("17"), &slide());
    assert_eq!(refused, Err(Breach::Unread { wrote: Scene::default(), read: Some(slide()) }), "the empty scene must replace");
    let mut stuck = StuckHome::default();
    stuck.set_scene(&SetScene { moment: moment("17"), scene: Scene::default() }).unwrap();
    assert!(matches!(check_home(&mut stuck, &moment("18"), &moment("17"), &slide()), Err(Breach::Unread { .. })), "it never reads back at 18");
}
```

A home that writes the right moment but also changes another is refused as
`Disturbed`: here, one that clears every other moment when it writes one.

<a name="chunk-tests-2"></a><sub>[`src/lib.rs`](../../crates/x0k-sequence/src/lib.rs) · `#tests` · continues</sub>

```rust {#tests}

/// A home that forgets every other moment's scene when it writes one.
#[derive(Default)]
struct SweepingHome(MemoryHome);

impl MomentHome for SweepingHome {
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
fn a_home_that_changes_another_moment_breaks_the_contract() {
    let mut home = SweepingHome::default();
    let neighbour = Scene { params: vec![("letters".into(), serde_json::json!("N"))], looping: None, data: None };
    home.set_scene(&SetScene { moment: moment("17"), scene: neighbour.clone() }).unwrap();
    let refused = check_home(&mut home, &moment("18"), &moment("17"), &slide());
    assert_eq!(refused, Err(Breach::Disturbed { before: Some(neighbour), after: None }));
}
```
