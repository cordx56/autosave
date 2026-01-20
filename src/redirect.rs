///! Path redirect functions for LD_PRELOAD

use libc::*;
use std::env;
use std::ffi::{CStr, CString};
use std::sync::{LazyLock, OnceLock};

static REDIRECT: LazyLock<Option<(String, String)>> = LazyLock::new(|| {
    let from = env::var("REDIRECT_FROM").ok()?;
    let to = env::var("REDIRECT_TO").ok()?;
    Some((from, to))
});

fn get_redirect_path(path: *const c_char) -> Option<CString> {
    if path.is_null() {
        return None;
    }

    let (from, to) = REDIRECT.as_ref()?;
    let path_str = unsafe { CStr::from_ptr(path) }.to_str().ok()?;

    if let Some(suffix) = path_str.strip_prefix(&format!("{}/", from)) {
        let redirected = format!("{}/{}", to, suffix);
        Some(CString::new(redirected).ok()?)
    } else if path_str == from {
        Some(CString::new(to.as_str()).ok()?)
    } else {
        None
    }
}

/// Get redirect path only for absolute paths (used by *at functions)
fn get_redirect_path_absolute(path: *const c_char) -> Option<CString> {
    if path.is_null() {
        return None;
    }
    let first_char = unsafe { *path };
    if first_char == b'/' as c_char {
        get_redirect_path(path)
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

//
// File open functions
//

type OpenFn = unsafe extern "C" fn(*const c_char, c_int, mode_t) -> c_int;
#[unsafe(no_mangle)]
pub unsafe extern "C" fn open(path: *const c_char, flags: c_int, mode: mode_t) -> c_int {
    static ORIGINAL: OnceLock<Option<OpenFn>> = OnceLock::new();
    let original = ORIGINAL.get_or_init(|| original_func("open"));

    let redirected = get_redirect_path(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());

    if let Some(f) = original {
        unsafe { f(actual, flags, mode) }
    } else {
        -1
    }
}

type Open64Fn = unsafe extern "C" fn(*const c_char, c_int, mode_t) -> c_int;
#[unsafe(no_mangle)]
pub unsafe extern "C" fn open64(path: *const c_char, flags: c_int, mode: mode_t) -> c_int {
    static ORIGINAL: OnceLock<Option<Open64Fn>> = OnceLock::new();
    let original = ORIGINAL.get_or_init(|| original_func("open64"));

    let redirected = get_redirect_path(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());

    if let Some(f) = original {
        unsafe { f(actual, flags, mode) }
    } else {
        -1
    }
}

type OpenatFn = unsafe extern "C" fn(c_int, *const c_char, c_int, mode_t) -> c_int;
#[unsafe(no_mangle)]
pub unsafe extern "C" fn openat(dirfd: c_int, path: *const c_char, flags: c_int, mode: mode_t) -> c_int {
    static ORIGINAL: OnceLock<Option<OpenatFn>> = OnceLock::new();
    let original = ORIGINAL.get_or_init(|| original_func("openat"));

    let redirected = get_redirect_path_absolute(path);
    let actual = redirected.as_ref().map_or(path, |p| p.as_ptr());

    if let Some(f) = original {
        unsafe { f(dirfd, actual, flags, mode) }
    } else {
        -1
    }
}

type Openat64Fn = unsafe extern "C" fn(c_int, *const c_char, c_int, mode_t) -> c_int;
#[unsafe(no_mangle)]
pub unsafe extern "C" fn openat64(dirfd: c_int, path: *const c_char, flags: c_int, mode: mode_t) -> c_int {
    static ORIGINAL: OnceLock<Option<Openat64Fn>> = OnceLock::new();
    let original = ORIGINAL.get_or_init(|| original_func("openat64"));

    let redirected = get_redirect_path_absolute(path);
    let actual = redirected.as_ref().map_or(path, |p| p.as_ptr());

    if let Some(f) = original {
        unsafe { f(dirfd, actual, flags, mode) }
    } else {
        -1
    }
}

type CreatFn = unsafe extern "C" fn(*const c_char, mode_t) -> c_int;
#[unsafe(no_mangle)]
pub unsafe extern "C" fn creat(path: *const c_char, mode: mode_t) -> c_int {
    static ORIGINAL: OnceLock<Option<CreatFn>> = OnceLock::new();
    let original = ORIGINAL.get_or_init(|| original_func("creat"));

    let redirected = get_redirect_path(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());

    if let Some(f) = original {
        unsafe { f(actual, mode) }
    } else {
        -1
    }
}

type Creat64Fn = unsafe extern "C" fn(*const c_char, mode_t) -> c_int;
#[unsafe(no_mangle)]
pub unsafe extern "C" fn creat64(path: *const c_char, mode: mode_t) -> c_int {
    static ORIGINAL: OnceLock<Option<Creat64Fn>> = OnceLock::new();
    let original = ORIGINAL.get_or_init(|| original_func("creat64"));

    let redirected = get_redirect_path(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());

    if let Some(f) = original {
        unsafe { f(actual, mode) }
    } else {
        -1
    }
}

//
// Stat functions
//

type StatFn = unsafe extern "C" fn(*const c_char, *mut stat) -> c_int;
#[unsafe(no_mangle)]
pub unsafe extern "C" fn stat(path: *const c_char, buf: *mut stat) -> c_int {
    static ORIGINAL: OnceLock<Option<StatFn>> = OnceLock::new();
    let original = ORIGINAL.get_or_init(|| original_func("stat"));

    let redirected = get_redirect_path(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());

    if let Some(f) = original {
        unsafe { f(actual, buf) }
    } else {
        -1
    }
}

type Stat64Fn = unsafe extern "C" fn(*const c_char, *mut stat64) -> c_int;
#[unsafe(no_mangle)]
pub unsafe extern "C" fn stat64(path: *const c_char, buf: *mut stat64) -> c_int {
    static ORIGINAL: OnceLock<Option<Stat64Fn>> = OnceLock::new();
    let original = ORIGINAL.get_or_init(|| original_func("stat64"));

    let redirected = get_redirect_path(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());

    if let Some(f) = original {
        unsafe { f(actual, buf) }
    } else {
        -1
    }
}

type LstatFn = unsafe extern "C" fn(*const c_char, *mut stat) -> c_int;
#[unsafe(no_mangle)]
pub unsafe extern "C" fn lstat(path: *const c_char, buf: *mut stat) -> c_int {
    static ORIGINAL: OnceLock<Option<LstatFn>> = OnceLock::new();
    let original = ORIGINAL.get_or_init(|| original_func("lstat"));

    let redirected = get_redirect_path(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());

    if let Some(f) = original {
        unsafe { f(actual, buf) }
    } else {
        -1
    }
}

type Lstat64Fn = unsafe extern "C" fn(*const c_char, *mut stat64) -> c_int;
#[unsafe(no_mangle)]
pub unsafe extern "C" fn lstat64(path: *const c_char, buf: *mut stat64) -> c_int {
    static ORIGINAL: OnceLock<Option<Lstat64Fn>> = OnceLock::new();
    let original = ORIGINAL.get_or_init(|| original_func("lstat64"));

    let redirected = get_redirect_path(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());

    if let Some(f) = original {
        unsafe { f(actual, buf) }
    } else {
        -1
    }
}

type FstatatFn = unsafe extern "C" fn(c_int, *const c_char, *mut stat, c_int) -> c_int;
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fstatat(dirfd: c_int, path: *const c_char, buf: *mut stat, flags: c_int) -> c_int {
    static ORIGINAL: OnceLock<Option<FstatatFn>> = OnceLock::new();
    let original = ORIGINAL.get_or_init(|| original_func("fstatat"));

    let redirected = get_redirect_path_absolute(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());

    if let Some(f) = original {
        unsafe { f(dirfd, actual, buf, flags) }
    } else {
        -1
    }
}

type Fstatat64Fn = unsafe extern "C" fn(c_int, *const c_char, *mut stat64, c_int) -> c_int;
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fstatat64(dirfd: c_int, path: *const c_char, buf: *mut stat64, flags: c_int) -> c_int {
    static ORIGINAL: OnceLock<Option<Fstatat64Fn>> = OnceLock::new();
    let original = ORIGINAL.get_or_init(|| original_func("fstatat64"));

    let redirected = get_redirect_path_absolute(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());

    if let Some(f) = original {
        unsafe { f(dirfd, actual, buf, flags) }
    } else {
        -1
    }
}

//
// Access functions
//

type AccessFn = unsafe extern "C" fn(*const c_char, c_int) -> c_int;
#[unsafe(no_mangle)]
pub unsafe extern "C" fn access(path: *const c_char, mode: c_int) -> c_int {
    static ORIGINAL: OnceLock<Option<AccessFn>> = OnceLock::new();
    let original = ORIGINAL.get_or_init(|| original_func("access"));

    let redirected = get_redirect_path(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());

    if let Some(f) = original {
        unsafe { f(actual, mode) }
    } else {
        -1
    }
}

type FaccessatFn = unsafe extern "C" fn(c_int, *const c_char, c_int, c_int) -> c_int;
#[unsafe(no_mangle)]
pub unsafe extern "C" fn faccessat(dirfd: c_int, path: *const c_char, mode: c_int, flags: c_int) -> c_int {
    static ORIGINAL: OnceLock<Option<FaccessatFn>> = OnceLock::new();
    let original = ORIGINAL.get_or_init(|| original_func("faccessat"));

    let redirected = get_redirect_path_absolute(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());

    if let Some(f) = original {
        unsafe { f(dirfd, actual, mode, flags) }
    } else {
        -1
    }
}

//
// Directory functions
//

type OpendirFn = unsafe extern "C" fn(*const c_char) -> *mut DIR;
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opendir(path: *const c_char) -> *mut DIR {
    static ORIGINAL: OnceLock<Option<OpendirFn>> = OnceLock::new();
    let original = ORIGINAL.get_or_init(|| original_func("opendir"));

    let redirected = get_redirect_path(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());

    if let Some(f) = original {
        unsafe { f(actual) }
    } else {
        std::ptr::null_mut()
    }
}

type MkdirFn = unsafe extern "C" fn(*const c_char, mode_t) -> c_int;
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mkdir(path: *const c_char, mode: mode_t) -> c_int {
    static ORIGINAL: OnceLock<Option<MkdirFn>> = OnceLock::new();
    let original = ORIGINAL.get_or_init(|| original_func("mkdir"));

    let redirected = get_redirect_path(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());

    if let Some(f) = original {
        unsafe { f(actual, mode) }
    } else {
        -1
    }
}

type MkdiratFn = unsafe extern "C" fn(c_int, *const c_char, mode_t) -> c_int;
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mkdirat(dirfd: c_int, path: *const c_char, mode: mode_t) -> c_int {
    static ORIGINAL: OnceLock<Option<MkdiratFn>> = OnceLock::new();
    let original = ORIGINAL.get_or_init(|| original_func("mkdirat"));

    let redirected = get_redirect_path_absolute(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());

    if let Some(f) = original {
        unsafe { f(dirfd, actual, mode) }
    } else {
        -1
    }
}

type RmdirFn = unsafe extern "C" fn(*const c_char) -> c_int;
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmdir(path: *const c_char) -> c_int {
    static ORIGINAL: OnceLock<Option<RmdirFn>> = OnceLock::new();
    let original = ORIGINAL.get_or_init(|| original_func("rmdir"));

    let redirected = get_redirect_path(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());

    if let Some(f) = original {
        unsafe { f(actual) }
    } else {
        -1
    }
}

type ChdirFn = unsafe extern "C" fn(*const c_char) -> c_int;
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chdir(path: *const c_char) -> c_int {
    static ORIGINAL: OnceLock<Option<ChdirFn>> = OnceLock::new();
    let original = ORIGINAL.get_or_init(|| original_func("chdir"));

    let redirected = get_redirect_path(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());

    if let Some(f) = original {
        unsafe { f(actual) }
    } else {
        -1
    }
}

//
// File manipulation functions
//

type UnlinkFn = unsafe extern "C" fn(*const c_char) -> c_int;
#[unsafe(no_mangle)]
pub unsafe extern "C" fn unlink(path: *const c_char) -> c_int {
    static ORIGINAL: OnceLock<Option<UnlinkFn>> = OnceLock::new();
    let original = ORIGINAL.get_or_init(|| original_func("unlink"));

    let redirected = get_redirect_path(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());

    if let Some(f) = original {
        unsafe { f(actual) }
    } else {
        -1
    }
}

type UnlinkatFn = unsafe extern "C" fn(c_int, *const c_char, c_int) -> c_int;
#[unsafe(no_mangle)]
pub unsafe extern "C" fn unlinkat(dirfd: c_int, path: *const c_char, flags: c_int) -> c_int {
    static ORIGINAL: OnceLock<Option<UnlinkatFn>> = OnceLock::new();
    let original = ORIGINAL.get_or_init(|| original_func("unlinkat"));

    let redirected = get_redirect_path_absolute(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());

    if let Some(f) = original {
        unsafe { f(dirfd, actual, flags) }
    } else {
        -1
    }
}

type RenameFn = unsafe extern "C" fn(*const c_char, *const c_char) -> c_int;
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rename(oldpath: *const c_char, newpath: *const c_char) -> c_int {
    static ORIGINAL: OnceLock<Option<RenameFn>> = OnceLock::new();
    let original = ORIGINAL.get_or_init(|| original_func("rename"));

    let old_redirected = get_redirect_path(oldpath);
    let new_redirected = get_redirect_path(newpath);
    let actual_old = old_redirected.as_ref().map_or(oldpath, |v| v.as_ptr());
    let actual_new = new_redirected.as_ref().map_or(newpath, |v| v.as_ptr());

    if let Some(f) = original {
        unsafe { f(actual_old, actual_new) }
    } else {
        -1
    }
}

type RenameatFn = unsafe extern "C" fn(c_int, *const c_char, c_int, *const c_char) -> c_int;
#[unsafe(no_mangle)]
pub unsafe extern "C" fn renameat(
    olddirfd: c_int,
    oldpath: *const c_char,
    newdirfd: c_int,
    newpath: *const c_char,
) -> c_int {
    static ORIGINAL: OnceLock<Option<RenameatFn>> = OnceLock::new();
    let original = ORIGINAL.get_or_init(|| original_func("renameat"));

    let old_redirected = get_redirect_path_absolute(oldpath);
    let new_redirected = get_redirect_path_absolute(newpath);
    let actual_old = old_redirected.as_ref().map_or(oldpath, |v| v.as_ptr());
    let actual_new = new_redirected.as_ref().map_or(newpath, |v| v.as_ptr());

    if let Some(f) = original {
        unsafe { f(olddirfd, actual_old, newdirfd, actual_new) }
    } else {
        -1
    }
}

type Renameat2Fn = unsafe extern "C" fn(c_int, *const c_char, c_int, *const c_char, c_uint) -> c_int;
#[unsafe(no_mangle)]
pub unsafe extern "C" fn renameat2(
    olddirfd: c_int,
    oldpath: *const c_char,
    newdirfd: c_int,
    newpath: *const c_char,
    flags: c_uint,
) -> c_int {
    static ORIGINAL: OnceLock<Option<Renameat2Fn>> = OnceLock::new();
    let original = ORIGINAL.get_or_init(|| original_func("renameat2"));

    let old_redirected = get_redirect_path_absolute(oldpath);
    let new_redirected = get_redirect_path_absolute(newpath);
    let actual_old = old_redirected.as_ref().map_or(oldpath, |v| v.as_ptr());
    let actual_new = new_redirected.as_ref().map_or(newpath, |v| v.as_ptr());

    if let Some(f) = original {
        unsafe { f(olddirfd, actual_old, newdirfd, actual_new, flags) }
    } else {
        -1
    }
}

type TruncateFn = unsafe extern "C" fn(*const c_char, off_t) -> c_int;
#[unsafe(no_mangle)]
pub unsafe extern "C" fn truncate(path: *const c_char, length: off_t) -> c_int {
    static ORIGINAL: OnceLock<Option<TruncateFn>> = OnceLock::new();
    let original = ORIGINAL.get_or_init(|| original_func("truncate"));

    let redirected = get_redirect_path(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());

    if let Some(f) = original {
        unsafe { f(actual, length) }
    } else {
        -1
    }
}

type Truncate64Fn = unsafe extern "C" fn(*const c_char, off64_t) -> c_int;
#[unsafe(no_mangle)]
pub unsafe extern "C" fn truncate64(path: *const c_char, length: off64_t) -> c_int {
    static ORIGINAL: OnceLock<Option<Truncate64Fn>> = OnceLock::new();
    let original = ORIGINAL.get_or_init(|| original_func("truncate64"));

    let redirected = get_redirect_path(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());

    if let Some(f) = original {
        unsafe { f(actual, length) }
    } else {
        -1
    }
}

//
// Link functions
//

type LinkFn = unsafe extern "C" fn(*const c_char, *const c_char) -> c_int;
#[unsafe(no_mangle)]
pub unsafe extern "C" fn link(oldpath: *const c_char, newpath: *const c_char) -> c_int {
    static ORIGINAL: OnceLock<Option<LinkFn>> = OnceLock::new();
    let original = ORIGINAL.get_or_init(|| original_func("link"));

    let old_redirected = get_redirect_path(oldpath);
    let new_redirected = get_redirect_path(newpath);
    let actual_old = old_redirected.as_ref().map_or(oldpath, |v| v.as_ptr());
    let actual_new = new_redirected.as_ref().map_or(newpath, |v| v.as_ptr());

    if let Some(f) = original {
        unsafe { f(actual_old, actual_new) }
    } else {
        -1
    }
}

type LinkatFn = unsafe extern "C" fn(c_int, *const c_char, c_int, *const c_char, c_int) -> c_int;
#[unsafe(no_mangle)]
pub unsafe extern "C" fn linkat(
    olddirfd: c_int,
    oldpath: *const c_char,
    newdirfd: c_int,
    newpath: *const c_char,
    flags: c_int,
) -> c_int {
    static ORIGINAL: OnceLock<Option<LinkatFn>> = OnceLock::new();
    let original = ORIGINAL.get_or_init(|| original_func("linkat"));

    let old_redirected = get_redirect_path_absolute(oldpath);
    let new_redirected = get_redirect_path_absolute(newpath);
    let actual_old = old_redirected.as_ref().map_or(oldpath, |v| v.as_ptr());
    let actual_new = new_redirected.as_ref().map_or(newpath, |v| v.as_ptr());

    if let Some(f) = original {
        unsafe { f(olddirfd, actual_old, newdirfd, actual_new, flags) }
    } else {
        -1
    }
}

type SymlinkFn = unsafe extern "C" fn(*const c_char, *const c_char) -> c_int;
#[unsafe(no_mangle)]
pub unsafe extern "C" fn symlink(target: *const c_char, linkpath: *const c_char) -> c_int {
    static ORIGINAL: OnceLock<Option<SymlinkFn>> = OnceLock::new();
    let original = ORIGINAL.get_or_init(|| original_func("symlink"));

    // Note: target is the content of the symlink, so we don't redirect it
    // We only redirect linkpath (where the symlink is created)
    let link_redirected = get_redirect_path(linkpath);
    let actual_link = link_redirected.as_ref().map_or(linkpath, |v| v.as_ptr());

    if let Some(f) = original {
        unsafe { f(target, actual_link) }
    } else {
        -1
    }
}

type SymlinkatFn = unsafe extern "C" fn(*const c_char, c_int, *const c_char) -> c_int;
#[unsafe(no_mangle)]
pub unsafe extern "C" fn symlinkat(target: *const c_char, newdirfd: c_int, linkpath: *const c_char) -> c_int {
    static ORIGINAL: OnceLock<Option<SymlinkatFn>> = OnceLock::new();
    let original = ORIGINAL.get_or_init(|| original_func("symlinkat"));

    let link_redirected = get_redirect_path_absolute(linkpath);
    let actual_link = link_redirected.as_ref().map_or(linkpath, |v| v.as_ptr());

    if let Some(f) = original {
        unsafe { f(target, newdirfd, actual_link) }
    } else {
        -1
    }
}

type ReadlinkFn = unsafe extern "C" fn(*const c_char, *mut c_char, size_t) -> ssize_t;
#[unsafe(no_mangle)]
pub unsafe extern "C" fn readlink(path: *const c_char, buf: *mut c_char, bufsize: size_t) -> ssize_t {
    static ORIGINAL: OnceLock<Option<ReadlinkFn>> = OnceLock::new();
    let original = ORIGINAL.get_or_init(|| original_func("readlink"));

    let redirected = get_redirect_path(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());

    if let Some(f) = original {
        unsafe { f(actual, buf, bufsize) }
    } else {
        -1
    }
}

type ReadlinkatFn = unsafe extern "C" fn(c_int, *const c_char, *mut c_char, size_t) -> ssize_t;
#[unsafe(no_mangle)]
pub unsafe extern "C" fn readlinkat(
    dirfd: c_int,
    path: *const c_char,
    buf: *mut c_char,
    bufsize: size_t,
) -> ssize_t {
    static ORIGINAL: OnceLock<Option<ReadlinkatFn>> = OnceLock::new();
    let original = ORIGINAL.get_or_init(|| original_func("readlinkat"));

    let redirected = get_redirect_path_absolute(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());

    if let Some(f) = original {
        unsafe { f(dirfd, actual, buf, bufsize) }
    } else {
        -1
    }
}

//
// Permission functions
//

type ChmodFn = unsafe extern "C" fn(*const c_char, mode_t) -> c_int;
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chmod(path: *const c_char, mode: mode_t) -> c_int {
    static ORIGINAL: OnceLock<Option<ChmodFn>> = OnceLock::new();
    let original = ORIGINAL.get_or_init(|| original_func("chmod"));

    let redirected = get_redirect_path(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());

    if let Some(f) = original {
        unsafe { f(actual, mode) }
    } else {
        -1
    }
}

type FchmodatFn = unsafe extern "C" fn(c_int, *const c_char, mode_t, c_int) -> c_int;
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fchmodat(dirfd: c_int, path: *const c_char, mode: mode_t, flags: c_int) -> c_int {
    static ORIGINAL: OnceLock<Option<FchmodatFn>> = OnceLock::new();
    let original = ORIGINAL.get_or_init(|| original_func("fchmodat"));

    let redirected = get_redirect_path_absolute(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());

    if let Some(f) = original {
        unsafe { f(dirfd, actual, mode, flags) }
    } else {
        -1
    }
}

type ChownFn = unsafe extern "C" fn(*const c_char, uid_t, gid_t) -> c_int;
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chown(path: *const c_char, owner: uid_t, group: gid_t) -> c_int {
    static ORIGINAL: OnceLock<Option<ChownFn>> = OnceLock::new();
    let original = ORIGINAL.get_or_init(|| original_func("chown"));

    let redirected = get_redirect_path(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());

    if let Some(f) = original {
        unsafe { f(actual, owner, group) }
    } else {
        -1
    }
}

type LchownFn = unsafe extern "C" fn(*const c_char, uid_t, gid_t) -> c_int;
#[unsafe(no_mangle)]
pub unsafe extern "C" fn lchown(path: *const c_char, owner: uid_t, group: gid_t) -> c_int {
    static ORIGINAL: OnceLock<Option<LchownFn>> = OnceLock::new();
    let original = ORIGINAL.get_or_init(|| original_func("lchown"));

    let redirected = get_redirect_path(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());

    if let Some(f) = original {
        unsafe { f(actual, owner, group) }
    } else {
        -1
    }
}

type FchownatFn = unsafe extern "C" fn(c_int, *const c_char, uid_t, gid_t, c_int) -> c_int;
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fchownat(
    dirfd: c_int,
    path: *const c_char,
    owner: uid_t,
    group: gid_t,
    flags: c_int,
) -> c_int {
    static ORIGINAL: OnceLock<Option<FchownatFn>> = OnceLock::new();
    let original = ORIGINAL.get_or_init(|| original_func("fchownat"));

    let redirected = get_redirect_path_absolute(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());

    if let Some(f) = original {
        unsafe { f(dirfd, actual, owner, group, flags) }
    } else {
        -1
    }
}

//
// Other functions
//

type RealpathFn = unsafe extern "C" fn(*const c_char, *mut c_char) -> *mut c_char;
#[unsafe(no_mangle)]
pub unsafe extern "C" fn realpath(path: *const c_char, resolved_path: *mut c_char) -> *mut c_char {
    static ORIGINAL: OnceLock<Option<RealpathFn>> = OnceLock::new();
    let original = ORIGINAL.get_or_init(|| original_func("realpath"));

    let redirected = get_redirect_path(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());

    if let Some(f) = original {
        unsafe { f(actual, resolved_path) }
    } else {
        std::ptr::null_mut()
    }
}

type UtimeFn = unsafe extern "C" fn(*const c_char, *const utimbuf) -> c_int;
#[unsafe(no_mangle)]
pub unsafe extern "C" fn utime(path: *const c_char, times: *const utimbuf) -> c_int {
    static ORIGINAL: OnceLock<Option<UtimeFn>> = OnceLock::new();
    let original = ORIGINAL.get_or_init(|| original_func("utime"));

    let redirected = get_redirect_path(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());

    if let Some(f) = original {
        unsafe { f(actual, times) }
    } else {
        -1
    }
}

type UtimesFn = unsafe extern "C" fn(*const c_char, *const timeval) -> c_int;
#[unsafe(no_mangle)]
pub unsafe extern "C" fn utimes(path: *const c_char, times: *const timeval) -> c_int {
    static ORIGINAL: OnceLock<Option<UtimesFn>> = OnceLock::new();
    let original = ORIGINAL.get_or_init(|| original_func("utimes"));

    let redirected = get_redirect_path(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());

    if let Some(f) = original {
        unsafe { f(actual, times) }
    } else {
        -1
    }
}

type UtimensatFn = unsafe extern "C" fn(c_int, *const c_char, *const timespec, c_int) -> c_int;
#[unsafe(no_mangle)]
pub unsafe extern "C" fn utimensat(
    dirfd: c_int,
    path: *const c_char,
    times: *const timespec,
    flags: c_int,
) -> c_int {
    static ORIGINAL: OnceLock<Option<UtimensatFn>> = OnceLock::new();
    let original = ORIGINAL.get_or_init(|| original_func("utimensat"));

    let redirected = get_redirect_path_absolute(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());

    if let Some(f) = original {
        unsafe { f(dirfd, actual, times, flags) }
    } else {
        -1
    }
}

type FutimesatFn = unsafe extern "C" fn(c_int, *const c_char, *const timeval) -> c_int;
#[unsafe(no_mangle)]
pub unsafe extern "C" fn futimesat(dirfd: c_int, path: *const c_char, times: *const timeval) -> c_int {
    static ORIGINAL: OnceLock<Option<FutimesatFn>> = OnceLock::new();
    let original = ORIGINAL.get_or_init(|| original_func("futimesat"));

    let redirected = get_redirect_path_absolute(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());

    if let Some(f) = original {
        unsafe { f(dirfd, actual, times) }
    } else {
        -1
    }
}

//
// Extended attribute functions
//

type GetxattrFn = unsafe extern "C" fn(*const c_char, *const c_char, *mut c_void, size_t) -> ssize_t;
#[unsafe(no_mangle)]
pub unsafe extern "C" fn getxattr(
    path: *const c_char,
    name: *const c_char,
    value: *mut c_void,
    size: size_t,
) -> ssize_t {
    static ORIGINAL: OnceLock<Option<GetxattrFn>> = OnceLock::new();
    let original = ORIGINAL.get_or_init(|| original_func("getxattr"));

    let redirected = get_redirect_path(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());

    if let Some(f) = original {
        unsafe { f(actual, name, value, size) }
    } else {
        -1
    }
}

type LgetxattrFn = unsafe extern "C" fn(*const c_char, *const c_char, *mut c_void, size_t) -> ssize_t;
#[unsafe(no_mangle)]
pub unsafe extern "C" fn lgetxattr(
    path: *const c_char,
    name: *const c_char,
    value: *mut c_void,
    size: size_t,
) -> ssize_t {
    static ORIGINAL: OnceLock<Option<LgetxattrFn>> = OnceLock::new();
    let original = ORIGINAL.get_or_init(|| original_func("lgetxattr"));

    let redirected = get_redirect_path(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());

    if let Some(f) = original {
        unsafe { f(actual, name, value, size) }
    } else {
        -1
    }
}

type SetxattrFn = unsafe extern "C" fn(*const c_char, *const c_char, *const c_void, size_t, c_int) -> c_int;
#[unsafe(no_mangle)]
pub unsafe extern "C" fn setxattr(
    path: *const c_char,
    name: *const c_char,
    value: *const c_void,
    size: size_t,
    flags: c_int,
) -> c_int {
    static ORIGINAL: OnceLock<Option<SetxattrFn>> = OnceLock::new();
    let original = ORIGINAL.get_or_init(|| original_func("setxattr"));

    let redirected = get_redirect_path(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());

    if let Some(f) = original {
        unsafe { f(actual, name, value, size, flags) }
    } else {
        -1
    }
}

type LsetxattrFn = unsafe extern "C" fn(*const c_char, *const c_char, *const c_void, size_t, c_int) -> c_int;
#[unsafe(no_mangle)]
pub unsafe extern "C" fn lsetxattr(
    path: *const c_char,
    name: *const c_char,
    value: *const c_void,
    size: size_t,
    flags: c_int,
) -> c_int {
    static ORIGINAL: OnceLock<Option<LsetxattrFn>> = OnceLock::new();
    let original = ORIGINAL.get_or_init(|| original_func("lsetxattr"));

    let redirected = get_redirect_path(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());

    if let Some(f) = original {
        unsafe { f(actual, name, value, size, flags) }
    } else {
        -1
    }
}

type ListxattrFn = unsafe extern "C" fn(*const c_char, *mut c_char, size_t) -> ssize_t;
#[unsafe(no_mangle)]
pub unsafe extern "C" fn listxattr(path: *const c_char, list: *mut c_char, size: size_t) -> ssize_t {
    static ORIGINAL: OnceLock<Option<ListxattrFn>> = OnceLock::new();
    let original = ORIGINAL.get_or_init(|| original_func("listxattr"));

    let redirected = get_redirect_path(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());

    if let Some(f) = original {
        unsafe { f(actual, list, size) }
    } else {
        -1
    }
}

type LlistxattrFn = unsafe extern "C" fn(*const c_char, *mut c_char, size_t) -> ssize_t;
#[unsafe(no_mangle)]
pub unsafe extern "C" fn llistxattr(path: *const c_char, list: *mut c_char, size: size_t) -> ssize_t {
    static ORIGINAL: OnceLock<Option<LlistxattrFn>> = OnceLock::new();
    let original = ORIGINAL.get_or_init(|| original_func("llistxattr"));

    let redirected = get_redirect_path(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());

    if let Some(f) = original {
        unsafe { f(actual, list, size) }
    } else {
        -1
    }
}

type RemovexattrFn = unsafe extern "C" fn(*const c_char, *const c_char) -> c_int;
#[unsafe(no_mangle)]
pub unsafe extern "C" fn removexattr(path: *const c_char, name: *const c_char) -> c_int {
    static ORIGINAL: OnceLock<Option<RemovexattrFn>> = OnceLock::new();
    let original = ORIGINAL.get_or_init(|| original_func("removexattr"));

    let redirected = get_redirect_path(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());

    if let Some(f) = original {
        unsafe { f(actual, name) }
    } else {
        -1
    }
}

type LremovexattrFn = unsafe extern "C" fn(*const c_char, *const c_char) -> c_int;
#[unsafe(no_mangle)]
pub unsafe extern "C" fn lremovexattr(path: *const c_char, name: *const c_char) -> c_int {
    static ORIGINAL: OnceLock<Option<LremovexattrFn>> = OnceLock::new();
    let original = ORIGINAL.get_or_init(|| original_func("lremovexattr"));

    let redirected = get_redirect_path(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());

    if let Some(f) = original {
        unsafe { f(actual, name) }
    } else {
        -1
    }
}

//
// Exec functions (to propagate LD_PRELOAD)
//

type ExecveFn = unsafe extern "C" fn(*const c_char, *const *const c_char, *const *const c_char) -> c_int;
#[unsafe(no_mangle)]
pub unsafe extern "C" fn execve(
    path: *const c_char,
    argv: *const *const c_char,
    envp: *const *const c_char,
) -> c_int {
    static ORIGINAL: OnceLock<Option<ExecveFn>> = OnceLock::new();
    let original = ORIGINAL.get_or_init(|| original_func("execve"));

    let redirected = get_redirect_path(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());

    if let Some(f) = original {
        unsafe { f(actual, argv, envp) }
    } else {
        -1
    }
}

type ExecvFn = unsafe extern "C" fn(*const c_char, *const *const c_char) -> c_int;
#[unsafe(no_mangle)]
pub unsafe extern "C" fn execv(path: *const c_char, argv: *const *const c_char) -> c_int {
    static ORIGINAL: OnceLock<Option<ExecvFn>> = OnceLock::new();
    let original = ORIGINAL.get_or_init(|| original_func("execv"));

    let redirected = get_redirect_path(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());

    if let Some(f) = original {
        unsafe { f(actual, argv) }
    } else {
        -1
    }
}
