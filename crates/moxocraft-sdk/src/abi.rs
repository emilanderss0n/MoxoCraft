//! The code behind the exports [`export_filter!`](crate::export_filter) writes.
//!
//! Every band runs in a fresh WebAssembly instance, so nothing here is ever freed: the instance's
//! whole memory is thrown away after the call.

use crate::{Error, Filter, Format, Geometry, Image, Params};

/// `pc_manifest`: `len << 32 | ptr` of the manifest text.
pub fn manifest(text: &'static str) -> u64 {
    ((text.len() as u64) << 32) | (text.as_ptr() as usize as u64)
}

/// `pc_alloc`: a block of `size` bytes PhotoCraft may write into, 8-byte aligned so it can hold
/// `f32` samples, or 0 when the memory cap is reached.
#[cfg(target_arch = "wasm32")]
pub fn alloc(size: u32) -> u32 {
    let words = (size as usize).div_ceil(8).max(1);
    let mut block: Vec<u64> = Vec::new();
    if block.try_reserve_exact(words).is_err() {
        return 0;
    }
    let ptr = block.as_mut_ptr();
    // Deliberately never freed: see the module comment.
    core::mem::forget(block);
    ptr as usize as u32
}

/// `pc_filter`: checks the raw arguments, then runs [`run`].
///
/// # Safety
/// `buf` must point at `len` writable bytes and `params` at `params_len` readable bytes (PhotoCraft
/// guarantees both: they are blocks it got from `pc_alloc`).
#[cfg(target_arch = "wasm32")]
#[allow(clippy::too_many_arguments)]
pub unsafe fn filter<F: Filter>(
    buf: *mut f32,
    len: u32,
    width: u32,
    height: u32,
    channels: u32,
    format: u32,
    params: *const u8,
    params_len: u32,
) -> i32 {
    if buf.is_null() || params.is_null() || !len.is_multiple_of(4) || !(buf as usize).is_multiple_of(4) {
        return Error::BadInput as i32;
    }
    // SAFETY: see the function's contract; the pointer is non-null and aligned for f32.
    let data = unsafe { core::slice::from_raw_parts_mut(buf, len as usize / 4) };
    // SAFETY: as above.
    let params = unsafe { core::slice::from_raw_parts(params, params_len as usize) };
    run::<F>(data, width as usize, height as usize, channels as usize, format, params)
}

/// Runs filter `F` on a band, returning PhotoCraft's status code (0 = success).
pub fn run<F: Filter>(data: &mut [f32], width: usize, height: usize, channels: usize, format: u32, params: &[u8]) -> i32 {
    match try_run::<F>(data, width, height, channels, format, params) {
        Ok(()) => 0,
        Err(e) => e as i32,
    }
}

fn try_run<F: Filter>(data: &mut [f32], width: usize, height: usize, channels: usize, format: u32, params: &[u8]) -> Result<(), Error> {
    let expected = width.checked_mul(height).and_then(|p| p.checked_mul(channels)).ok_or(Error::BadInput)?;
    if channels == 0 || expected != data.len() {
        return Err(Error::BadInput);
    }
    let params = Params::parse(core::str::from_utf8(params).map_err(|_| Error::BadInput)?)?;
    let geometry = params.geometry().unwrap_or(Geometry { x: 0, y: 0, overlap: 0, canvas_width: width, canvas_height: height });
    let mut image = Image { data, width, height, channels, format: Format::from_code(format), geometry };
    F::apply(&mut image, &params)
}
