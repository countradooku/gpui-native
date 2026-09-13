//! Validate Kit mutations against the final batch state before publishing any changes.

use crate::{renderer::batch::BatchOp, retained_tree::RetainedTree};
use serde_json::Value;
use std::collections::HashMap;

fn props_for(name: &str) -> Option<&'static [&'static str]> {
    super::elements::COMPONENTS
        .iter()
        .chain(super::stateful::COMPONENTS)
        .chain(super::extras::COMPONENTS)
        .chain(super::application::COMPONENTS)
        .chain(super::compound::COMPONENTS)
        .chain(super::table::COMPONENTS)
        .chain(super::lists::COMPONENTS)
        .chain(super::workspace::COMPONENTS)
        .chain(super::charts::COMPONENTS)
        .chain(super::menu::COMPONENTS)
        .chain(super::theme::COMPONENTS)
        .chain(super::collections::COMPONENTS)
        .chain(super::input::COMPONENTS)
        .chain(super::layout::COMPONENTS)
        .chain(super::choice::COMPONENTS)
        .chain(super::overlays::COMPONENTS)
        .find_map(|&(tag, props)| (tag == name).then_some(props))
}

pub(crate) fn validate(tree: &RetainedTree, ops: &[BatchOp<'_>]) -> Result<(), String> {
    let mut changed: HashMap<u64, (String, HashMap<String, Value>)> = HashMap::new();
    for op in ops {
        match op {
            BatchOp::CreateElement { id, element_type } => {
                if element_type.starts_with("kit-") && props_for(element_type).is_none() {
                    return Err(format!("Unknown Kit component {element_type}"));
                }
                changed.insert(*id, (element_type.clone(), HashMap::new()));
            }
            BatchOp::DestroyElement { id } => {
                changed.insert(*id, (String::new(), HashMap::new()));
            }
            BatchOp::SetCustomProp { id, key, value } => {
                let original = tree.elements.get(id);
                let entry = changed.entry(*id).or_insert_with(|| {
                    original.map_or_else(
                        || (String::new(), HashMap::new()),
                        |element| {
                            (
                                element.element_type.clone(),
                                if element.element_type.starts_with("kit-") {
                                    element.custom_props.clone()
                                } else {
                                    HashMap::new()
                                },
                            )
                        },
                    )
                });
                if !entry.0.starts_with("kit-") {
                    continue;
                }
                if value.is_null() {
                    entry.1.remove(key);
                } else {
                    entry.1.insert(key.clone(), value.clone());
                }
            }
            _ => {}
        }
    }
    for (id, (name, props)) in changed {
        if let Some(supported) = props_for(&name) {
            validate_props(&name, &props, supported)
                .map_err(|error| format!("Kit element {id} ({name}): {error}"))?;
        }
    }
    Ok(())
}
#[allow(
    clippy::too_many_lines,
    reason = "one exhaustive schema dispatch for the native Kit catalog"
)]
fn validate_props(
    name: &str,
    props: &HashMap<String, Value>,
    supported: &[&str],
) -> Result<(), String> {
    for (key, value) in props {
        if !supported.contains(&key.as_str()) {
            continue;
        }
        let valid = match key.as_str() {
            "columns" if name == "kit-data-table" => value.as_array().is_some_and(|v| {
                v.iter().all(|col| {
                    col.get("key").is_some_and(Value::is_string)
                        && col
                            .get("width")
                            .is_none_or(|w| numeric(w) && w.as_f64() > Some(0.))
                })
            }),
            "rows" if name == "kit-textarea" => value.as_u64().is_some_and(|v| v > 0),
            "rows" => value
                .as_array()
                .is_some_and(|v| v.iter().all(Value::is_object)),
            "scrollbar" | "jumpButton" | "autohide" | "reverse" | "once" | "filterable"
            | "locked" | "grid" | "xAxis" | "horizontal" | "labelAxis" | "valueAxis" | "shadow"
            | "focusRing" | "looping" | "controls" | "pagination" | "readonly" | "masked"
            | "lineNumbers" | "softWrap" | "required" | "collapsed" | "textCenter"
            | "searchable" | "defaultOpen" | "overlayClosable" | "appearance" | "overlay"
            | "closeButton" | "keyboard" | "resizable" | "stripe" | "checked" | "disabled"
            | "selected" | "loading" | "outline" | "compact" | "dot" | "secondary" | "vertical"
            | "dashed" | "open" | "range" | "cleanable" | "bordered" | "multiple" => {
                value.is_boolean()
            }
            "maxValue" | "nodeWidth" | "outerRadius" | "fontSize" | "monoFontSize"
            | "dialogWidth" | "sheetSize" => numeric(value) && value.as_f64() > Some(0.),
            "placement" => value
                .as_str()
                .is_some_and(|v| matches!(v, "left" | "right" | "top" | "bottom")),
            "anchor" => value.as_str().is_some_and(|v| {
                matches!(
                    v,
                    "top-left"
                        | "top-center"
                        | "top-right"
                        | "bottom-left"
                        | "bottom-center"
                        | "bottom-right"
                )
            }),
            "status" => value.as_str().is_some_and(|v| {
                matches!(
                    v,
                    "pending" | "uploading" | "processing" | "failed" | "complete"
                )
            }),
            "duration" => value.as_u64().is_some_and(|v| v > 0 && v <= 60000),
            "openDelay" | "closeDelay" => value.as_u64().is_some_and(|v| v <= 60000),
            "query" | "locale" | "fontFamily" | "monoFontFamily" | "selectedId" | "language"
            | "searchPlaceholder" | "label" | "title" | "message" | "description" | "tooltip"
            | "src" | "name" | "href" | "placeholder" => value.is_string(),
            "size" => value
                .as_str()
                .is_some_and(|v| matches!(v, "xsmall" | "small" | "medium" | "large")),
            "variant" => value.as_str().is_some_and(|v| match name {
                "kit-bubble" => matches!(v, "default" | "outline" | "ghost"),
                "kit-marker" => matches!(v, "default" | "separator"),
                "kit-tabs" => matches!(v, "pill" | "outline" | "segmented" | "underline"),
                "kit-notification" | "kit-alert" => {
                    matches!(v, "default" | "info" | "success" | "warning" | "error")
                }
                "kit-tag" => matches!(
                    v,
                    "primary" | "secondary" | "danger" | "success" | "warning" | "info"
                ),
                _ => matches!(
                    v,
                    "default"
                        | "primary"
                        | "secondary"
                        | "danger"
                        | "success"
                        | "warning"
                        | "info"
                        | "ghost"
                        | "link"
                ),
            }),
            "value" => match name {
                "kit-input" | "kit-textarea" | "kit-editor" | "kit-number-input"
                | "kit-otp-input" | "kit-select" | "kit-clipboard" => value.is_string(),
                "kit-combobox" => {
                    value.is_string()
                        || value
                            .as_array()
                            .is_some_and(|v| v.iter().all(Value::is_string))
                }
                "kit-calendar" | "kit-date-picker" => valid_date(value),
                "kit-color-picker" => value
                    .as_str()
                    .is_some_and(|v| crate::color::parse_color_rgba(v).is_some()),
                "kit-slider" => {
                    numeric(value)
                        || value
                            .as_array()
                            .is_some_and(|v| v.len() == 2 && v.iter().all(numeric))
                }
                _ => numeric(value),
            },
            "min" | "max" | "step" => numeric(value),
            "firstIndex"
            | "defaultSelectedIndex"
            | "count"
            | "page"
            | "totalPages"
            | "visiblePages"
            | "selectedIndex"
            | "numberOfMonths"
            | "columns" => value.as_u64().is_some(),
            "openIndices" => value
                .as_array()
                .is_some_and(|v| v.iter().all(|v| v.as_u64().is_some())),
            "items" if name == "kit-list" => value
                .as_array()
                .is_some_and(|v| v.iter().all(Value::is_string)),
            "items" if name == "kit-tree" => {
                valid_tree(value, 0, &mut std::collections::HashSet::new())
            }
            "items" => value.as_array().is_some_and(|items| {
                items.iter().all(|item| {
                    item.is_string()
                        || item.as_object().is_some_and(|fields| {
                            fields.iter().all(|(key, value)| match key.as_str() {
                                "description" | "label" | "title" | "content" | "value" => {
                                    value.is_string()
                                }
                                "checked" | "separator" | "disabled" | "required" => {
                                    value.is_boolean()
                                }
                                _ => false,
                            })
                        })
                })
            }),
            "pages" => valid_settings(value),
            "panels" => value.as_array().is_some_and(|values| {
                let mut ids = std::collections::HashSet::new();
                values.iter().all(|v| {
                    v.get("id")
                        .and_then(Value::as_str)
                        .is_some_and(|id| !id.is_empty() && ids.insert(id))
                        && v.get("title").is_none_or(Value::is_string)
                        && v.get("closable").is_none_or(Value::is_boolean)
                        && v.get("placement").is_none_or(|v| {
                            v.as_str().is_some_and(|v| {
                                matches!(v, "center" | "left" | "right" | "bottom")
                            })
                        })
                })
            }),
            "layout" => {
                serde_json::from_value::<gpui_component::dock::DockAreaState>(value.clone()).is_ok()
            }
            "sizes" => value
                .as_array()
                .is_some_and(|values| values.iter().all(|v| numeric(v) && v.as_f64() >= Some(0.))),
            "tokens" => {
                serde_json::from_value::<gpui_component::SemanticThemeConfig>(value.clone()).is_ok()
            }
            "data" => value.as_array().is_some_and(|values| {
                values.iter().all(|v| {
                    v.is_object()
                        && v.get("label").is_some_and(Value::is_string)
                        && ["value", "open", "high", "low", "close"]
                            .iter()
                            .all(|key| v.get(key).is_none_or(numeric))
                        && v.get("color").is_none_or(|c| {
                            c.as_str()
                                .is_some_and(|c| crate::color::parse_color_rgba(c).is_some())
                        })
                })
            }),
            "links" => value.as_array().is_some_and(|values| {
                values.iter().all(|v| {
                    v.get("source").and_then(Value::as_u64).is_some()
                        && v.get("target").and_then(Value::as_u64).is_some()
                        && v.get("value")
                            .is_some_and(|v| numeric(v) && v.as_f64() >= Some(0.))
                })
            }),
            "curve" => value
                .as_str()
                .is_some_and(|v| matches!(v, "linear" | "natural" | "step")),
            "mode" => value
                .as_str()
                .is_some_and(|v| matches!(v, "light" | "dark" | "system")),
            "shadowSize" | "resizeHitSize" | "sidebarWidth" | "radius" | "minSize" | "maxSize"
            | "innerRadius" | "padAngle" | "bodyWidthRatio" | "linkOpacity" | "nodePadding" => {
                numeric(value) && value.as_f64() >= Some(0.)
            }
            "fill" | "stroke" => value
                .as_str()
                .is_some_and(|v| crate::color::parse_color_rgba(v).is_some()),
            "tickMargin" | "gridLevels" | "length" | "groups" => {
                value.as_u64().is_some_and(|v| v > 0 && v <= 128)
            }
            "side" => value
                .as_str()
                .is_some_and(|v| matches!(v, "left" | "right")),
            "alignment" => value.as_str().is_some_and(|v| matches!(v, "start" | "end")),
            "keystroke" => value
                .as_str()
                .is_some_and(|v| gpui::Keystroke::parse(v).is_ok()),
            _ => true,
        };
        if !valid {
            return Err(format!("invalid {key}: {value}"));
        }
    }
    validate_relations(name, props)?;
    if name == "kit-slider" {
        let min = props.get("min").and_then(Value::as_f64).unwrap_or(0.);
        let max = props.get("max").and_then(Value::as_f64).unwrap_or(100.);
        let step = props.get("step").and_then(Value::as_f64).unwrap_or(1.);
        if min >= max || step <= 0. {
            return Err("slider requires min < max and step > 0".into());
        }
        if let Some(value) = props.get("value") {
            let values = value
                .as_array()
                .map_or_else(|| vec![value], |v| v.iter().collect());
            if values
                .iter()
                .any(|v| v.as_f64().is_none_or(|v| v < min || v > max))
            {
                return Err("slider value must be within min and max".into());
            }
            if values.len() == 2 && values[0].as_f64() > values[1].as_f64() {
                return Err("slider range must be ordered".into());
            }
        }
    }
    for key in ["totalPages", "visiblePages", "numberOfMonths", "columns"] {
        if supported.contains(&key) && props.get(key).and_then(Value::as_u64) == Some(0) {
            return Err(format!("{key} must be positive"));
        }
    }
    Ok(())
}
fn numeric(value: &Value) -> bool {
    value
        .as_f64()
        .is_some_and(|v| v.is_finite() && v.abs() <= f64::from(f32::MAX))
}
fn valid_date(value: &Value) -> bool {
    let date = |v: &Value| {
        v.is_null()
            || v.as_str()
                .is_some_and(|v| chrono::NaiveDate::parse_from_str(v, "%Y-%m-%d").is_ok())
    };
    date(value)
        || value
            .as_array()
            .is_some_and(|v| v.len() == 2 && v.iter().all(date))
}

fn valid_tree(value: &Value, depth: usize, ids: &mut std::collections::HashSet<String>) -> bool {
    depth < 128
        && value.as_array().is_some_and(|items| {
            items.iter().all(|item| {
                let Some(id) = item.get("id").and_then(Value::as_str) else {
                    return false;
                };
                !id.is_empty()
                    && ids.insert(id.into())
                    && item.get("label").is_none_or(Value::is_string)
                    && ["expanded", "disabled"]
                        .iter()
                        .all(|key| item.get(key).is_none_or(Value::is_boolean))
                    && item
                        .get("children")
                        .is_none_or(|children| valid_tree(children, depth + 1, ids))
            })
        })
}

#[allow(
    clippy::too_many_lines,
    reason = "keep related native prop and event mappings adjacent"
)]
fn validate_relations(name: &str, props: &HashMap<String, Value>) -> Result<(), String> {
    let number = |key: &str, default| props.get(key).and_then(Value::as_f64).unwrap_or(default);
    if name == "kit-resizable" && number("minSize", 0.) >= number("maxSize", 10000.) {
        return Err("resizable requires minSize < maxSize".into());
    }
    if name == "kit-number-input"
        && (number("min", -f64::MAX) > number("max", f64::MAX) || number("step", 1.) <= 0.)
    {
        return Err("number input requires min <= max and step > 0".into());
    }
    if name == "kit-pie-chart"
        && (number("innerRadius", 0.) >= number("outerRadius", f64::MAX)
            || number("padAngle", 0.) > std::f64::consts::TAU)
    {
        return Err("pie radii must be ordered and padAngle must not exceed a full circle".into());
    }
    for key in ["linkOpacity", "bodyWidthRatio"] {
        if number(key, 0.) > 1. {
            return Err(format!("{key} must be between 0 and 1"));
        }
    }
    for (key, max) in [
        ("numberOfMonths", 24),
        ("columns", 1024),
        ("visiblePages", 1024),
        ("rows", 10000),
    ] {
        if props
            .get(key)
            .and_then(Value::as_u64)
            .is_some_and(|v| v > max)
        {
            return Err(format!("{key} exceeds {max}"));
        }
    }
    if let Some(data) = props.get("data").and_then(Value::as_array) {
        for datum in data {
            let num = |key: &str| datum.get(key).and_then(Value::as_f64);
            if name == "kit-candlestick-chart" {
                if ["open", "high", "low", "close"]
                    .iter()
                    .any(|key| num(key).is_none())
                    || num("low") > num("open")
                    || num("low") > num("close")
                    || num("high") < num("open")
                    || num("high") < num("close")
                {
                    return Err("candlesticks require low <= open/close <= high".into());
                }
            } else if name != "kit-sankey-chart" && num("value").is_none() {
                return Err("chart rows require a numeric value".into());
            }
            if matches!(name, "kit-pie-chart" | "kit-radar-chart") && num("value") < Some(0.) {
                return Err("pie and radar values must be nonnegative".into());
            }
        }
    }
    if name == "kit-sankey-chart" {
        let count = props
            .get("data")
            .and_then(Value::as_array)
            .map_or(0, Vec::len);
        let mut edges = vec![Vec::new(); count];
        let mut incoming = vec![0_usize; count];
        if let Some(links) = props.get("links").and_then(Value::as_array) {
            for link in links {
                let source = link["source"]
                    .as_u64()
                    .and_then(|v| usize::try_from(v).ok())
                    .unwrap_or(usize::MAX);
                let target = link["target"]
                    .as_u64()
                    .and_then(|v| usize::try_from(v).ok())
                    .unwrap_or(usize::MAX);
                if source >= count || target >= count {
                    return Err("Sankey link refers to a missing node".into());
                }
                edges[source].push(target);
                incoming[target] += 1;
            }
        }
        let mut ready = incoming
            .iter()
            .enumerate()
            .filter_map(|(i, n)| (*n == 0).then_some(i))
            .collect::<Vec<_>>();
        let mut visited = 0;
        while let Some(node) = ready.pop() {
            visited += 1;
            for &target in &edges[node] {
                incoming[target] -= 1;
                if incoming[target] == 0 {
                    ready.push(target);
                }
            }
        }
        if visited != count {
            return Err("Sankey links must form an acyclic graph".into());
        }
    }
    if name == "kit-dock"
        && let Some(value) = props.get("layout")
    {
        let saved = serde_json::from_value::<gpui_component::dock::DockAreaState>(value.clone())
            .map_err(|e| e.to_string())?;
        let ids = props
            .get("panels")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|p| p.get("id").and_then(Value::as_str))
            .collect();
        let mut seen = std::collections::HashSet::new();
        if !valid_panel(&saved.center, &ids, &mut seen, 0)
            || [saved.left_dock, saved.right_dock, saved.bottom_dock]
                .into_iter()
                .flatten()
                .any(|dock| {
                    f32::from(dock.size()) < 0. || !valid_panel(dock.panel(), &ids, &mut seen, 0)
                })
        {
            return Err(
                "dock layout contains invalid sizes, indices or duplicate/missing panel references"
                    .into(),
            );
        }
    }
    Ok(())
}
fn valid_panel(
    panel: &gpui_component::dock::PanelState,
    ids: &std::collections::HashSet<&str>,
    seen: &mut std::collections::HashSet<String>,
    depth: usize,
) -> bool {
    use gpui_component::dock::PanelInfo;
    if depth >= 64 {
        return false;
    }
    let valid = match &panel.info {
        PanelInfo::Stack { sizes, axis } => {
            *axis <= 1
                && sizes.len() <= panel.children.len()
                && sizes
                    .iter()
                    .all(|size| f32::from(*size).is_finite() && *size >= gpui::px(0.))
        }
        PanelInfo::Tabs { active_index } => {
            (*active_index < panel.children.len()
                || panel.children.is_empty() && *active_index == 0)
                && panel
                    .children
                    .iter()
                    .all(|p| matches!(p.info, PanelInfo::Panel(_)))
        }
        PanelInfo::Panel(value) => {
            panel.children.is_empty()
                && value
                    .get("id")
                    .and_then(Value::as_str)
                    .is_some_and(|id| ids.contains(id) && seen.insert(id.to_owned()))
        }
    };
    valid
        && panel
            .children
            .iter()
            .all(|child| valid_panel(child, ids, seen, depth + 1))
}

fn valid_settings(value: &Value) -> bool {
    fn text_fields(value: &Value) -> bool {
        value.is_object()
            && ["title", "description"]
                .iter()
                .all(|key| value.get(key).is_none_or(Value::is_string))
    }
    value.as_array().is_some_and(|pages| {
        pages.iter().all(|page| {
            text_fields(page)
                && page
                    .get("groups")
                    .and_then(Value::as_array)
                    .is_some_and(|groups| {
                        groups.iter().all(|group| {
                            text_fields(group)
                                && group.get("items").and_then(Value::as_array).is_some_and(
                                    |items| {
                                        items.iter().all(|item| {
                                            text_fields(item)
                                                && item
                                                    .get("disabled")
                                                    .is_none_or(Value::is_boolean)
                                        })
                                    },
                                )
                        })
                    })
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::renderer::batch::apply_batch_to_tree;
    use serde_json::json;
    #[allow(
        clippy::needless_pass_by_value,
        reason = "test helper consumes temporary JSON fixtures"
    )]
    fn batch(tree: &mut RetainedTree, value: Value) -> Result<Vec<f64>, String> {
        apply_batch_to_tree(tree, &serde_json::to_vec(&value).unwrap())
    }
    #[test]
    fn invalid_kit_props_roll_back_every_operation() {
        let mut tree = RetainedTree::new();
        batch(
            &mut tree,
            json!([
                ["createElement", 1, "kit-slider"],
                ["setCustomPropValue", 1, "value", 20]
            ]),
        )
        .unwrap();
        assert!(
            batch(
                &mut tree,
                json!([
                    ["createElement", 2, "text"],
                    ["setCustomPropValue", 1, "value", 200]
                ])
            )
            .is_err()
        );
        assert!(!tree.elements.contains_key(&2));
        assert_eq!(tree.elements[&1].custom_props["value"], 20);
        batch(
            &mut tree,
            json!([
                ["setCustomPropValue", 1, "value", 200],
                ["setCustomPropValue", 1, "max", 300]
            ]),
        )
        .unwrap();
        assert_eq!(tree.elements[&1].custom_props["value"], 200);
        batch(
            &mut tree,
            json!([
                ["setCustomPropValue", 1, "value", null],
                ["setCustomPropValue", 1, "max", null]
            ]),
        )
        .unwrap();
        assert!(tree.elements[&1].custom_props.is_empty());
    }
    #[test]
    fn invalid_graphs_and_layout_constraints_are_rejected_before_render() {
        for (name, props) in [
            (
                "kit-sankey-chart",
                json!({"data":[{"label":"a"},{"label":"b"}],"links":[{"source":0,"target":1,"value":1},{"source":1,"target":0,"value":1}]}),
            ),
            (
                "kit-sankey-chart",
                json!({"data":[],"links":[{"source":0,"target":1,"value":1}]}),
            ),
            ("kit-resizable", json!({"minSize":200,"maxSize":100})),
            (
                "kit-tree",
                json!({"items":[{"id":"same","children":[{"id":"same"}]}]}),
            ),
            ("kit-calendar", json!({"numberOfMonths":1000000})),
            (
                "kit-line-chart",
                json!({"data":[{"label":"missing value"}]}),
            ),
        ] {
            let props = serde_json::from_value(props).unwrap();
            assert!(
                validate_props(name, &props, props_for(name).unwrap()).is_err(),
                "{name}"
            );
        }
    }
}
