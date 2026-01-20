///! Path redirect functions

use libc::*;
use std::env;
use std::ffi::{CStr, CString};
use std::sync::{LazyLock, OnceLock};

const REDIRECT: LazyLock<(String, String)> = LazyLock::new(|| {
    (
        env::var("REDIRECT_FROM").unwrap(),
        env::var("REDIRECT_TO").unwrap(),
    )
});

fn get_redirect_path(path: *const c_char) -> Option<CString> {
    if path.is_null() {
        return None;
    }

    let path = unsafe { CStr::from_ptr(path) }.to_str().ok()?;

    if let Some(suffix) = path.strip_prefix(&format!("{}/", REDIRECT.0)) {
        let redirected = format!("{}/{}", REDIRECT.1, suffix);
        Some(CString::new(redirected).ok()?)
    } else {
        None
    }
}

fn original_func<T>(name: &str) -> Option<T> {
    let name = CString::new(name).ok()?;
    let ptr = unsafe { libc::dlsym(libc::RTLD_NEXT, name.as_ptr()) };
    if ptr.is_null() {
        None
    } else {
        Some(unsafe { std::mem::transmute_copy(&ptr) })
    }
}

type OpenFn = unsafe extern "C" fn(*const c_char, c_int, ...) -> c_int;
#[unsafe(no_mangle)]
pub unsafe extern "C" fn open(path: *const c_char, flags: c_int) -> c_int {
    static ORIGINAL: OnceLock<Option<OpenFn>> = OnceLock::new();
    let original = ORIGINAL.get_or_init(|| original_func("open"));

    let redirected = get_redirect_path(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());

    if let Some(f) = original {
        unsafe { f(actual, flags) }
    } else {
        unsafe { libc::open(actual, flags) }
    }
}

type OpenatFn = unsafe extern "C" fn(c_int, *const c_char, c_int, ...) -> c_int;
#[unsafe(no_mangle)]
pub unsafe extern "C" fn openat(dirfd: c_int, path: *const c_char, flags: c_int) -> c_int {
    static ORIGINAL: OnceLock<Option<OpenatFn>> = OnceLock::new();
    let original = ORIGINAL.get_or_init(|| original_func("openat"));

    let redirected = if !path.is_null() {
        let first_char = unsafe { *path };
        if first_char == b'/' as c_char {
            get_redirect_path(path)
        } else {
            None
        }
    } else {
        None
    };
    let actual = redirected.as_ref().map_or(path, |p| p.as_ptr());

    if let Some(f) = original {
        unsafe { f(dirfd, actual, flags) }
    } else {
        unsafe { libc::openat(dirfd, actual, flags) }
    }
}
