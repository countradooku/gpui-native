use std::cell::RefCell;
use std::rc::Rc;

use js_sys::Float32Array;
use wasm_bindgen::prelude::*;

use super::{
    AudioBufferState, EventCallback, GpuiRenderer, TimelineState, WindowInsets, WindowOptions,
    WindowSize,
};
use crate::element_tree::EventPayload;

fn js_error(error: impl std::fmt::Display) -> JsValue {
    js_sys::Error::new(&error.to_string()).into()
}

fn json<T: serde::Serialize>(value: &T) -> Result<String, JsValue> {
    serde_json::to_string(value).map_err(js_error)
}

/// Browser-facing wrapper around the same retained renderer used by N-API.
/// Events are queued in Rust and drained from requestAnimationFrame in JS so
/// no JavaScript function has to cross GPUI's Send + Sync callback boundary.
#[wasm_bindgen]
pub struct WebGpuiRenderer {
    inner: GpuiRenderer,
    events: Rc<RefCell<Vec<EventPayload>>>,
}

#[wasm_bindgen]
impl WebGpuiRenderer {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        let events = Rc::new(RefCell::new(Vec::new()));
        let callback_events = events.clone();
        let callback: EventCallback = Rc::new(move |event| {
            callback_events.borrow_mut().push(event);
        });
        Self {
            inner: GpuiRenderer::new(Some(callback)),
            events,
        }
    }

    #[wasm_bindgen(js_name = createCanvasSource)]
    pub fn create_canvas_source(&self) -> Result<u32, JsValue> {
        self.inner.create_canvas_source().map_err(js_error)
    }

    #[wasm_bindgen(js_name = presentCanvasFrame)]
    #[allow(
        clippy::too_many_arguments,
        reason = "flat binary frame signature shared by N-API and Wasm avoids JSON metadata in the presentation path"
    )]
    pub fn present_canvas_frame(
        &self,
        id: u32,
        width: u32,
        height: u32,
        stride: u32,
        pixels: &[u8],
        bgra: bool,
        opaque: bool,
    ) -> Result<(), JsValue> {
        self.inner
            .present_canvas_slice(id, width, height, stride, pixels, bgra, opaque)
            .map_err(js_error)
    }

    #[wasm_bindgen(js_name = destroyCanvasSource)]
    pub fn destroy_canvas_source(&self, id: u32) -> Result<(), JsValue> {
        self.inner.destroy_canvas_source(id).map_err(js_error)
    }

    pub fn init(&self, options_json: &str) -> Result<(), JsValue> {
        let options = serde_json::from_str::<WindowOptions>(options_json).map_err(js_error)?;
        self.inner.init(Some(options)).map_err(js_error)
    }

    #[wasm_bindgen(js_name = applyBatch)]
    pub fn apply_batch(&self, mutations_json: String) -> Result<Box<[f64]>, JsValue> {
        self.inner
            .apply_batch(mutations_json)
            .map(Vec::into_boxed_slice)
            .map_err(js_error)
    }

    #[wasm_bindgen(js_name = commitMutations)]
    pub fn commit_mutations(&self) -> Result<(), JsValue> {
        self.inner.commit_mutations().map_err(js_error)
    }

    #[wasm_bindgen(js_name = getCustomProp)]
    pub fn get_custom_prop(&self, id: f64, key: String) -> Result<Option<String>, JsValue> {
        self.inner.get_custom_prop(id, key).map_err(js_error)
    }

    #[wasm_bindgen(js_name = isInitialized)]
    pub fn is_initialized(&self) -> bool {
        self.inner.is_initialized()
    }

    #[wasm_bindgen(js_name = requiresTick)]
    pub fn requires_tick(&self) -> bool {
        self.inner.requires_tick()
    }

    #[wasm_bindgen(js_name = supportsWindowEvents)]
    pub fn supports_window_events(&self) -> bool {
        self.inner.supports_window_events()
    }

    pub fn tick(&self) -> Result<bool, JsValue> {
        self.inner.tick().map_err(js_error)
    }

    pub fn close(&self) -> Result<(), JsValue> {
        self.inner.close().map_err(js_error)
    }

    #[wasm_bindgen(js_name = getWindowSizeJson)]
    pub fn get_window_size_json(&self) -> Result<String, JsValue> {
        let state: WindowSize = self.inner.get_window_size().map_err(js_error)?;
        json(&state)
    }

    #[wasm_bindgen(js_name = getWindowInsetsJson)]
    pub fn get_window_insets_json(&self) -> Result<String, JsValue> {
        let insets: WindowInsets = self.inner.get_window_insets().map_err(js_error)?;
        json(&insets)
    }

    #[wasm_bindgen(js_name = activateWindow)]
    pub fn activate_window(&self) -> Result<(), JsValue> {
        self.inner.activate_window().map_err(js_error)
    }

    #[wasm_bindgen(js_name = setWindowTitle)]
    pub fn set_window_title(&self, title: String) -> Result<(), JsValue> {
        self.inner.set_window_title(title).map_err(js_error)
    }

    #[wasm_bindgen(js_name = focusElement)]
    pub fn focus_element(&self, element_id: f64) -> Result<(), JsValue> {
        self.inner.focus_element(element_id).map_err(js_error)
    }

    #[wasm_bindgen(js_name = focusNext)]
    pub fn focus_next(&self) -> Result<(), JsValue> {
        self.inner.focus_next().map_err(js_error)
    }

    #[wasm_bindgen(js_name = focusPrevious)]
    pub fn focus_previous(&self) -> Result<(), JsValue> {
        self.inner.focus_previous().map_err(js_error)
    }

    #[wasm_bindgen(js_name = setWindowKeyEvents)]
    pub fn set_window_key_events(
        &self,
        key_down: bool,
        key_up: bool,
        event_id: f64,
    ) -> Result<(), JsValue> {
        self.inner
            .set_window_key_events(key_down, key_up, event_id)
            .map_err(js_error)
    }

    pub fn blur(&self) -> Result<(), JsValue> {
        self.inner.blur().map_err(js_error)
    }

    #[wasm_bindgen(js_name = getSelectedText)]
    pub fn get_selected_text(&self) -> Option<String> {
        self.inner.get_selected_text()
    }

    #[wasm_bindgen(js_name = clearSelection)]
    pub fn clear_selection(&self) -> Result<(), JsValue> {
        self.inner.clear_selection().map_err(js_error)
    }

    #[wasm_bindgen(js_name = scrollTo)]
    pub fn scroll_to(&self, element_id: f64, x: f64, y: f64) -> Result<(), JsValue> {
        self.inner.scroll_to(element_id, x, y).map_err(js_error)
    }

    #[wasm_bindgen(js_name = scrollToItem)]
    pub fn scroll_to_item(
        &self,
        element_id: f64,
        index: f64,
        offset_in_item: Option<f64>,
    ) -> Result<(), JsValue> {
        self.inner
            .scroll_to_item(element_id, index, offset_in_item)
            .map_err(js_error)
    }

    #[wasm_bindgen(js_name = getListScrollTopJson)]
    pub fn get_list_scroll_top_json(&self, element_id: f64) -> Result<String, JsValue> {
        json(
            &self
                .inner
                .get_list_scroll_top(element_id)
                .map_err(js_error)?,
        )
    }

    #[wasm_bindgen(js_name = getScrollOffsetJson)]
    pub fn get_scroll_offset_json(&self, element_id: f64) -> Result<String, JsValue> {
        json(&self.inner.get_scroll_offset(element_id).map_err(js_error)?)
    }

    #[wasm_bindgen(js_name = getAutomationTree)]
    pub fn get_automation_tree(&self) -> Result<String, JsValue> {
        self.inner.get_automation_tree().map_err(js_error)
    }

    #[wasm_bindgen(js_name = getElementBoundsJson)]
    pub fn get_element_bounds_json(&self, element_id: f64) -> Result<String, JsValue> {
        json(
            &self
                .inner
                .get_element_bounds(element_id)
                .map_err(js_error)?,
        )
    }

    #[wasm_bindgen(js_name = getAllTextJson)]
    pub fn get_all_text_json(&self) -> Result<String, JsValue> {
        json(&self.inner.get_all_text())
    }

    #[wasm_bindgen(js_name = getPaintedTextJson)]
    pub fn get_painted_text_json(&self) -> Result<String, JsValue> {
        json(&self.inner.get_painted_text())
    }

    #[wasm_bindgen(js_name = getPaintedHighlightsJson)]
    pub fn get_painted_highlights_json(&self) -> Result<String, JsValue> {
        json(&self.inner.get_painted_highlights())
    }

    #[wasm_bindgen(js_name = simulateKeystrokes)]
    pub fn simulate_keystrokes(&self, keystrokes: String) -> Result<(), JsValue> {
        self.inner.simulate_keystrokes(keystrokes).map_err(js_error)
    }

    #[wasm_bindgen(js_name = simulateKeyDown)]
    pub fn simulate_key_down(
        &self,
        keystroke: String,
        is_held: Option<bool>,
    ) -> Result<(), JsValue> {
        self.inner
            .simulate_key_down(keystroke, is_held)
            .map_err(js_error)
    }

    #[wasm_bindgen(js_name = simulateKeyUp)]
    pub fn simulate_key_up(&self, keystroke: String) -> Result<(), JsValue> {
        self.inner.simulate_key_up(keystroke).map_err(js_error)
    }

    #[wasm_bindgen(js_name = simulateClick)]
    pub fn simulate_click(
        &self,
        x: f64,
        y: f64,
        button: Option<u32>,
        modifiers: Option<String>,
    ) -> Result<(), JsValue> {
        self.inner
            .simulate_click(x, y, button, modifiers)
            .map_err(js_error)
    }

    #[wasm_bindgen(js_name = simulateMouseDown)]
    pub fn simulate_mouse_down(
        &self,
        x: f64,
        y: f64,
        button: Option<u32>,
        modifiers: Option<String>,
    ) -> Result<(), JsValue> {
        self.inner
            .simulate_mouse_down(x, y, button, modifiers)
            .map_err(js_error)
    }

    #[wasm_bindgen(js_name = simulateMouseUp)]
    pub fn simulate_mouse_up(
        &self,
        x: f64,
        y: f64,
        button: Option<u32>,
        modifiers: Option<String>,
    ) -> Result<(), JsValue> {
        self.inner
            .simulate_mouse_up(x, y, button, modifiers)
            .map_err(js_error)
    }

    #[wasm_bindgen(js_name = simulateMouseMove)]
    pub fn simulate_mouse_move(
        &self,
        x: f64,
        y: f64,
        pressed_button: Option<u32>,
        modifiers: Option<String>,
    ) -> Result<(), JsValue> {
        self.inner
            .simulate_mouse_move(x, y, pressed_button, modifiers)
            .map_err(js_error)
    }

    #[wasm_bindgen(js_name = simulateScrollWheel)]
    pub fn simulate_scroll_wheel(
        &self,
        x: f64,
        y: f64,
        delta_x: f64,
        delta_y: f64,
        modifiers: Option<String>,
    ) -> Result<(), JsValue> {
        self.inner
            .simulate_scroll_wheel(x, y, delta_x, delta_y, modifiers)
            .map_err(js_error)
    }

    #[wasm_bindgen(js_name = clockPause)]
    pub fn clock_pause(&self) -> Result<f64, JsValue> {
        self.inner.clock_pause().map_err(js_error)
    }

    #[wasm_bindgen(js_name = clockSet)]
    pub fn clock_set(&self, now_ms: f64) -> Result<f64, JsValue> {
        self.inner.clock_set(now_ms).map_err(js_error)
    }

    #[wasm_bindgen(js_name = clockFastForward)]
    pub fn clock_fast_forward(&self, delta_ms: f64) -> Result<f64, JsValue> {
        self.inner.clock_fast_forward(delta_ms).map_err(js_error)
    }

    #[wasm_bindgen(js_name = clockResume)]
    pub fn clock_resume(&self) -> Result<f64, JsValue> {
        self.inner.clock_resume().map_err(js_error)
    }

    #[wasm_bindgen(js_name = setDebugFrameOverlay)]
    pub fn set_debug_frame_overlay(&self, mode: String) -> Result<String, JsValue> {
        self.inner.set_debug_frame_overlay(mode).map_err(js_error)
    }

    #[wasm_bindgen(js_name = cycleDebugFrameOverlay)]
    pub fn cycle_debug_frame_overlay(&self) -> Result<String, JsValue> {
        self.inner.cycle_debug_frame_overlay().map_err(js_error)
    }

    #[wasm_bindgen(js_name = getDebugFrameOverlay)]
    pub fn get_debug_frame_overlay(&self) -> Result<String, JsValue> {
        self.inner.get_debug_frame_overlay().map_err(js_error)
    }

    #[wasm_bindgen(js_name = resetDebugFrameOverlayStats)]
    pub fn reset_debug_frame_overlay_stats(&self) -> Result<(), JsValue> {
        self.inner
            .reset_debug_frame_overlay_stats()
            .map_err(js_error)
    }

    #[wasm_bindgen(js_name = timelineGetStateJson)]
    pub fn timeline_get_state_json(&self) -> Result<String, JsValue> {
        let state: TimelineState = self.inner.timeline_get_state();
        json(&state)
    }

    #[wasm_bindgen(js_name = timelinePlayJson)]
    pub fn timeline_play_json(&self) -> Result<String, JsValue> {
        json(&self.inner.timeline_play().map_err(js_error)?)
    }

    #[wasm_bindgen(js_name = timelinePauseJson)]
    pub fn timeline_pause_json(&self) -> Result<String, JsValue> {
        json(&self.inner.timeline_pause().map_err(js_error)?)
    }

    #[wasm_bindgen(js_name = timelineSeekJson)]
    pub fn timeline_seek_json(&self, current_time_ms: f64) -> Result<String, JsValue> {
        json(
            &self
                .inner
                .timeline_seek(current_time_ms)
                .map_err(js_error)?,
        )
    }

    #[wasm_bindgen(js_name = timelineSetPlaybackRateJson)]
    pub fn timeline_set_playback_rate_json(&self, rate: f64) -> Result<String, JsValue> {
        json(
            &self
                .inner
                .timeline_set_playback_rate(rate)
                .map_err(js_error)?,
        )
    }

    #[wasm_bindgen(js_name = configureAudioJson)]
    pub fn configure_audio_json(
        &self,
        sample_rate: u32,
        channels: u32,
        capacity_frames: Option<u32>,
    ) -> Result<String, JsValue> {
        let state: AudioBufferState = self
            .inner
            .configure_audio(sample_rate, channels, capacity_frames)
            .map_err(js_error)?;
        json(&state)
    }

    #[wasm_bindgen(js_name = enqueueAudioFramesJson)]
    pub fn enqueue_audio_frames_json(&self, samples: Float32Array) -> Result<String, JsValue> {
        json(
            &self
                .inner
                .enqueue_audio_slice(&samples.to_vec())
                .map_err(js_error)?,
        )
    }

    #[wasm_bindgen(js_name = dequeueAudioFrames)]
    pub fn dequeue_audio_frames(&self, max_frames: u32) -> Float32Array {
        Float32Array::from(self.inner.dequeue_audio_vec(max_frames).as_slice())
    }

    #[wasm_bindgen(js_name = clearAudioFramesJson)]
    pub fn clear_audio_frames_json(&self) -> Result<String, JsValue> {
        json(&self.inner.clear_audio_frames())
    }

    #[wasm_bindgen(js_name = getAudioBufferStateJson)]
    pub fn get_audio_buffer_state_json(&self) -> Result<String, JsValue> {
        json(&self.inner.get_audio_buffer_state())
    }

    #[wasm_bindgen(js_name = drainEventsJson)]
    pub fn drain_events_json(&self) -> Result<String, JsValue> {
        let drained = {
            let mut events = self.events.borrow_mut();
            std::mem::take(&mut *events)
        };
        json(&drained)
    }
}
