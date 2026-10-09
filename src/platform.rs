//! Browser bridge (localStorage, URL query, test telemetry) with native fallbacks.

#[cfg(target_arch = "wasm32")]
mod imp {
    unsafe extern "C" {
        fn dm_storage_get(ptr: *mut u8, cap: u32) -> i32;
        fn dm_storage_set(ptr: *const u8, len: u32);
        fn dm_query(ptr: *mut u8, cap: u32) -> i32;
        fn dm_report(ptr: *const u8, len: u32);
    }
    fn read(f: unsafe extern "C" fn(*mut u8, u32) -> i32) -> Option<String> {
        let mut buf = vec![0u8; 256 * 1024];
        let n = unsafe { f(buf.as_mut_ptr(), buf.len() as u32) };
        if n <= 0 { return None; }
        buf.truncate(n as usize);
        String::from_utf8(buf).ok()
    }
    pub fn load() -> Option<String> { read(dm_storage_get) }
    pub fn save(s: &str) { unsafe { dm_storage_set(s.as_ptr(), s.len() as u32) } }
    pub fn query() -> String { read(dm_query).unwrap_or_default() }
    pub fn report(s: &str) { unsafe { dm_report(s.as_ptr(), s.len() as u32) } }

    #[unsafe(no_mangle)]
    pub extern "C" fn driftmoor_crate_version() -> u32 { 1 }
}

#[cfg(not(target_arch = "wasm32"))]
mod imp {
    const FILE: &str = "driftmoor-save.json";
    pub fn load() -> Option<String> { std::fs::read_to_string(FILE).ok() }
    pub fn save(s: &str) { let _ = std::fs::write(FILE, s); }
    pub fn query() -> String { std::env::var("DM_QUERY").unwrap_or_default() }
    pub fn report(_: &str) {}
}

pub use imp::*;

/// Value of `key` in a `?a=1&b=2` style query string.
pub fn param(q: &str, key: &str) -> Option<String> {
    q.trim_start_matches('?').split('&').find_map(|kv| {
        let mut it = kv.splitn(2, '=');
        (it.next()? == key).then(|| it.next().unwrap_or("").to_string())
    })
}
