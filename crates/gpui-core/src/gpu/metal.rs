//! Audited Metal object bridge between wgpu's objc2 and GPUI's metal-rs.
//! No pointer is exposed to JavaScript. Both wrappers retain the same `MTLTexture`.
#![allow(
    unsafe_code,
    reason = "Metal's two Rust wrappers require a retained Objective-C pointer conversion"
)]
use foreign_types::ForeignTypeRef;
use std::sync::Arc;

pub(super) fn share_texture(
    texture: &wgpu::Texture,
) -> super::Result<Arc<dyn std::any::Any + Send + Sync>> {
    // SAFETY: the guard holds the wgpu allocation alive. raw_handle is an actual
    // MTLTexture object, and to_owned sends retain before the guard is dropped.
    let guard = unsafe { texture.as_hal::<wgpu::hal::api::Metal>() }
        .ok_or("The wgpu device does not use Metal")?;
    let pointer = std::ptr::from_ref(guard.raw_handle())
        .cast_mut()
        .cast::<metal::MTLTexture>();
    let texture = unsafe { metal::TextureRef::from_ptr(pointer) }.to_owned();
    Ok(Arc::new(texture))
}
