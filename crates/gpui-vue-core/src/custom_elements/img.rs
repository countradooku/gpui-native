/// Image custom elements for raster images and tintable SVG icons.
///
/// This provides a native `<img>` for gpui-vue apps while keeping the same
/// custom-element prop pipeline (`setCustomProp`/`custom_props`).
use super::{CustomElement, CustomElementFactory, CustomRenderContext};

pub struct ImgFactory;

pub struct SvgFactory;

impl CustomElementFactory for SvgFactory {
    fn element_type(&self) -> &str {
        "svg"
    }

    fn create(&self, _id: u64) -> Box<dyn CustomElement> {
        Box::new(SvgElement::default())
    }
}

impl CustomElementFactory for ImgFactory {
    fn element_type(&self) -> &str {
        "img"
    }

    fn create(&self, _id: u64) -> Box<dyn CustomElement> {
        Box::new(ImgElement::default())
    }
}

#[derive(Debug, Clone)]
enum ImgObjectFit {
    Fill,
    Contain,
    Cover,
    ScaleDown,
    None,
}

impl Default for ImgObjectFit {
    fn default() -> Self {
        Self::Contain
    }
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
            let mut fallback = gpui::div()
                .flex()
                .items_center()
                .justify_center()
                .bg(gpui::rgba(0x1f2230ff))
                .border(gpui::px(1.0))
                .border_color(gpui::rgba(0x5d6481ff))
                .text_color(gpui::rgba(0xa4accdff))
                .child("img: no src");

            if let Some(style) = ctx.style {
                fallback = crate::renderer::apply_styles(fallback, style);
            }

            return fallback.into_any_element();
        }

        let src_path = std::path::PathBuf::from(self.src.clone());
        let mut el = gpui::img(src_path)
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
                    .child("img: load failed")
                    .into_any_element()
            });

        if let Some(style) = ctx.style {
            el = crate::renderer::apply_styles(el, style);
        }

        el.into_any_element()
    }

    fn set_prop(&mut self, key: &str, value: serde_json::Value) {
        match key {
            "src" => self.src = value.as_str().unwrap_or("").to_string(),
            "objectFit" => {
                self.object_fit = value
                    .as_str()
                    .map(ImgObjectFit::from_str)
                    .unwrap_or_default()
            }
            _ => {}
        }
    }

    fn supported_props(&self) -> &'static [&'static str] {
        &["src", "objectFit"]
    }

    fn supported_events(&self) -> &'static [&'static str] {
        &[]
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
        self.bytes = svg_bytes(&src).map(std::sync::Arc::from);
        self.src = src;
    }
}

fn svg_bytes(src: &str) -> Option<Vec<u8>> {
    if let Some(payload) = src.strip_prefix("data:") {
        let (meta, data) = payload.split_once(',')?;
        if !meta.starts_with("image/svg+xml") {
            return None;
        }
        return Some(percent_decode(data));
    }
    #[cfg(target_family = "wasm")]
    return None;
    #[cfg(not(target_family = "wasm"))]
    std::fs::read(src).ok()
}

fn percent_decode(input: &str) -> Vec<u8> {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let Ok(value) = u8::from_str_radix(
                std::str::from_utf8(&bytes[index + 1..index + 3]).unwrap_or(""),
                16,
            ) {
                out.push(value);
                index += 3;
                continue;
            }
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

        let bytes = if self.source.trim().is_empty() {
            self.bytes.as_deref()
        } else {
            Some(self.source.as_bytes())
        };
        let Some(bytes) = bytes else {
            let mut empty = gpui::div();
            if let Some(style) = ctx.style {
                empty = crate::renderer::apply_styles(empty, style);
            }
            return empty.into_any_element();
        };

        let tint = ctx
            .style
            .and_then(|style| style.color.as_deref())
            .and_then(crate::color::parse_color_rgba)
            .unwrap_or_else(|| gpui::rgb(0xe2e2e2).into());
        let mut icon = gpui::svg().data(bytes).flex_none().text_color(tint);
        if let Some(style) = ctx.style {
            icon = crate::renderer::apply_styles(icon, style);
        }
        icon.into_any_element()
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
        &[]
    }

    fn destroy(&mut self) {}
}
