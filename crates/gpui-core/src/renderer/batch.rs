//! Atomic decoding, structural validation, and application of Vue mutation batches.

#![allow(
    clippy::cast_precision_loss,
    reason = "validated element IDs are returned through JavaScript Number values"
)]

use std::sync::Arc;

use super::raw_element_id;
use crate::retained_tree::{RetainedTree, StyleTable};
use crate::style::StyleDesc;

/// Parsed batch operation — typed enum for atomic validation.
/// All ops are parsed and validated BEFORE any tree mutation occurs.
/// This prevents partial application on malformed batches.
pub(crate) enum BatchOp<'a> {
    CreateElement {
        id: u64,
        element_type: String,
    },
    DestroyElement {
        id: u64,
    },
    AppendChild {
        parent_id: u64,
        child_id: u64,
    },
    RemoveChild {
        parent_id: u64,
        child_id: u64,
    },
    InsertBefore {
        parent_id: u64,
        child_id: u64,
        before_id: u64,
    },
    /// The payload stays as raw JSON until apply time.
    ///
    /// Two reasons. A parsed `StyleDesc` is ~1.4 KB, and a `Vec<BatchOp>` is as
    /// wide as its widest variant, so inlining one made a 220k-op mount reserve
    /// over 300 MB before it parsed a single op. And the tree hash-conses
    /// styles by content, so it needs the bytes: hashing ~110 bytes is far
    /// cheaper than building 80 `Option` fields and throwing 99.8% of them away.
    SetStyle {
        id: u64,
        style: &'a serde_json::value::RawValue,
    },
    SetText {
        id: u64,
        content: String,
    },
    SetEventListener {
        id: u64,
        event_type: String,
        has_handler: bool,
    },
    SetRoot {
        id: u64,
    },
    SetCustomProp {
        id: u64,
        key: String,
        value: serde_json::Value,
    },
}

/// A batch failure. The message names the op index, so it survives the trip
/// back to JS as a plain `Error`.
pub type BatchResult<T> = std::result::Result<T, String>;

/// Decode the batch straight from its JSON bytes into `Vec<BatchOp>`.
///
/// There is deliberately no `Vec<serde_json::Value>` in between. That tree cost
/// a `String` per key and per value, every payload was then deep-cloned out of
/// it, and `from_value` parsed the clone a second time, so one style was
/// allocated three times. A 220k-op mount made 1.5M allocations that way.
///
/// Everything the `Value` version guaranteed still holds, and each one is
/// load-bearing:
///
/// * an unknown opcode is a hard error, not a skipped op. Silently ignoring one
///   would let a JS/Rust version skew desync the tree instead of throwing
/// * ids go through `raw_element_id`, so non-finite, negative, fractional and
///   out-of-safe-range values are still rejected
/// * `hasHandler` is accepted as a bool or a number
/// * errors still name the op index. `serde_json` reports a byte offset, which
///   is useless when you are chasing a desync
pub(crate) fn parse_batch_ops(bytes: &[u8]) -> BatchResult<Vec<BatchOp<'_>>> {
    serde_json::from_slice::<BatchOps>(bytes)
        .map(|batch| batch.0)
        .map_err(|error| format!("Failed to parse batch: {error}"))
}

struct BatchOps<'a>(Vec<BatchOp<'a>>);

impl<'de> serde::Deserialize<'de> for BatchOps<'de> {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        struct OpsVisitor;

        impl<'de> serde::de::Visitor<'de> for OpsVisitor {
            type Value = BatchOps<'de>;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("an array of mutation tuples")
            }

            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> std::result::Result<BatchOps<'de>, A::Error> {
                let mut ops = Vec::with_capacity(seq.size_hint().unwrap_or(64));
                loop {
                    // The index is attached here because this is the only place
                    // that knows it.
                    let index = ops.len();
                    match seq.next_element::<BatchOp<'de>>() {
                        Ok(Some(op)) => ops.push(op),
                        Ok(None) => break,
                        Err(error) => {
                            return Err(serde::de::Error::custom(format!(
                                "Batch op {index}: {error}"
                            )));
                        }
                    }
                }
                Ok(BatchOps(ops))
            }
        }

        deserializer.deserialize_seq(OpsVisitor)
    }
}

/// A string argument, borrowed from the input when the JSON has no escapes.
///
/// The owned copy happens exactly once, on the way into the `BatchOp`. The
/// `Value` path allocated twice: into `Value::String`, then into the op.
struct StrArg<'a>(std::borrow::Cow<'a, str>);

impl<'de> serde::Deserialize<'de> for StrArg<'de> {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        use std::borrow::Cow;
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = StrArg<'de>;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a string")
            }
            fn visit_borrowed_str<E: serde::de::Error>(
                self,
                v: &'de str,
            ) -> std::result::Result<StrArg<'de>, E> {
                Ok(StrArg(Cow::Borrowed(v)))
            }
            fn visit_str<E: serde::de::Error>(
                self,
                v: &str,
            ) -> std::result::Result<StrArg<'de>, E> {
                Ok(StrArg(Cow::Owned(v.to_owned())))
            }
            fn visit_string<E: serde::de::Error>(
                self,
                v: String,
            ) -> std::result::Result<StrArg<'de>, E> {
                Ok(StrArg(Cow::Owned(v)))
            }
        }
        deserializer.deserialize_str(V)
    }
}

/// A legacy `setCustomProp` payload: a JSON string gets decoded, anything else
/// is taken as-is. `setCustomPropValue` skips this and stores the raw value.
struct LegacyPropArg(serde_json::Value);

impl<'de> serde::Deserialize<'de> for LegacyPropArg {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(deserializer)?;
        if let serde_json::Value::String(encoded) = &value {
            return Ok(LegacyPropArg(
                serde_json::from_str(encoded).unwrap_or_else(|_| value.clone()),
            ));
        }
        Ok(LegacyPropArg(value))
    }
}

/// `hasHandler` arrives as a bool from the reconciler and as a non-negative
/// integer from hand-written batches. That is exactly what `as_bool()` then
/// `as_u64()` accepted before, so a negative or fractional number stays an
/// error rather than quietly meaning `true`.
struct BoolArg(bool);

impl<'de> serde::Deserialize<'de> for BoolArg {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        struct V;
        impl serde::de::Visitor<'_> for V {
            type Value = BoolArg;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a boolean or a non-negative integer")
            }
            fn visit_bool<E: serde::de::Error>(self, v: bool) -> std::result::Result<BoolArg, E> {
                Ok(BoolArg(v))
            }
            fn visit_u64<E: serde::de::Error>(self, v: u64) -> std::result::Result<BoolArg, E> {
                Ok(BoolArg(v != 0))
            }
        }
        deserializer.deserialize_any(V)
    }
}

fn next_arg<'de, A, T>(seq: &mut A, what: &str) -> std::result::Result<T, A::Error>
where
    A: serde::de::SeqAccess<'de>,
    T: serde::Deserialize<'de>,
{
    seq.next_element()?
        .ok_or_else(|| serde::de::Error::custom(format!("missing {what}")))
}

/// Read an element id. Ids cross napi as JS numbers, so they are read as `f64`
/// and validated exactly as `batch_id` did.
fn next_id<'de, A: serde::de::SeqAccess<'de>>(
    seq: &mut A,
    what: &str,
) -> std::result::Result<u64, A::Error> {
    let raw: f64 = next_arg(seq, what)?;
    raw_element_id(raw).map_err(serde::de::Error::custom)
}

impl<'de> serde::Deserialize<'de> for BatchOp<'de> {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        struct V;

        impl<'de> serde::de::Visitor<'de> for V {
            type Value = BatchOp<'de>;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a [opcode, ...args] mutation tuple")
            }

            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> std::result::Result<BatchOp<'de>, A::Error> {
                let name: StrArg<'de> = next_arg(&mut seq, "op name")?;
                let op = match name.0.as_ref() {
                    "createElement" => BatchOp::CreateElement {
                        id: next_id(&mut seq, "id")?,
                        element_type: next_arg::<A, StrArg>(&mut seq, "element type")?
                            .0
                            .into_owned(),
                    },
                    "destroyElement" => BatchOp::DestroyElement {
                        id: next_id(&mut seq, "id")?,
                    },
                    "appendChild" => BatchOp::AppendChild {
                        parent_id: next_id(&mut seq, "parent id")?,
                        child_id: next_id(&mut seq, "child id")?,
                    },
                    "removeChild" => BatchOp::RemoveChild {
                        parent_id: next_id(&mut seq, "parent id")?,
                        child_id: next_id(&mut seq, "child id")?,
                    },
                    "insertBefore" => BatchOp::InsertBefore {
                        parent_id: next_id(&mut seq, "parent id")?,
                        child_id: next_id(&mut seq, "child id")?,
                        before_id: next_id(&mut seq, "before id")?,
                    },
                    "setStyle" => BatchOp::SetStyle {
                        id: next_id(&mut seq, "id")?,
                        style: next_arg(&mut seq, "style")?,
                    },
                    "setText" => BatchOp::SetText {
                        id: next_id(&mut seq, "id")?,
                        content: next_arg::<A, StrArg>(&mut seq, "text")?.0.into_owned(),
                    },
                    "setEventListener" => BatchOp::SetEventListener {
                        id: next_id(&mut seq, "id")?,
                        event_type: next_arg::<A, StrArg>(&mut seq, "event type")?
                            .0
                            .into_owned(),
                        has_handler: next_arg::<A, BoolArg>(&mut seq, "hasHandler")?.0,
                    },
                    "setRoot" => BatchOp::SetRoot {
                        id: next_id(&mut seq, "id")?,
                    },
                    "setCustomProp" => BatchOp::SetCustomProp {
                        id: next_id(&mut seq, "id")?,
                        key: next_arg::<A, StrArg>(&mut seq, "prop key")?.0.into_owned(),
                        value: next_arg::<A, LegacyPropArg>(&mut seq, "custom prop value")?.0,
                    },
                    "setCustomPropValue" => BatchOp::SetCustomProp {
                        id: next_id(&mut seq, "id")?,
                        key: next_arg::<A, StrArg>(&mut seq, "prop key")?.0.into_owned(),
                        value: next_arg(&mut seq, "custom prop value")?,
                    },
                    other => {
                        return Err(serde::de::Error::custom(format!(
                            "unknown operation: {other:?}"
                        )));
                    }
                };
                // Trailing arguments are tolerated, as they were when the op was
                // an indexed array.
                while seq.next_element::<serde::de::IgnoredAny>()?.is_some() {}
                Ok(op)
            }
        }

        deserializer.deserialize_seq(V)
    }
}

/// Turn one raw `setStyle` payload into a shared style.
///
/// The reconciler always sends an object. A legacy batch can send the same
/// object as a JSON *string*, so that is unwrapped to the bytes the interner
/// should see. Anything else, `null` included, is handed to `StyleDesc` and
/// rejected there. Doing this here, rather than in the deserializer, keeps the
/// raw bytes available for the content hash.
fn intern_style_payload(
    styles: &mut StyleTable,
    payload: &serde_json::value::RawValue,
) -> BatchResult<Arc<StyleDesc>> {
    // A `RawValue` always holds exactly one complete JSON value, so this is
    // never empty and never a fragment.
    let raw = payload.get().trim();
    if raw.starts_with('"') {
        let encoded: String = serde_json::from_str(raw).map_err(|error| error.to_string())?;
        styles.intern(encoded.as_bytes())
    } else {
        styles.intern(raw.as_bytes())
    }
}

/// Resolve every `setStyle` payload in the batch, in op order.
///
/// This is the last fallible step, so it runs before the apply loop and borrows
/// only the style table. The borrow checker then proves no element was touched
/// when it returns `Err`, which is what makes a batch atomic. An earlier
/// version interned inside the apply loop, so a malformed style at the end of a
/// batch left everything before it applied and then threw.
fn resolve_styles(
    styles: &mut StyleTable,
    ops: &[BatchOp<'_>],
) -> BatchResult<Vec<Arc<StyleDesc>>> {
    let mut resolved = Vec::new();
    for (index, op) in ops.iter().enumerate() {
        if let BatchOp::SetStyle { style, .. } = op {
            let shared = intern_style_payload(styles, style)
                .map_err(|error| format!("Batch op {index} setStyle parse error: {error}"))?;
            resolved.push(shared);
        }
    }
    Ok(resolved)
}

#[derive(Clone)]
struct ShadowNode {
    parent: Option<u64>,
    children: Vec<u64>,
}

/// Minimal relationship-only copy used to validate an entire batch before the
/// retained tree is touched. This preserves the batch's sequential semantics
/// while making cycles a fallible decode-phase concern.
struct BatchStructure {
    nodes: rustc_hash::FxHashMap<u64, ShadowNode>,
}

impl BatchStructure {
    fn from_tree(tree: &RetainedTree) -> Self {
        Self {
            nodes: tree
                .elements
                .iter()
                .map(|(&id, element)| {
                    (
                        id,
                        ShadowNode {
                            parent: element.parent,
                            children: element.children.clone(),
                        },
                    )
                })
                .collect(),
        }
    }

    fn create(&mut self, id: u64) {
        self.nodes.insert(
            id,
            ShadowNode {
                parent: None,
                children: Vec::new(),
            },
        );
    }

    fn destroy(&mut self, id: u64) {
        let parent = self.nodes.get(&id).and_then(|node| node.parent);
        if let Some(parent) = parent.and_then(|parent| self.nodes.get_mut(&parent)) {
            parent.children.retain(|child| *child != id);
        }
        let mut pending = vec![id];
        while let Some(current) = pending.pop() {
            if let Some(node) = self.nodes.remove(&current) {
                pending.extend(node.children);
            }
        }
    }

    fn reparent(
        &mut self,
        parent_id: u64,
        child_id: u64,
        before_id: Option<u64>,
    ) -> BatchResult<()> {
        if parent_id == child_id {
            return Err(format!("element {child_id} cannot be its own parent"));
        }
        if before_id == Some(child_id) {
            return Ok(());
        }

        let mut current = Some(parent_id);
        for _ in 0..=self.nodes.len() {
            let Some(id) = current else {
                break;
            };
            if id == child_id {
                return Err(format!(
                    "reparenting element {child_id} under {parent_id} would create a cycle"
                ));
            }
            current = self.nodes.get(&id).and_then(|node| node.parent);
        }
        if current.is_some() {
            return Err("retained tree already contains a parent cycle".to_string());
        }

        let old_parent = self.nodes.get(&child_id).and_then(|node| node.parent);
        if let Some(old_parent) = old_parent.and_then(|id| self.nodes.get_mut(&id)) {
            old_parent.children.retain(|child| *child != child_id);
        }
        if let Some(child) = self.nodes.get_mut(&child_id) {
            child.parent = Some(parent_id);
        }
        if let Some(parent) = self.nodes.get_mut(&parent_id) {
            let position = before_id
                .and_then(|before| parent.children.iter().position(|child| *child == before))
                .unwrap_or(parent.children.len());
            parent.children.insert(position, child_id);
        }
        Ok(())
    }

    fn remove(&mut self, parent_id: u64, child_id: u64) {
        if let Some(parent) = self.nodes.get_mut(&parent_id) {
            parent.children.retain(|child| *child != child_id);
        }
        if let Some(child) = self.nodes.get_mut(&child_id) {
            child.parent = None;
        }
    }
}

fn validate_batch_structure(tree: &RetainedTree, ops: &[BatchOp<'_>]) -> BatchResult<()> {
    let mut structure = BatchStructure::from_tree(tree);
    for (index, op) in ops.iter().enumerate() {
        let result = match op {
            BatchOp::CreateElement { id, .. } => {
                structure.create(*id);
                Ok(())
            }
            BatchOp::DestroyElement { id } => {
                structure.destroy(*id);
                Ok(())
            }
            BatchOp::AppendChild {
                parent_id,
                child_id,
            } => structure.reparent(*parent_id, *child_id, None),
            BatchOp::RemoveChild {
                parent_id,
                child_id,
            } => {
                structure.remove(*parent_id, *child_id);
                Ok(())
            }
            BatchOp::InsertBefore {
                parent_id,
                child_id,
                before_id,
            } => structure.reparent(*parent_id, *child_id, Some(*before_id)),
            _ => Ok(()),
        };
        result.map_err(|error| format!("Batch op {index} structural error: {error}"))?;
    }
    Ok(())
}

/// Apply a batch of mutation tuples to a `RetainedTree`.
/// Shared between `GpuiRenderer::apply_batch` and `TestGpuiRenderer::apply_batch`.
/// Returns accumulated destroyed IDs (as f64) from all destroyElement ops.
///
/// ATOMIC: the batch is decoded and every style is resolved before a single
/// element is touched. If any op is malformed the tree is left unchanged and an
/// error is returned. Nothing after that point can fail, so JS and Rust cannot
/// desync when a batch is retried.
///
/// Batch format: JSON array of tuples [opcode, ...args].
/// See `GpuiRenderer::apply_batch` for opcode documentation.
///
/// Kept on the production decode path so unit tests and both native renderers
/// exercise the same atomic implementation.
#[cfg(test)]
pub(crate) fn apply_batch_to_tree(tree: &mut RetainedTree, bytes: &[u8]) -> BatchResult<Vec<f64>> {
    let parsed = parse_batch_ops(bytes)?;
    apply_parsed_batch_to_tree(tree, parsed)
}

pub(crate) fn apply_parsed_batch_to_tree(
    tree: &mut RetainedTree,
    parsed: Vec<BatchOp<'_>>,
) -> BatchResult<Vec<f64>> {
    // Validate relationships against a small structural shadow. This
    // catches cycles created across multiple ops while preserving atomicity.
    validate_batch_structure(tree, &parsed)?;

    // Resolve styles. Touches the style table only; a failure here
    // sweeps back out whatever this call interned.
    let styles = resolve_styles(&mut tree.styles, &parsed).inspect_err(|_| tree.styles.sweep())?;
    let mut styles = styles.into_iter();

    // Apply. Cannot fail.
    let mut destroyed_ids: Vec<f64> = Vec::new();
    for batch_op in parsed {
        match batch_op {
            BatchOp::CreateElement { id, element_type } => {
                tree.create_element(id, element_type);
            }
            BatchOp::DestroyElement { id } => {
                let destroyed = tree.destroy_element(id);
                destroyed_ids.extend(destroyed.iter().map(|&id| id as f64));
            }
            BatchOp::AppendChild {
                parent_id,
                child_id,
            } => {
                tree.append_child_unchecked(parent_id, child_id);
            }
            BatchOp::RemoveChild {
                parent_id,
                child_id,
            } => {
                tree.remove_child(parent_id, child_id);
            }
            BatchOp::InsertBefore {
                parent_id,
                child_id,
                before_id,
            } => {
                tree.insert_before_unchecked(parent_id, child_id, before_id);
            }
            BatchOp::SetStyle { id, .. } => {
                let shared = styles.next().expect("one resolved style per setStyle op");
                tree.set_style(id, shared);
            }
            BatchOp::SetText { id, content } => {
                tree.set_text(id, content);
            }
            BatchOp::SetEventListener {
                id,
                event_type,
                has_handler,
            } => {
                tree.set_event_listener(id, event_type, has_handler);
            }
            BatchOp::SetRoot { id } => {
                tree.set_root(id);
            }
            BatchOp::SetCustomProp { id, key, value } => {
                tree.set_custom_prop(id, key, value);
            }
        }
    }

    // Release styles nothing references any more. Without this a dragged
    // element, which produces a distinct style every frame, would grow the
    // table for as long as the app runs. The element count is what catches the
    // opposite case, a batch that destroyed most of the tree.
    let live_elements = tree.elements.len();
    tree.styles.maybe_sweep(live_elements);

    Ok(destroyed_ids)
}
