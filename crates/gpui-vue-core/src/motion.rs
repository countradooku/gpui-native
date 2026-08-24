//! Native keyframe, tween, and spring tracks resolved outside Vue.

use std::time::{Duration, Instant};

use serde::Deserialize;

use crate::style::{DimensionValue, StyleDesc};

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MotionStyle {
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub opacity: Option<f64>,
    pub top: Option<f64>,
    pub right: Option<f64>,
    pub bottom: Option<f64>,
    pub left: Option<f64>,
    pub border_radius: Option<f64>,
}

impl MotionStyle {
    fn overlay(self, next: Self) -> Self {
        Self {
            width: next.width.or(self.width),
            height: next.height.or(self.height),
            opacity: next.opacity.or(self.opacity),
            top: next.top.or(self.top),
            right: next.right.or(self.right),
            bottom: next.bottom.or(self.bottom),
            left: next.left.or(self.left),
            border_radius: next.border_radius.or(self.border_radius),
        }
    }

    fn interpolate(self, target: Self, progress: f64) -> Self {
        fn value(from: Option<f64>, to: Option<f64>, progress: f64) -> Option<f64> {
            match (from, to) {
                (Some(from), Some(to)) => Some(from + (to - from) * progress),
                (Some(from), None) => Some(from),
                (None, Some(to)) => Some(to),
                (None, None) => None,
            }
        }
        Self {
            width: value(self.width, target.width, progress),
            height: value(self.height, target.height, progress),
            opacity: value(self.opacity, target.opacity, progress),
            top: value(self.top, target.top, progress),
            right: value(self.right, target.right, progress),
            bottom: value(self.bottom, target.bottom, progress),
            left: value(self.left, target.left, progress),
            border_radius: value(self.border_radius, target.border_radius, progress),
        }
    }

    pub(crate) fn apply_to(self, style: &mut StyleDesc) {
        if let Some(value) = self.width {
            style.width = Some(DimensionValue::Pixels(value.max(0.0)));
        }
        if let Some(value) = self.height {
            style.height = Some(DimensionValue::Pixels(value.max(0.0)));
        }
        if let Some(value) = self.opacity {
            style.opacity = Some(value.clamp(0.0, 1.0));
        }
        if let Some(value) = self.top {
            style.top = Some(value);
        }
        if let Some(value) = self.right {
            style.right = Some(value);
        }
        if let Some(value) = self.bottom {
            style.bottom = Some(value);
        }
        if let Some(value) = self.left {
            style.left = Some(value);
        }
        if let Some(value) = self.border_radius {
            style.border_radius = Some(value.max(0.0));
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(untagged)]
enum MotionInitial {
    Disabled(bool),
    Style(MotionStyle),
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(untagged)]
enum MotionEase {
    Name(String),
    CubicBezier([f64; 4]),
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
enum MotionType {
    #[default]
    Tween,
    Spring,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
enum RepeatType {
    #[default]
    Loop,
    Reverse,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct MotionTransition {
    #[serde(rename = "type", default)]
    motion_type: MotionType,
    #[serde(default)]
    duration: Option<f64>,
    #[serde(default)]
    delay: f64,
    #[serde(default = "default_ease")]
    ease: MotionEase,
    #[serde(default = "default_stiffness")]
    stiffness: f64,
    #[serde(default = "default_damping")]
    damping: f64,
    #[serde(default = "default_mass")]
    mass: f64,
    #[serde(default)]
    velocity: f64,
    #[serde(default)]
    repeat: u32,
    #[serde(default)]
    repeat_type: RepeatType,
    #[serde(default)]
    stagger: f64,
    #[serde(default)]
    stagger_index: u32,
}

impl Default for MotionTransition {
    fn default() -> Self {
        Self {
            motion_type: MotionType::Tween,
            duration: None,
            delay: 0.0,
            ease: default_ease(),
            stiffness: default_stiffness(),
            damping: default_damping(),
            mass: default_mass(),
            velocity: 0.0,
            repeat: 0,
            repeat_type: RepeatType::Loop,
            stagger: 0.0,
            stagger_index: 0,
        }
    }
}

fn default_duration() -> f64 {
    0.3
}
fn default_ease() -> MotionEase {
    MotionEase::Name("easeOut".to_string())
}
fn default_stiffness() -> f64 {
    170.0
}
fn default_damping() -> f64 {
    26.0
}
fn default_mass() -> f64 {
    1.0
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct MotionKeyframe {
    #[serde(default)]
    at: Option<f64>,
    value: MotionStyle,
    #[serde(default)]
    ease: Option<MotionEase>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(untagged)]
enum MotionTarget {
    Style(MotionStyle),
    Keyframes(Vec<MotionKeyframe>),
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct MotionDescription {
    #[serde(default)]
    initial: Option<MotionInitial>,
    animate: MotionTarget,
    #[serde(default)]
    transition: MotionTransition,
}

#[derive(Clone, Debug)]
struct ResolvedKeyframe {
    at: f64,
    value: MotionStyle,
    ease: Option<MotionEase>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct MotionFrame {
    pub style: MotionStyle,
    pub active: bool,
}

pub(crate) struct MotionState {
    source: serde_json::Value,
    keyframes: Vec<ResolvedKeyframe>,
    transition: MotionTransition,
    started: Instant,
    valid: bool,
}

impl MotionState {
    pub(crate) fn new(source: &serde_json::Value, now: Instant) -> Result<Self, String> {
        let description = parse_description(source)?;
        Ok(Self {
            source: source.clone(),
            keyframes: resolve_keyframes(&description, None),
            transition: description.transition,
            started: now,
            valid: true,
        })
    }

    pub(crate) fn invalid(source: &serde_json::Value, now: Instant) -> Self {
        Self {
            source: source.clone(),
            keyframes: vec![ResolvedKeyframe {
                at: 0.0,
                value: MotionStyle::default(),
                ease: None,
            }],
            transition: MotionTransition::default(),
            started: now,
            valid: false,
        }
    }

    pub(crate) fn is_valid(&self) -> bool {
        self.valid
    }

    pub(crate) fn sync(&mut self, source: &serde_json::Value, now: Instant) -> Result<(), String> {
        if self.source == *source {
            return Ok(());
        }
        let description = match parse_description(source) {
            Ok(description) => description,
            Err(error) => {
                self.source = source.clone();
                self.valid = false;
                return Err(error);
            }
        };
        let visible = self.valid.then(|| self.frame(now).style);
        self.keyframes = resolve_keyframes(&description, visible);
        self.transition = description.transition;
        self.started = now;
        self.source = source.clone();
        self.valid = true;
        Ok(())
    }

    pub(crate) fn frame(&self, now: Instant) -> MotionFrame {
        let delay = self.transition.delay
            + self.transition.stagger * f64::from(self.transition.stagger_index);
        let elapsed = now.saturating_duration_since(self.started).as_secs_f64() - delay;
        if elapsed <= 0.0 {
            return MotionFrame {
                style: self.keyframes[0].value,
                active: self.is_animated(),
            };
        }
        let (progress, active) = match self.transition.motion_type {
            MotionType::Tween => self.tween_progress(elapsed),
            MotionType::Spring => self.spring_progress(elapsed),
        };
        MotionFrame {
            style: self.style_at(progress),
            active,
        }
    }

    fn tween_progress(&self, elapsed: f64) -> (f64, bool) {
        let duration = self.transition.duration.unwrap_or_else(default_duration);
        if duration == 0.0 {
            return (self.final_progress(), false);
        }
        let cycles = u64::from(self.transition.repeat) + 1;
        let completed = (elapsed / duration).floor().max(0.0) as u64;
        if completed >= cycles {
            return (self.final_progress(), false);
        }
        let mut progress = (elapsed % duration) / duration;
        if self.transition.repeat_type == RepeatType::Reverse && completed % 2 == 1 {
            progress = 1.0 - progress;
        }
        (progress.clamp(0.0, 1.0), self.is_animated())
    }

    fn spring_progress(&self, elapsed: f64) -> (f64, bool) {
        let config = gpui::SpringConfig::new(
            self.transition.stiffness as f32,
            self.transition.damping as f32,
            self.transition.mass as f32,
        );
        let state = config.step(
            gpui::SpringState {
                position: 0.0,
                velocity: self.transition.velocity as f32,
            },
            1.0,
            elapsed.min(10.0) as f32,
        );
        let deadline = self
            .transition
            .duration
            .is_some_and(|duration| elapsed >= duration)
            || elapsed >= 10.0;
        let active = !deadline && !config.is_settled(state, 1.0, 0.001);
        (
            if active {
                f64::from(state.position)
            } else {
                1.0
            },
            active && self.is_animated(),
        )
    }

    fn final_progress(&self) -> f64 {
        if self.transition.repeat_type == RepeatType::Reverse && self.transition.repeat % 2 == 1 {
            0.0
        } else {
            1.0
        }
    }

    fn is_animated(&self) -> bool {
        self.keyframes
            .windows(2)
            .any(|frames| frames[0].value != frames[1].value)
    }

    fn style_at(&self, progress: f64) -> MotionStyle {
        if self.keyframes.len() == 1 {
            return self.keyframes[0].value;
        }
        if self.keyframes.len() == 2 {
            let from = &self.keyframes[0];
            let to = &self.keyframes[1];
            let raw = (progress - from.at) / (to.at - from.at).max(f64::EPSILON);
            let progress = if self.transition.motion_type == MotionType::Spring {
                raw
            } else {
                ease(
                    raw.clamp(0.0, 1.0),
                    to.ease.as_ref().unwrap_or(&self.transition.ease),
                )
            };
            return from.value.interpolate(to.value, progress);
        }
        let progress = progress.clamp(0.0, 1.0);
        let index = self
            .keyframes
            .windows(2)
            .position(|pair| progress <= pair[1].at)
            .unwrap_or(self.keyframes.len() - 2);
        let from = &self.keyframes[index];
        let to = &self.keyframes[index + 1];
        let raw = (progress - from.at) / (to.at - from.at).max(f64::EPSILON);
        let progress = ease(
            raw.clamp(0.0, 1.0),
            to.ease.as_ref().unwrap_or(&self.transition.ease),
        );
        from.value.interpolate(to.value, progress)
    }
}

fn parse_description(source: &serde_json::Value) -> Result<MotionDescription, String> {
    let description: MotionDescription =
        serde_json::from_value(source.clone()).map_err(|error| error.to_string())?;
    if matches!(description.initial, Some(MotionInitial::Disabled(true))) {
        return Err("motion initial only accepts false or a style object".to_string());
    }
    if let Some(MotionInitial::Style(initial)) = &description.initial {
        validate_style(initial)?;
    }
    match &description.animate {
        MotionTarget::Style(style) => validate_style(style)?,
        MotionTarget::Keyframes(keyframes) => {
            if keyframes.is_empty() {
                return Err("motion keyframes cannot be empty".to_string());
            }
            for frame in keyframes {
                validate_style(&frame.value)?;
                if frame
                    .at
                    .is_some_and(|at| !at.is_finite() || !(0.0..=1.0).contains(&at))
                {
                    return Err("motion keyframe at must be between 0 and 1".to_string());
                }
                if let Some(ease) = &frame.ease {
                    validate_ease(ease)?;
                }
            }
        }
    }
    validate_transition(&description.transition)?;
    Ok(description)
}

fn resolve_keyframes(
    description: &MotionDescription,
    visible: Option<MotionStyle>,
) -> Vec<ResolvedKeyframe> {
    let initial = visible.or(match description.initial {
        Some(MotionInitial::Style(style)) => Some(style),
        _ => None,
    });
    match &description.animate {
        MotionTarget::Style(target) => {
            let from = initial.unwrap_or(*target);
            let target = from.overlay(*target);
            vec![
                ResolvedKeyframe {
                    at: 0.0,
                    value: from,
                    ease: None,
                },
                ResolvedKeyframe {
                    at: 1.0,
                    value: target,
                    ease: None,
                },
            ]
        }
        MotionTarget::Keyframes(frames) => {
            let first = initial.unwrap_or(frames[0].value);
            let mut result = vec![ResolvedKeyframe {
                at: 0.0,
                value: first,
                ease: None,
            }];
            let denominator = frames.len().saturating_sub(1).max(1) as f64;
            let mut previous = first;
            for (index, frame) in frames.iter().enumerate() {
                previous = previous.overlay(frame.value);
                result.push(ResolvedKeyframe {
                    at: frame.at.unwrap_or(index as f64 / denominator),
                    value: previous,
                    ease: frame.ease.clone(),
                });
            }
            result.sort_by(|left, right| left.at.total_cmp(&right.at));
            result.dedup_by(|left, right| {
                if left.at == right.at {
                    *left = right.clone();
                    true
                } else {
                    false
                }
            });
            if result.last().is_some_and(|frame| frame.at < 1.0) {
                result.push(ResolvedKeyframe {
                    at: 1.0,
                    value: previous,
                    ease: None,
                });
            }
            result
        }
    }
}

fn validate_style(style: &MotionStyle) -> Result<(), String> {
    for (name, value) in [
        ("width", style.width),
        ("height", style.height),
        ("opacity", style.opacity),
        ("top", style.top),
        ("right", style.right),
        ("bottom", style.bottom),
        ("left", style.left),
        ("borderRadius", style.border_radius),
    ] {
        if value.is_some_and(|value| !value.is_finite() || value.abs() > f32::MAX as f64) {
            return Err(format!("motion {name} must fit a finite 32-bit float"));
        }
    }
    if style.width.is_some_and(|value| value < 0.0)
        || style.height.is_some_and(|value| value < 0.0)
        || style.border_radius.is_some_and(|value| value < 0.0)
    {
        return Err("motion sizes and borderRadius must be non-negative".to_string());
    }
    if style
        .opacity
        .is_some_and(|value| !(0.0..=1.0).contains(&value))
    {
        return Err("motion opacity must be between 0 and 1".to_string());
    }
    Ok(())
}

fn validate_transition(transition: &MotionTransition) -> Result<(), String> {
    for (name, value) in [("delay", transition.delay), ("stagger", transition.stagger)] {
        validate_seconds(value, name)?;
    }
    if let Some(duration) = transition.duration {
        validate_seconds(duration, "duration")?;
    }
    validate_ease(&transition.ease)?;
    for (name, value, zero_ok) in [
        ("stiffness", transition.stiffness, false),
        ("damping", transition.damping, true),
        ("mass", transition.mass, false),
    ] {
        if !value.is_finite() || value < 0.0 || (!zero_ok && value == 0.0) {
            return Err(format!("motion {name} must be a finite positive number"));
        }
    }
    if !transition.velocity.is_finite() {
        return Err("motion velocity must be finite".to_string());
    }
    Ok(())
}

fn validate_seconds(value: f64, name: &str) -> Result<(), String> {
    if !value.is_finite() || value < 0.0 || Duration::try_from_secs_f64(value).is_err() {
        return Err(format!(
            "motion {name} must be a supported finite non-negative number"
        ));
    }
    Ok(())
}

fn validate_ease(ease: &MotionEase) -> Result<(), String> {
    match ease {
        MotionEase::Name(name)
            if matches!(
                name.as_str(),
                "linear" | "ease" | "easeIn" | "easeOut" | "easeInOut"
            ) => {}
        MotionEase::Name(name) => return Err(format!("unknown motion easing: {name}")),
        MotionEase::CubicBezier([x1, y1, x2, y2]) => {
            if ![x1, y1, x2, y2].iter().all(|value| value.is_finite())
                || !(0.0..=1.0).contains(x1)
                || !(0.0..=1.0).contains(x2)
            {
                return Err(
                    "motion cubic bezier values must be finite and x values must be 0..1"
                        .to_string(),
                );
            }
        }
    }
    Ok(())
}

fn ease(progress: f64, ease: &MotionEase) -> f64 {
    let curve = match ease {
        MotionEase::CubicBezier(curve) => *curve,
        MotionEase::Name(name) => match name.as_str() {
            "linear" => return progress,
            "easeIn" => [0.42, 0.0, 1.0, 1.0],
            "easeInOut" => [0.42, 0.0, 0.58, 1.0],
            "ease" => [0.25, 0.1, 0.25, 1.0],
            _ => [0.0, 0.0, 0.58, 1.0],
        },
    };
    cubic_bezier(progress, curve)
}

fn cubic_bezier(x: f64, [x1, y1, x2, y2]: [f64; 4]) -> f64 {
    fn sample(t: f64, a: f64, b: f64) -> f64 {
        let c = 3.0 * a;
        let b = 3.0 * (b - a) - c;
        let a = 1.0 - c - b;
        ((a * t + b) * t + c) * t
    }
    let (mut low, mut high) = (0.0, 1.0);
    for _ in 0..20 {
        let middle = (low + high) / 2.0;
        if sample(middle, x1, x2) < x {
            low = middle;
        } else {
            high = middle;
        }
    }
    sample((low + high) / 2.0, y1, y2).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interpolates_and_retargets_from_the_visible_value() {
        let started = Instant::now();
        let source = serde_json::json!({
            "initial": { "width": 0.0 }, "animate": { "width": 100.0 },
            "transition": { "duration": 1.0, "ease": "linear" }
        });
        let mut state = MotionState::new(&source, started).unwrap();
        assert_eq!(
            state
                .frame(started + Duration::from_millis(500))
                .style
                .width,
            Some(50.0)
        );
        let reversed = serde_json::json!({
            "initial": false, "animate": { "width": 0.0 },
            "transition": { "duration": 1.0, "ease": "linear" }
        });
        state
            .sync(&reversed, started + Duration::from_millis(500))
            .unwrap();
        assert_eq!(
            state.frame(started + Duration::from_secs(1)).style.width,
            Some(25.0)
        );
    }

    #[test]
    fn keyframes_use_segment_offsets() {
        let started = Instant::now();
        let state = MotionState::new(
            &serde_json::json!({
                "animate": [
                    { "at": 0.0, "value": { "left": 0.0 } },
                    { "at": 0.25, "value": { "left": 100.0 } },
                    { "at": 1.0, "value": { "left": 200.0 } }
                ], "transition": { "duration": 1.0, "ease": "linear" }
            }),
            started,
        )
        .unwrap();
        assert_eq!(
            state.frame(started + Duration::from_millis(250)).style.left,
            Some(100.0)
        );
        assert_eq!(
            state.frame(started + Duration::from_millis(625)).style.left,
            Some(150.0)
        );
    }

    #[test]
    fn spring_settles_at_the_exact_target() {
        let started = Instant::now();
        let state = MotionState::new(
            &serde_json::json!({
                "initial": { "left": 0.0 }, "animate": { "left": 100.0 },
                "transition": { "type": "spring", "stiffness": 170, "damping": 26 }
            }),
            started,
        )
        .unwrap();
        assert!(state.frame(started + Duration::from_millis(100)).active);
        let done = state.frame(started + Duration::from_secs(3));
        assert!(!done.active);
        assert_eq!(done.style.left, Some(100.0));
    }

    #[test]
    fn stagger_delays_start() {
        let started = Instant::now();
        let state = MotionState::new(
            &serde_json::json!({
                "initial": { "opacity": 0.0 }, "animate": { "opacity": 1.0 },
                "transition": { "duration": 1.0, "stagger": 0.2, "staggerIndex": 2 }
            }),
            started,
        )
        .unwrap();
        assert_eq!(
            state
                .frame(started + Duration::from_millis(300))
                .style
                .opacity,
            Some(0.0)
        );
    }

    #[test]
    fn disabled_initial_starts_at_target_and_invalid_values_fail() {
        let started = Instant::now();
        let state = MotionState::new(
            &serde_json::json!({
                "initial": false, "animate": { "opacity": 1.0 }
            }),
            started,
        )
        .unwrap();
        assert!(!state.frame(started).active);
        assert!(MotionState::new(
            &serde_json::json!({
                "initial": true, "animate": { "width": 1.0 }
            }),
            started
        )
        .is_err());
        assert!(MotionState::new(
            &serde_json::json!({
                "animate": { "opacity": 2.0 }
            }),
            started
        )
        .is_err());
    }
}
