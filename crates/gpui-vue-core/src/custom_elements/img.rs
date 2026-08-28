/// Image custom elements for raster images and tintable SVG icons.
///
/// This provides a native `<img>` for gpui-vue apps while keeping the same
/// custom-element prop pipeline (`setCustomProp`/`custom_props`).
use super::{CustomElement, CustomElementFactory, CustomRenderContext};

pub struct ImgFactory;

pub struct SvgFactory;

impl CustomElementFactory for SvgFactory {
    fn element_type(&self) -> &'static str {
        "svg"
    }

    fn create(&self, _id: u64) -> Box<dyn CustomElement> {
        Box::new(SvgElement::default())
    }
}

impl CustomElementFactory for ImgFactory {
    fn element_type(&self) -> &'static str {
        "img"
    }

    fn create(&self, _id: u64) -> Box<dyn CustomElement> {
        Box::new(ImgElement::default())
    }
}

#[derive(Debug, Clone, Default)]
enum ImgObjectFit {
    Fill,
    #[default]
    Contain,
    Cover,
    ScaleDown,
    None,
}

impl ImgObjectFit {
    fn from_str(value: &str) -> Self {
        match value {
            "fill" => Self::Fill,
            "cover" => Self::Cover,
            "scaleDown" => Self::ScaleDown,
            "none" => Self::None,
            _ => Self::Contain,
        }
    }

    fn as_gpui(&self) -> gpui::ObjectFit {
        match self {
            Self::Fill => gpui::ObjectFit::Fill,
            Self::Contain => gpui::ObjectFit::Contain,
            Self::Cover => gpui::ObjectFit::Cover,
            Self::ScaleDown => gpui::ObjectFit::ScaleDown,
            Self::None => gpui::ObjectFit::None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ImgElement {
    src: String,
    object_fit: ImgObjectFit,
}

impl CustomElement for ImgElement {
    fn render(
        &mut self,
        ctx: CustomRenderContext,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<crate::renderer::GpuiView>,
    ) -> gpui::AnyElement {
        use gpui::prelude::*;

        if self.src.trim().is_empty() {
            let fallback = gpui::div().id(super::custom_element_id("__gpui_vue_img", ctx.id));
            let fallback = super::custom_surface(fallback, &ctx)
                .flex()
                .items_center()
                .justify_center()
                .bg(gpui::rgba(0x1f2230ff))
                .border(gpui::px(1.0))
                .border_color(gpui::rgba(0x5d6481ff))
                .text_color(gpui::rgba(0xa4accdff))
                .child(ctx.chrome_text("img: no src", None));

            return fallback.into_any_element();
        }

        let mut el = gpui::img(self.src.clone())
            .object_fit(self.object_fit.as_gpui())
            .with_fallback(|| {
                gpui::div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(gpui::rgba(0x1f2230ff))
                    .border(gpui::px(1.0))
                    .border_color(gpui::rgba(0x5d6481ff))
                    .text_color(gpui::rgba(0xa4accdff))
                    .child(crate::text::chrome_text("img: load failed".into(), None))
                    .into_any_element()
            })
            .id(super::custom_element_id("__gpui_vue_img", ctx.id));

        if let Some(style) = ctx.style {
            el = crate::renderer::apply_interactive_styles(el, style);
        }

        let el = super::wire_standard_events(el, &ctx);
        crate::automation::track_own_bounds(el, ctx.id).into_any_element()
    }

    fn set_prop(&mut self, key: &str, value: serde_json::Value) {
        match key {
            "src" => self.src = value.as_str().unwrap_or("").to_string(),
            "objectFit" => {
                self.object_fit = value
                    .as_str()
                    .map(ImgObjectFit::from_str)
                    .unwrap_or_default();
            }
            _ => {}
        }
    }

    fn supported_props(&self) -> &'static [&'static str] {
        &["src", "objectFit"]
    }

    fn supported_events(&self) -> &'static [&'static str] {
        &["click", "mouseEnter", "mouseLeave"]
    }

    fn destroy(&mut self) {}
}

#[derive(Debug, Clone, Default)]
pub struct SvgElement {
    src: String,
    bytes: Option<std::sync::Arc<[u8]>>,
    source: String,
}

impl SvgElement {
    fn load_src(&mut self, src: String) {
        self.bytes = svg_data_bytes(&src).map(std::sync::Arc::from);
        self.src = src;
    }
}

fn svg_data_bytes(src: &str) -> Option<Vec<u8>> {
    let payload = src.strip_prefix("data:")?;
    let (meta, data) = payload.split_once(',')?;
    if !meta.starts_with("image/svg+xml") {
        return None;
    }
    if meta.split(';').any(|part| part == "base64") {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD.decode(data).ok()
    } else {
        Some(percent_decode(data))
    }
}

fn percent_decode(input: &str) -> Vec<u8> {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && index + 2 < bytes.len()
            && let Ok(value) = u8::from_str_radix(
                std::str::from_utf8(&bytes[index + 1..index + 3]).unwrap_or(""),
                16,
            )
        {
            out.push(value);
            index += 3;
            continue;
        }
        out.push(bytes[index]);
        index += 1;
    }
    out
}

impl CustomElement for SvgElement {
    fn render(
        &mut self,
        ctx: CustomRenderContext,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<crate::renderer::GpuiView>,
    ) -> gpui::AnyElement {
        use gpui::prelude::*;

        if self.source.trim().is_empty() && self.bytes.is_none() && self.src.trim().is_empty() {
            let empty = gpui::div().id(super::custom_element_id("__gpui_vue_svg", ctx.id));
            let empty = super::custom_surface(empty, &ctx);
            return empty.into_any_element();
        }

        let tint = ctx
            .style
            .and_then(|style| style.color.as_deref())
            .and_then(crate::color::parse_color_rgba)
            .unwrap_or_else(|| gpui::rgb(0xe2e2e2));
        let icon = gpui::svg();
        let mut icon = if !self.source.trim().is_empty() {
            icon.data(self.source.as_bytes())
        } else if let Some(bytes) = self.bytes.as_deref() {
            icon.data(bytes)
        } else {
            // GPUI's SvgAsset performs this read on its asset executor and
            // caches the bytes; no filesystem I/O occurs on the frame thread.
            icon.external_path(self.src.clone())
        }
        .flex_none()
        .text_color(tint)
        .id(super::custom_element_id("__gpui_vue_svg", ctx.id));
        if let Some(style) = ctx.style {
            icon = crate::renderer::apply_interactive_styles(icon, style);
        }
        let icon = super::wire_standard_events(icon, &ctx);
        crate::automation::track_own_bounds(icon, ctx.id).into_any_element()
    }

    fn set_prop(&mut self, key: &str, value: serde_json::Value) {
        match key {
            "src" => self.load_src(value.as_str().unwrap_or_default().to_string()),
            "source" => self.source = value.as_str().unwrap_or_default().to_string(),
            _ => {}
        }
    }

    fn supported_props(&self) -> &'static [&'static str] {
        &["src", "source"]
    }

    fn supported_events(&self) -> &'static [&'static str] {
        &["click", "mouseEnter", "mouseLeave"]
    }

    fn destroy(&mut self) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn svg_data_urls_decode_base64_and_percent_encoding() {
        let base64 = "data:image/svg+xml;base64,PHN2Zy8+";
        assert_eq!(
            svg_data_bytes(base64).as_deref(),
            Some(b"<svg/>".as_slice())
        );

        let encoded = "data:image/svg+xml,%3Csvg%20viewBox%3D%220%200%201%201%22/%3E";
        assert_eq!(
            svg_data_bytes(encoded).as_deref(),
            Some(b"<svg viewBox=\"0 0 1 1\"/>".as_slice())
        );
    }

    #[test]
    fn image_source_keeps_urls_raw_for_gpui() {
        let mut image = ImgElement::default();
        image.set_prop(
            "src",
            serde_json::Value::String("https://example.com/image.png".to_string()),
        );
        assert_eq!(image.src, "https://example.com/image.png");
    }
}
