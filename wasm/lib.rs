//! The engine as a WebAssembly module without imports, for Gscouter: its
//! browser workers run it in place of gcsim's `runSlice`, and its server
//! can run the same module (wazero), so that both give the same results.
//!
//! Strings cross as UTF-8 in the module's memory. The host allocates the
//! config with `gse_alloc`, writes it, and calls `gse_run_slice` or
//! `gse_check`; each returns a pointer to a result: a little-endian u32
//! status (0 ok, 1 error), a u32 length, then that many bytes of UTF-8. The
//! host reads it and gives it back with `gse_free_result`, and frees the
//! config with `gse_free`.
//!
//! - `gse_run_slice(cfg, len, warmup, seed, first, count)`: Gscouter's
//!   `score.RunIterations` of iterations `first` to `first + count - 1`,
//!   as its JSON (`[]score.Iteration`); an error is the message.
//! - `gse_check(cfg, len)`: what the config needs that the engine does
//!   not implement, as a JSON array of strings, empty when the engine can
//!   run it; an error is the config's parse error.

use gse::api;

/// A buffer of `len` bytes in the module's memory for the host to write.
#[unsafe(no_mangle)]
pub extern "C" fn gse_alloc(len: u32) -> *mut u8 {
    let mut v = Vec::<u8>::with_capacity(len as usize);
    let p = v.as_mut_ptr();
    std::mem::forget(v);
    p
}

/// Frees a buffer `gse_alloc` gave.
///
/// # Safety
/// `ptr` and `len` are those of one `gse_alloc` call, freed once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gse_free(ptr: *mut u8, len: u32) {
    drop(unsafe { Vec::from_raw_parts(ptr, 0, len as usize) });
}

/// Frees a result.
///
/// # Safety
/// `ptr` is a result this module returned, freed once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gse_free_result(ptr: *mut u8) {
    let len = u32::from_le_bytes(unsafe { [*ptr.add(4), *ptr.add(5), *ptr.add(6), *ptr.add(7)] }) as usize;
    drop(unsafe { Vec::from_raw_parts(ptr, 0, len + 8) });
}

/// # Safety
/// `cfg` and `len` are a buffer from `gse_alloc` holding UTF-8.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gse_run_slice(cfg: *const u8, len: u32, warmup: f64, seed: i64, first: u32, count: u32) -> *mut u8 {
    let r = unsafe { text(cfg, len) }.and_then(|src| api::run_iterations(src, warmup, seed, first as usize, count as usize));
    result(r.map(|its| api::to_json(&its)))
}

/// # Safety
/// `cfg` and `len` are a buffer from `gse_alloc` holding UTF-8.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gse_check(cfg: *const u8, len: u32) -> *mut u8 {
    let r = unsafe { text(cfg, len) }.and_then(api::unsupported);
    result(r.map(|list| {
        let items: Vec<String> = list.iter().map(|s| api::json_string(s)).collect();
        format!("[{}]", items.join(","))
    }))
}

unsafe fn text<'a>(ptr: *const u8, len: u32) -> Result<&'a str, String> {
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len as usize) };
    std::str::from_utf8(bytes).map_err(|e| format!("config is not UTF-8: {e}"))
}

fn result(r: Result<String, String>) -> *mut u8 {
    let (status, body) = match r {
        Ok(s) => (0u32, s),
        Err(e) => (1u32, e),
    };
    let mut v = Vec::with_capacity(8 + body.len());
    v.extend_from_slice(&status.to_le_bytes());
    v.extend_from_slice(&(body.len() as u32).to_le_bytes());
    v.extend_from_slice(body.as_bytes());
    // free_result rebuilds the Vec with capacity len + 8
    let mut v = v.into_boxed_slice().into_vec();
    let p = v.as_mut_ptr();
    std::mem::forget(v);
    p
}
