//! Path redirect functions for LD_PRELOAD
//!
//! # Safety
//!
//! All functions in this module are FFI wrappers that intercept libc calls.
//! They require the same safety guarantees as the original libc functions:
//! - Pointers must be valid and point to properly initialized memory
//! - String pointers must be null-terminated C strings
//! - Buffer sizes must be accurate
#![allow(clippy::missing_safety_doc)]
#![allow(unsafe_op_in_unsafe_fn)]
#![allow(static_mut_refs)]

use crate::git::GitRepo;
use ctor::ctor;
use libc::*;
use std::cell::Cell;
use std::ffi::{CStr, CString};
use std::path::{Path, PathBuf};

// Thread-local recursion guard to prevent infinite recursion in hooks
thread_local! {
    static IN_HOOK: Cell<bool> = const { Cell::new(false) };
}

/// RAII guard for recursion protection
struct RecursionGuard;

impl RecursionGuard {
    /// Try to enter hook. Returns Some(guard) if not already in hook, None otherwise.
    fn try_enter() -> Option<Self> {
        IN_HOOK.with(|flag| {
            if flag.get() {
                None
            } else {
                flag.set(true);
                Some(RecursionGuard)
            }
        })
    }
}

impl Drop for RecursionGuard {
    fn drop(&mut self) {
        IN_HOOK.with(|flag| flag.set(false));
    }
}

/// Redirect configuration loaded at library init time
static mut REDIRECT_FROM: Option<String> = None;
static mut REDIRECT_TO: Option<String> = None;
/// Whether to skip redirecting gitignored paths
static mut SKIP_GITIGNORE: bool = false;

fn get_redirect() -> Option<(&'static str, &'static str)> {
    unsafe {
        match (REDIRECT_FROM.as_ref(), REDIRECT_TO.as_ref()) {
            (Some(from), Some(to)) => Some((from.as_str(), to.as_str())),
            _ => None,
        }
    }
}

/// Check if a path is gitignored (using cached prefixes from git2)
fn is_gitignored(absolute_str: &str) -> bool {
    unsafe {
        if !SKIP_GITIGNORE {
            return false;
        }

        let repo = match GitRepo::new(absolute_str).ok() {
            Some(v) => v,
            None => return false,
        };

        repo.is_ignored(absolute_str)
    }
}

/// Normalize a path without requiring it to exist
/// This handles . and .. components and joins with cwd if relative
fn normalize_path(path: &Path) -> Option<PathBuf> {
    use std::path::Component;

    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().ok()?.join(path)
    };

    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(p) => normalized.push(p.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {} // Skip .
            Component::ParentDir => {
                normalized.pop(); // Go up one directory
            }
            Component::Normal(c) => normalized.push(c),
        }
    }
    Some(normalized)
}

/// Core redirect logic - takes a normalized absolute path string
fn redirect_path_str(path_str: &str) -> Option<CString> {
    let (from, to) = get_redirect()?;

    // Convert input path to absolute and normalized
    // Use normalize_path instead of canonicalize to handle non-existent files
    let path = Path::new(path_str);
    let absolute_path = normalize_path(path)?;
    let absolute_str = absolute_path.to_str()?;

    // Don't redirect .git or its subdirectories
    // Git metadata should be accessed from the original repository.
    // Git worktree context is handled via GIT_DIR and GIT_WORK_TREE env vars.
    let is_git = absolute_str == format!("{from}/.git")
        || absolute_path.starts_with(format!("{from}/.git/"));
    if is_git {
        return None;
    }

    // Don't redirect gitignored paths (when REDIRECT_SKIP_GITIGNORE is set)
    let git_ignored = is_gitignored(absolute_str);
    if git_ignored {
        return None;
    }

    if let Some(suffix) = absolute_str.strip_prefix(&format!("{}/", from)) {
        let redirected = format!("{}/{}", to, suffix);
        Some(CString::new(redirected).ok()?)
    } else if absolute_str == from {
        Some(CString::new(to).ok()?)
    } else {
        None
    }
}

fn get_redirect_path(path: *const c_char) -> Option<CString> {
    if path.is_null() {
        return None;
    }

    let path_str = unsafe { CStr::from_ptr(path) }.to_str().ok()?;
    redirect_path_str(path_str)
}

/// Get redirect path for *at functions
fn get_redirect_path_at(dirfd: c_int, path: *const c_char) -> Option<CString> {
    if path.is_null() {
        return None;
    }

    let path_str = unsafe { CStr::from_ptr(path) }.to_str().ok()?;
    let first_char = unsafe { *path };

    if first_char == b'/' as c_char {
        // Absolute path
        redirect_path_str(path_str)
    } else if dirfd == AT_FDCWD {
        // Relative path with AT_FDCWD - resolve against cwd
        redirect_path_str(path_str)
    } else {
        // Relative path with specific dirfd - try to resolve via /proc/self/fd
        let fd_path = format!("/proc/self/fd/{}", dirfd);
        if let Ok(resolved) = std::fs::read_link(&fd_path) {
            let full_path = resolved.join(path_str);
            let full_path_str = full_path.to_str()?;
            redirect_path_str(full_path_str)
        } else {
            None
        }
    }
}

fn load_original<T>(name: &[u8]) -> Option<T> {
    let ptr = unsafe { libc::dlsym(libc::RTLD_NEXT, name.as_ptr() as *const c_char) };
    if ptr.is_null() {
        None
    } else {
        Some(unsafe { std::mem::transmute_copy(&ptr) })
    }
}

// Type aliases for function pointers
type OpenFn = unsafe extern "C" fn(*const c_char, c_int, mode_t) -> c_int;
type Open64Fn = unsafe extern "C" fn(*const c_char, c_int, mode_t) -> c_int;
type OpenatFn = unsafe extern "C" fn(c_int, *const c_char, c_int, mode_t) -> c_int;
type Openat64Fn = unsafe extern "C" fn(c_int, *const c_char, c_int, mode_t) -> c_int;
type CreatFn = unsafe extern "C" fn(*const c_char, mode_t) -> c_int;
type Creat64Fn = unsafe extern "C" fn(*const c_char, mode_t) -> c_int;
type StatFn = unsafe extern "C" fn(*const c_char, *mut stat) -> c_int;
type Stat64Fn = unsafe extern "C" fn(*const c_char, *mut stat64) -> c_int;
type LstatFn = unsafe extern "C" fn(*const c_char, *mut stat) -> c_int;
type Lstat64Fn = unsafe extern "C" fn(*const c_char, *mut stat64) -> c_int;
type FstatatFn = unsafe extern "C" fn(c_int, *const c_char, *mut stat, c_int) -> c_int;
type Fstatat64Fn = unsafe extern "C" fn(c_int, *const c_char, *mut stat64, c_int) -> c_int;
type StatxFn = unsafe extern "C" fn(c_int, *const c_char, c_int, c_uint, *mut statx) -> c_int;
type XstatFn = unsafe extern "C" fn(c_int, *const c_char, *mut stat) -> c_int;
type Xstat64Fn = unsafe extern "C" fn(c_int, *const c_char, *mut stat64) -> c_int;
type LxstatFn = unsafe extern "C" fn(c_int, *const c_char, *mut stat) -> c_int;
type Lxstat64Fn = unsafe extern "C" fn(c_int, *const c_char, *mut stat64) -> c_int;
type FxstatFn = unsafe extern "C" fn(c_int, c_int, *mut stat) -> c_int;
type Fxstat64Fn = unsafe extern "C" fn(c_int, c_int, *mut stat64) -> c_int;
type FxstatatFn = unsafe extern "C" fn(c_int, c_int, *const c_char, *mut stat, c_int) -> c_int;
type Fxstatat64Fn = unsafe extern "C" fn(c_int, c_int, *const c_char, *mut stat64, c_int) -> c_int;
type AccessFn = unsafe extern "C" fn(*const c_char, c_int) -> c_int;
type FaccessatFn = unsafe extern "C" fn(c_int, *const c_char, c_int, c_int) -> c_int;
type OpendirFn = unsafe extern "C" fn(*const c_char) -> *mut DIR;
type MkdirFn = unsafe extern "C" fn(*const c_char, mode_t) -> c_int;
type MkdiratFn = unsafe extern "C" fn(c_int, *const c_char, mode_t) -> c_int;
type RmdirFn = unsafe extern "C" fn(*const c_char) -> c_int;
type ChdirFn = unsafe extern "C" fn(*const c_char) -> c_int;
type UnlinkFn = unsafe extern "C" fn(*const c_char) -> c_int;
type UnlinkatFn = unsafe extern "C" fn(c_int, *const c_char, c_int) -> c_int;
type RenameFn = unsafe extern "C" fn(*const c_char, *const c_char) -> c_int;
type RenameatFn = unsafe extern "C" fn(c_int, *const c_char, c_int, *const c_char) -> c_int;
type Renameat2Fn =
    unsafe extern "C" fn(c_int, *const c_char, c_int, *const c_char, c_uint) -> c_int;
type TruncateFn = unsafe extern "C" fn(*const c_char, off_t) -> c_int;
type Truncate64Fn = unsafe extern "C" fn(*const c_char, off64_t) -> c_int;
type LinkFn = unsafe extern "C" fn(*const c_char, *const c_char) -> c_int;
type LinkatFn = unsafe extern "C" fn(c_int, *const c_char, c_int, *const c_char, c_int) -> c_int;
type SymlinkFn = unsafe extern "C" fn(*const c_char, *const c_char) -> c_int;
type SymlinkatFn = unsafe extern "C" fn(*const c_char, c_int, *const c_char) -> c_int;
type ReadlinkFn = unsafe extern "C" fn(*const c_char, *mut c_char, size_t) -> ssize_t;
type ReadlinkatFn = unsafe extern "C" fn(c_int, *const c_char, *mut c_char, size_t) -> ssize_t;
type ChmodFn = unsafe extern "C" fn(*const c_char, mode_t) -> c_int;
type FchmodatFn = unsafe extern "C" fn(c_int, *const c_char, mode_t, c_int) -> c_int;
type ChownFn = unsafe extern "C" fn(*const c_char, uid_t, gid_t) -> c_int;
type LchownFn = unsafe extern "C" fn(*const c_char, uid_t, gid_t) -> c_int;
type FchownatFn = unsafe extern "C" fn(c_int, *const c_char, uid_t, gid_t, c_int) -> c_int;
type RealpathFn = unsafe extern "C" fn(*const c_char, *mut c_char) -> *mut c_char;
type UtimeFn = unsafe extern "C" fn(*const c_char, *const utimbuf) -> c_int;
type UtimesFn = unsafe extern "C" fn(*const c_char, *const timeval) -> c_int;
type UtimensatFn = unsafe extern "C" fn(c_int, *const c_char, *const timespec, c_int) -> c_int;
type FutimesatFn = unsafe extern "C" fn(c_int, *const c_char, *const timeval) -> c_int;
type GetxattrFn =
    unsafe extern "C" fn(*const c_char, *const c_char, *mut c_void, size_t) -> ssize_t;
type LgetxattrFn =
    unsafe extern "C" fn(*const c_char, *const c_char, *mut c_void, size_t) -> ssize_t;
type SetxattrFn =
    unsafe extern "C" fn(*const c_char, *const c_char, *const c_void, size_t, c_int) -> c_int;
type LsetxattrFn =
    unsafe extern "C" fn(*const c_char, *const c_char, *const c_void, size_t, c_int) -> c_int;
type ListxattrFn = unsafe extern "C" fn(*const c_char, *mut c_char, size_t) -> ssize_t;
type LlistxattrFn = unsafe extern "C" fn(*const c_char, *mut c_char, size_t) -> ssize_t;
type RemovexattrFn = unsafe extern "C" fn(*const c_char, *const c_char) -> c_int;
type LremovexattrFn = unsafe extern "C" fn(*const c_char, *const c_char) -> c_int;
type ExecveFn =
    unsafe extern "C" fn(*const c_char, *const *const c_char, *const *const c_char) -> c_int;
type ExecvFn = unsafe extern "C" fn(*const c_char, *const *const c_char) -> c_int;

/// Pre-initialized original function pointers
#[allow(non_snake_case)]
struct OriginalFunctions {
    open: Option<OpenFn>,
    open64: Option<Open64Fn>,
    openat: Option<OpenatFn>,
    openat64: Option<Openat64Fn>,
    creat: Option<CreatFn>,
    creat64: Option<Creat64Fn>,
    stat: Option<StatFn>,
    stat64: Option<Stat64Fn>,
    lstat: Option<LstatFn>,
    lstat64: Option<Lstat64Fn>,
    fstatat: Option<FstatatFn>,
    fstatat64: Option<Fstatat64Fn>,
    statx: Option<StatxFn>,
    __xstat: Option<XstatFn>,
    __xstat64: Option<Xstat64Fn>,
    __lxstat: Option<LxstatFn>,
    __lxstat64: Option<Lxstat64Fn>,
    __fxstat: Option<FxstatFn>,
    __fxstat64: Option<Fxstat64Fn>,
    __fxstatat: Option<FxstatatFn>,
    __fxstatat64: Option<Fxstatat64Fn>,
    access: Option<AccessFn>,
    faccessat: Option<FaccessatFn>,
    opendir: Option<OpendirFn>,
    mkdir: Option<MkdirFn>,
    mkdirat: Option<MkdiratFn>,
    rmdir: Option<RmdirFn>,
    chdir: Option<ChdirFn>,
    unlink: Option<UnlinkFn>,
    unlinkat: Option<UnlinkatFn>,
    rename: Option<RenameFn>,
    renameat: Option<RenameatFn>,
    renameat2: Option<Renameat2Fn>,
    truncate: Option<TruncateFn>,
    truncate64: Option<Truncate64Fn>,
    link: Option<LinkFn>,
    linkat: Option<LinkatFn>,
    symlink: Option<SymlinkFn>,
    symlinkat: Option<SymlinkatFn>,
    readlink: Option<ReadlinkFn>,
    readlinkat: Option<ReadlinkatFn>,
    chmod: Option<ChmodFn>,
    fchmodat: Option<FchmodatFn>,
    chown: Option<ChownFn>,
    lchown: Option<LchownFn>,
    fchownat: Option<FchownatFn>,
    realpath: Option<RealpathFn>,
    utime: Option<UtimeFn>,
    utimes: Option<UtimesFn>,
    utimensat: Option<UtimensatFn>,
    futimesat: Option<FutimesatFn>,
    getxattr: Option<GetxattrFn>,
    lgetxattr: Option<LgetxattrFn>,
    setxattr: Option<SetxattrFn>,
    lsetxattr: Option<LsetxattrFn>,
    listxattr: Option<ListxattrFn>,
    llistxattr: Option<LlistxattrFn>,
    removexattr: Option<RemovexattrFn>,
    lremovexattr: Option<LremovexattrFn>,
    execve: Option<ExecveFn>,
    execv: Option<ExecvFn>,
}

static mut ORIGINAL: OriginalFunctions = OriginalFunctions {
    open: None,
    open64: None,
    openat: None,
    openat64: None,
    creat: None,
    creat64: None,
    stat: None,
    stat64: None,
    lstat: None,
    lstat64: None,
    fstatat: None,
    fstatat64: None,
    statx: None,
    __xstat: None,
    __xstat64: None,
    __lxstat: None,
    __lxstat64: None,
    __fxstat: None,
    __fxstat64: None,
    __fxstatat: None,
    __fxstatat64: None,
    access: None,
    faccessat: None,
    opendir: None,
    mkdir: None,
    mkdirat: None,
    rmdir: None,
    chdir: None,
    unlink: None,
    unlinkat: None,
    rename: None,
    renameat: None,
    renameat2: None,
    truncate: None,
    truncate64: None,
    link: None,
    linkat: None,
    symlink: None,
    symlinkat: None,
    readlink: None,
    readlinkat: None,
    chmod: None,
    fchmodat: None,
    chown: None,
    lchown: None,
    fchownat: None,
    realpath: None,
    utime: None,
    utimes: None,
    utimensat: None,
    futimesat: None,
    getxattr: None,
    lgetxattr: None,
    setxattr: None,
    lsetxattr: None,
    listxattr: None,
    llistxattr: None,
    removexattr: None,
    lremovexattr: None,
    execve: None,
    execv: None,
};

/// Library constructor - initializes all original function pointers and environment
#[ctor]
unsafe fn init() {
    // Pre-load all original function pointers FIRST
    // This must happen before any git2 operations which call libc functions
    unsafe {
        ORIGINAL.open = load_original(b"open\0");
        ORIGINAL.open64 = load_original(b"open64\0");
        ORIGINAL.openat = load_original(b"openat\0");
        ORIGINAL.openat64 = load_original(b"openat64\0");
        ORIGINAL.creat = load_original(b"creat\0");
        ORIGINAL.creat64 = load_original(b"creat64\0");
        ORIGINAL.stat = load_original(b"stat\0");
        ORIGINAL.stat64 = load_original(b"stat64\0");
        ORIGINAL.lstat = load_original(b"lstat\0");
        ORIGINAL.lstat64 = load_original(b"lstat64\0");
        ORIGINAL.fstatat = load_original(b"fstatat\0");
        ORIGINAL.fstatat64 = load_original(b"fstatat64\0");
        ORIGINAL.statx = load_original(b"statx\0");
        ORIGINAL.__xstat = load_original(b"__xstat\0");
        ORIGINAL.__xstat64 = load_original(b"__xstat64\0");
        ORIGINAL.__lxstat = load_original(b"__lxstat\0");
        ORIGINAL.__lxstat64 = load_original(b"__lxstat64\0");
        ORIGINAL.__fxstat = load_original(b"__fxstat\0");
        ORIGINAL.__fxstat64 = load_original(b"__fxstat64\0");
        ORIGINAL.__fxstatat = load_original(b"__fxstatat\0");
        ORIGINAL.__fxstatat64 = load_original(b"__fxstatat64\0");
        ORIGINAL.access = load_original(b"access\0");
        ORIGINAL.faccessat = load_original(b"faccessat\0");
        ORIGINAL.opendir = load_original(b"opendir\0");
        ORIGINAL.mkdir = load_original(b"mkdir\0");
        ORIGINAL.mkdirat = load_original(b"mkdirat\0");
        ORIGINAL.rmdir = load_original(b"rmdir\0");
        ORIGINAL.chdir = load_original(b"chdir\0");
        ORIGINAL.unlink = load_original(b"unlink\0");
        ORIGINAL.unlinkat = load_original(b"unlinkat\0");
        ORIGINAL.rename = load_original(b"rename\0");
        ORIGINAL.renameat = load_original(b"renameat\0");
        ORIGINAL.renameat2 = load_original(b"renameat2\0");
        ORIGINAL.truncate = load_original(b"truncate\0");
        ORIGINAL.truncate64 = load_original(b"truncate64\0");
        ORIGINAL.link = load_original(b"link\0");
        ORIGINAL.linkat = load_original(b"linkat\0");
        ORIGINAL.symlink = load_original(b"symlink\0");
        ORIGINAL.symlinkat = load_original(b"symlinkat\0");
        ORIGINAL.readlink = load_original(b"readlink\0");
        ORIGINAL.readlinkat = load_original(b"readlinkat\0");
        ORIGINAL.chmod = load_original(b"chmod\0");
        ORIGINAL.fchmodat = load_original(b"fchmodat\0");
        ORIGINAL.chown = load_original(b"chown\0");
        ORIGINAL.lchown = load_original(b"lchown\0");
        ORIGINAL.fchownat = load_original(b"fchownat\0");
        ORIGINAL.realpath = load_original(b"realpath\0");
        ORIGINAL.utime = load_original(b"utime\0");
        ORIGINAL.utimes = load_original(b"utimes\0");
        ORIGINAL.utimensat = load_original(b"utimensat\0");
        ORIGINAL.futimesat = load_original(b"futimesat\0");
        ORIGINAL.getxattr = load_original(b"getxattr\0");
        ORIGINAL.lgetxattr = load_original(b"lgetxattr\0");
        ORIGINAL.setxattr = load_original(b"setxattr\0");
        ORIGINAL.lsetxattr = load_original(b"lsetxattr\0");
        ORIGINAL.listxattr = load_original(b"listxattr\0");
        ORIGINAL.llistxattr = load_original(b"llistxattr\0");
        ORIGINAL.removexattr = load_original(b"removexattr\0");
        ORIGINAL.lremovexattr = load_original(b"lremovexattr\0");
        ORIGINAL.execve = load_original(b"execve\0");
        ORIGINAL.execv = load_original(b"execv\0");
    }

    // Now load environment variables (after original functions are available)
    if let Ok(from) = std::env::var("REDIRECT_FROM")
        && let Ok(to) = std::env::var("REDIRECT_TO")
    {
        // Check if we should skip gitignored paths
        SKIP_GITIGNORE = std::env::var("REDIRECT_SKIP_GITIGNORE")
            .map(|v| v != "0" && v.to_lowercase() != "false")
            .unwrap_or(true);

        REDIRECT_FROM = Some(from);
        REDIRECT_TO = Some(to);
    }
}

//
// File open functions
//

#[unsafe(no_mangle)]
pub unsafe extern "C" fn open(path: *const c_char, flags: c_int, mode: mode_t) -> c_int {
    let f = match ORIGINAL.open {
        Some(f) => f,
        None => return -1,
    };

    let _guard = match RecursionGuard::try_enter() {
        Some(g) => g,
        None => return f(path, flags, mode),
    };

    let redirected = get_redirect_path(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());
    f(actual, flags, mode)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn open64(path: *const c_char, flags: c_int, mode: mode_t) -> c_int {
    let f = match ORIGINAL.open64 {
        Some(f) => f,
        None => return -1,
    };

    let _guard = match RecursionGuard::try_enter() {
        Some(g) => g,
        None => return f(path, flags, mode),
    };

    let redirected = get_redirect_path(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());
    f(actual, flags, mode)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn openat(
    dirfd: c_int,
    path: *const c_char,
    flags: c_int,
    mode: mode_t,
) -> c_int {
    let f = match ORIGINAL.openat {
        Some(f) => f,
        None => return -1,
    };

    let _guard = match RecursionGuard::try_enter() {
        Some(g) => g,
        None => return f(dirfd, path, flags, mode),
    };

    let redirected = get_redirect_path_at(dirfd, path);
    let actual = redirected.as_ref().map_or(path, |p| p.as_ptr());
    f(dirfd, actual, flags, mode)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn openat64(
    dirfd: c_int,
    path: *const c_char,
    flags: c_int,
    mode: mode_t,
) -> c_int {
    let f = match ORIGINAL.openat64 {
        Some(f) => f,
        None => return -1,
    };

    let _guard = match RecursionGuard::try_enter() {
        Some(g) => g,
        None => return f(dirfd, path, flags, mode),
    };

    let redirected = get_redirect_path_at(dirfd, path);
    let actual = redirected.as_ref().map_or(path, |p| p.as_ptr());
    f(dirfd, actual, flags, mode)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn creat(path: *const c_char, mode: mode_t) -> c_int {
    let f = match ORIGINAL.creat {
        Some(f) => f,
        None => return -1,
    };

    let _guard = match RecursionGuard::try_enter() {
        Some(g) => g,
        None => return f(path, mode),
    };

    let redirected = get_redirect_path(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());
    f(actual, mode)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn creat64(path: *const c_char, mode: mode_t) -> c_int {
    let f = match ORIGINAL.creat64 {
        Some(f) => f,
        None => return -1,
    };

    let _guard = match RecursionGuard::try_enter() {
        Some(g) => g,
        None => return f(path, mode),
    };

    let redirected = get_redirect_path(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());
    f(actual, mode)
}

//
// Stat functions
//

#[unsafe(no_mangle)]
pub unsafe extern "C" fn stat(path: *const c_char, buf: *mut stat) -> c_int {
    let f = match ORIGINAL.stat {
        Some(f) => f,
        None => return -1,
    };

    let _guard = match RecursionGuard::try_enter() {
        Some(g) => g,
        None => return f(path, buf),
    };

    let redirected = get_redirect_path(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());
    f(actual, buf)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn stat64(path: *const c_char, buf: *mut stat64) -> c_int {
    let f = match ORIGINAL.stat64 {
        Some(f) => f,
        None => return -1,
    };

    let _guard = match RecursionGuard::try_enter() {
        Some(g) => g,
        None => return f(path, buf),
    };

    let redirected = get_redirect_path(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());
    f(actual, buf)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn lstat(path: *const c_char, buf: *mut stat) -> c_int {
    let f = match ORIGINAL.lstat {
        Some(f) => f,
        None => return -1,
    };

    let _guard = match RecursionGuard::try_enter() {
        Some(g) => g,
        None => return f(path, buf),
    };

    let redirected = get_redirect_path(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());
    f(actual, buf)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn lstat64(path: *const c_char, buf: *mut stat64) -> c_int {
    let f = match ORIGINAL.lstat64 {
        Some(f) => f,
        None => return -1,
    };

    let _guard = match RecursionGuard::try_enter() {
        Some(g) => g,
        None => return f(path, buf),
    };

    let redirected = get_redirect_path(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());
    f(actual, buf)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn fstatat(
    dirfd: c_int,
    path: *const c_char,
    buf: *mut stat,
    flags: c_int,
) -> c_int {
    let f = match ORIGINAL.fstatat {
        Some(f) => f,
        None => return -1,
    };

    let _guard = match RecursionGuard::try_enter() {
        Some(g) => g,
        None => return f(dirfd, path, buf, flags),
    };

    let redirected = get_redirect_path_at(dirfd, path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());
    f(dirfd, actual, buf, flags)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn fstatat64(
    dirfd: c_int,
    path: *const c_char,
    buf: *mut stat64,
    flags: c_int,
) -> c_int {
    let f = match ORIGINAL.fstatat64 {
        Some(f) => f,
        None => return -1,
    };

    let _guard = match RecursionGuard::try_enter() {
        Some(g) => g,
        None => return f(dirfd, path, buf, flags),
    };

    let redirected = get_redirect_path_at(dirfd, path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());
    f(dirfd, actual, buf, flags)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn statx(
    dirfd: c_int,
    path: *const c_char,
    flags: c_int,
    mask: c_uint,
    buf: *mut statx,
) -> c_int {
    let f = match ORIGINAL.statx {
        Some(f) => f,
        None => return -1,
    };

    let _guard = match RecursionGuard::try_enter() {
        Some(g) => g,
        None => return f(dirfd, path, flags, mask, buf),
    };

    let redirected = get_redirect_path_at(dirfd, path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());
    f(dirfd, actual, flags, mask, buf)
}

//
// Glibc internal stat functions
//

#[unsafe(no_mangle)]
pub unsafe extern "C" fn __xstat(ver: c_int, path: *const c_char, buf: *mut stat) -> c_int {
    let f = match ORIGINAL.__xstat {
        Some(f) => f,
        None => return -1,
    };

    let _guard = match RecursionGuard::try_enter() {
        Some(g) => g,
        None => return f(ver, path, buf),
    };

    let redirected = get_redirect_path(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());
    f(ver, actual, buf)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn __xstat64(ver: c_int, path: *const c_char, buf: *mut stat64) -> c_int {
    let f = match ORIGINAL.__xstat64 {
        Some(f) => f,
        None => return -1,
    };

    let _guard = match RecursionGuard::try_enter() {
        Some(g) => g,
        None => return f(ver, path, buf),
    };

    let redirected = get_redirect_path(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());
    f(ver, actual, buf)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn __lxstat(ver: c_int, path: *const c_char, buf: *mut stat) -> c_int {
    let f = match ORIGINAL.__lxstat {
        Some(f) => f,
        None => return -1,
    };

    let _guard = match RecursionGuard::try_enter() {
        Some(g) => g,
        None => return f(ver, path, buf),
    };

    let redirected = get_redirect_path(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());
    f(ver, actual, buf)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn __lxstat64(ver: c_int, path: *const c_char, buf: *mut stat64) -> c_int {
    let f = match ORIGINAL.__lxstat64 {
        Some(f) => f,
        None => return -1,
    };

    let _guard = match RecursionGuard::try_enter() {
        Some(g) => g,
        None => return f(ver, path, buf),
    };

    let redirected = get_redirect_path(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());
    f(ver, actual, buf)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn __fxstat(ver: c_int, fd: c_int, buf: *mut stat) -> c_int {
    // fxstat operates on fd, no path to redirect, but still need recursion guard
    let f = match ORIGINAL.__fxstat {
        Some(f) => f,
        None => return -1,
    };
    f(ver, fd, buf)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn __fxstat64(ver: c_int, fd: c_int, buf: *mut stat64) -> c_int {
    // fxstat operates on fd, no path to redirect
    let f = match ORIGINAL.__fxstat64 {
        Some(f) => f,
        None => return -1,
    };
    f(ver, fd, buf)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn __fxstatat(
    ver: c_int,
    dirfd: c_int,
    path: *const c_char,
    buf: *mut stat,
    flags: c_int,
) -> c_int {
    let f = match ORIGINAL.__fxstatat {
        Some(f) => f,
        None => return -1,
    };

    let _guard = match RecursionGuard::try_enter() {
        Some(g) => g,
        None => return f(ver, dirfd, path, buf, flags),
    };

    let redirected = get_redirect_path_at(dirfd, path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());
    f(ver, dirfd, actual, buf, flags)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn __fxstatat64(
    ver: c_int,
    dirfd: c_int,
    path: *const c_char,
    buf: *mut stat64,
    flags: c_int,
) -> c_int {
    let f = match ORIGINAL.__fxstatat64 {
        Some(f) => f,
        None => return -1,
    };

    let _guard = match RecursionGuard::try_enter() {
        Some(g) => g,
        None => return f(ver, dirfd, path, buf, flags),
    };

    let redirected = get_redirect_path_at(dirfd, path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());
    f(ver, dirfd, actual, buf, flags)
}

//
// Access functions
//

#[unsafe(no_mangle)]
pub unsafe extern "C" fn access(path: *const c_char, mode: c_int) -> c_int {
    let f = match ORIGINAL.access {
        Some(f) => f,
        None => return -1,
    };

    let _guard = match RecursionGuard::try_enter() {
        Some(g) => g,
        None => return f(path, mode),
    };

    let redirected = get_redirect_path(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());
    f(actual, mode)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn faccessat(
    dirfd: c_int,
    path: *const c_char,
    mode: c_int,
    flags: c_int,
) -> c_int {
    let f = match ORIGINAL.faccessat {
        Some(f) => f,
        None => return -1,
    };

    let _guard = match RecursionGuard::try_enter() {
        Some(g) => g,
        None => return f(dirfd, path, mode, flags),
    };

    let redirected = get_redirect_path_at(dirfd, path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());
    f(dirfd, actual, mode, flags)
}

//
// Directory functions
//

#[unsafe(no_mangle)]
pub unsafe extern "C" fn opendir(path: *const c_char) -> *mut DIR {
    let f = match ORIGINAL.opendir {
        Some(f) => f,
        None => return std::ptr::null_mut(),
    };

    let _guard = match RecursionGuard::try_enter() {
        Some(g) => g,
        None => return f(path),
    };

    let redirected = get_redirect_path(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());
    f(actual)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mkdir(path: *const c_char, mode: mode_t) -> c_int {
    let f = match ORIGINAL.mkdir {
        Some(f) => f,
        None => return -1,
    };

    let _guard = match RecursionGuard::try_enter() {
        Some(g) => g,
        None => return f(path, mode),
    };

    let redirected = get_redirect_path(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());
    f(actual, mode)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mkdirat(dirfd: c_int, path: *const c_char, mode: mode_t) -> c_int {
    let f = match ORIGINAL.mkdirat {
        Some(f) => f,
        None => return -1,
    };

    let _guard = match RecursionGuard::try_enter() {
        Some(g) => g,
        None => return f(dirfd, path, mode),
    };

    let redirected = get_redirect_path_at(dirfd, path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());
    f(dirfd, actual, mode)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmdir(path: *const c_char) -> c_int {
    let f = match ORIGINAL.rmdir {
        Some(f) => f,
        None => return -1,
    };

    let _guard = match RecursionGuard::try_enter() {
        Some(g) => g,
        None => return f(path),
    };

    let redirected = get_redirect_path(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());
    f(actual)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn chdir(path: *const c_char) -> c_int {
    let f = match ORIGINAL.chdir {
        Some(f) => f,
        None => return -1,
    };

    let _guard = match RecursionGuard::try_enter() {
        Some(g) => g,
        None => return f(path),
    };

    let redirected = get_redirect_path(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());
    f(actual)
}

//
// File manipulation functions
//

#[unsafe(no_mangle)]
pub unsafe extern "C" fn unlink(path: *const c_char) -> c_int {
    let f = match ORIGINAL.unlink {
        Some(f) => f,
        None => return -1,
    };

    let _guard = match RecursionGuard::try_enter() {
        Some(g) => g,
        None => return f(path),
    };

    let redirected = get_redirect_path(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());
    f(actual)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn unlinkat(dirfd: c_int, path: *const c_char, flags: c_int) -> c_int {
    let f = match ORIGINAL.unlinkat {
        Some(f) => f,
        None => return -1,
    };

    let _guard = match RecursionGuard::try_enter() {
        Some(g) => g,
        None => return f(dirfd, path, flags),
    };

    let redirected = get_redirect_path_at(dirfd, path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());
    f(dirfd, actual, flags)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rename(oldpath: *const c_char, newpath: *const c_char) -> c_int {
    let f = match ORIGINAL.rename {
        Some(f) => f,
        None => return -1,
    };

    let _guard = match RecursionGuard::try_enter() {
        Some(g) => g,
        None => return f(oldpath, newpath),
    };

    let old_redirected = get_redirect_path(oldpath);
    let new_redirected = get_redirect_path(newpath);
    let actual_old = old_redirected.as_ref().map_or(oldpath, |v| v.as_ptr());
    let actual_new = new_redirected.as_ref().map_or(newpath, |v| v.as_ptr());
    f(actual_old, actual_new)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn renameat(
    olddirfd: c_int,
    oldpath: *const c_char,
    newdirfd: c_int,
    newpath: *const c_char,
) -> c_int {
    let f = match ORIGINAL.renameat {
        Some(f) => f,
        None => return -1,
    };

    let _guard = match RecursionGuard::try_enter() {
        Some(g) => g,
        None => return f(olddirfd, oldpath, newdirfd, newpath),
    };

    let old_redirected = get_redirect_path_at(olddirfd, oldpath);
    let new_redirected = get_redirect_path_at(newdirfd, newpath);
    let actual_old = old_redirected.as_ref().map_or(oldpath, |v| v.as_ptr());
    let actual_new = new_redirected.as_ref().map_or(newpath, |v| v.as_ptr());
    f(olddirfd, actual_old, newdirfd, actual_new)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn renameat2(
    olddirfd: c_int,
    oldpath: *const c_char,
    newdirfd: c_int,
    newpath: *const c_char,
    flags: c_uint,
) -> c_int {
    let f = match ORIGINAL.renameat2 {
        Some(f) => f,
        None => return -1,
    };

    let _guard = match RecursionGuard::try_enter() {
        Some(g) => g,
        None => return f(olddirfd, oldpath, newdirfd, newpath, flags),
    };

    let old_redirected = get_redirect_path_at(olddirfd, oldpath);
    let new_redirected = get_redirect_path_at(newdirfd, newpath);
    let actual_old = old_redirected.as_ref().map_or(oldpath, |v| v.as_ptr());
    let actual_new = new_redirected.as_ref().map_or(newpath, |v| v.as_ptr());
    f(olddirfd, actual_old, newdirfd, actual_new, flags)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn truncate(path: *const c_char, length: off_t) -> c_int {
    let f = match ORIGINAL.truncate {
        Some(f) => f,
        None => return -1,
    };

    let _guard = match RecursionGuard::try_enter() {
        Some(g) => g,
        None => return f(path, length),
    };

    let redirected = get_redirect_path(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());
    f(actual, length)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn truncate64(path: *const c_char, length: off64_t) -> c_int {
    let f = match ORIGINAL.truncate64 {
        Some(f) => f,
        None => return -1,
    };

    let _guard = match RecursionGuard::try_enter() {
        Some(g) => g,
        None => return f(path, length),
    };

    let redirected = get_redirect_path(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());
    f(actual, length)
}

//
// Link functions
//

#[unsafe(no_mangle)]
pub unsafe extern "C" fn link(oldpath: *const c_char, newpath: *const c_char) -> c_int {
    let f = match ORIGINAL.link {
        Some(f) => f,
        None => return -1,
    };

    let _guard = match RecursionGuard::try_enter() {
        Some(g) => g,
        None => return f(oldpath, newpath),
    };

    let old_redirected = get_redirect_path(oldpath);
    let new_redirected = get_redirect_path(newpath);
    let actual_old = old_redirected.as_ref().map_or(oldpath, |v| v.as_ptr());
    let actual_new = new_redirected.as_ref().map_or(newpath, |v| v.as_ptr());
    f(actual_old, actual_new)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn linkat(
    olddirfd: c_int,
    oldpath: *const c_char,
    newdirfd: c_int,
    newpath: *const c_char,
    flags: c_int,
) -> c_int {
    let f = match ORIGINAL.linkat {
        Some(f) => f,
        None => return -1,
    };

    let _guard = match RecursionGuard::try_enter() {
        Some(g) => g,
        None => return f(olddirfd, oldpath, newdirfd, newpath, flags),
    };

    let old_redirected = get_redirect_path_at(olddirfd, oldpath);
    let new_redirected = get_redirect_path_at(newdirfd, newpath);
    let actual_old = old_redirected.as_ref().map_or(oldpath, |v| v.as_ptr());
    let actual_new = new_redirected.as_ref().map_or(newpath, |v| v.as_ptr());
    f(olddirfd, actual_old, newdirfd, actual_new, flags)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn symlink(target: *const c_char, linkpath: *const c_char) -> c_int {
    let f = match ORIGINAL.symlink {
        Some(f) => f,
        None => return -1,
    };

    let _guard = match RecursionGuard::try_enter() {
        Some(g) => g,
        None => return f(target, linkpath),
    };

    // Note: target is the content of the symlink, so we don't redirect it
    // We only redirect linkpath (where the symlink is created)
    let link_redirected = get_redirect_path(linkpath);
    let actual_link = link_redirected.as_ref().map_or(linkpath, |v| v.as_ptr());
    f(target, actual_link)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn symlinkat(
    target: *const c_char,
    newdirfd: c_int,
    linkpath: *const c_char,
) -> c_int {
    let f = match ORIGINAL.symlinkat {
        Some(f) => f,
        None => return -1,
    };

    let _guard = match RecursionGuard::try_enter() {
        Some(g) => g,
        None => return f(target, newdirfd, linkpath),
    };

    let link_redirected = get_redirect_path_at(newdirfd, linkpath);
    let actual_link = link_redirected.as_ref().map_or(linkpath, |v| v.as_ptr());
    f(target, newdirfd, actual_link)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn readlink(
    path: *const c_char,
    buf: *mut c_char,
    bufsize: size_t,
) -> ssize_t {
    let f = match ORIGINAL.readlink {
        Some(f) => f,
        None => return -1,
    };

    let _guard = match RecursionGuard::try_enter() {
        Some(g) => g,
        None => return f(path, buf, bufsize),
    };

    let redirected = get_redirect_path(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());
    f(actual, buf, bufsize)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn readlinkat(
    dirfd: c_int,
    path: *const c_char,
    buf: *mut c_char,
    bufsize: size_t,
) -> ssize_t {
    let f = match ORIGINAL.readlinkat {
        Some(f) => f,
        None => return -1,
    };

    let _guard = match RecursionGuard::try_enter() {
        Some(g) => g,
        None => return f(dirfd, path, buf, bufsize),
    };

    let redirected = get_redirect_path_at(dirfd, path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());
    f(dirfd, actual, buf, bufsize)
}

//
// Permission functions
//

#[unsafe(no_mangle)]
pub unsafe extern "C" fn chmod(path: *const c_char, mode: mode_t) -> c_int {
    let f = match ORIGINAL.chmod {
        Some(f) => f,
        None => return -1,
    };

    let _guard = match RecursionGuard::try_enter() {
        Some(g) => g,
        None => return f(path, mode),
    };

    let redirected = get_redirect_path(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());
    f(actual, mode)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn fchmodat(
    dirfd: c_int,
    path: *const c_char,
    mode: mode_t,
    flags: c_int,
) -> c_int {
    let f = match ORIGINAL.fchmodat {
        Some(f) => f,
        None => return -1,
    };

    let _guard = match RecursionGuard::try_enter() {
        Some(g) => g,
        None => return f(dirfd, path, mode, flags),
    };

    let redirected = get_redirect_path_at(dirfd, path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());
    f(dirfd, actual, mode, flags)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn chown(path: *const c_char, owner: uid_t, group: gid_t) -> c_int {
    let f = match ORIGINAL.chown {
        Some(f) => f,
        None => return -1,
    };

    let _guard = match RecursionGuard::try_enter() {
        Some(g) => g,
        None => return f(path, owner, group),
    };

    let redirected = get_redirect_path(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());
    f(actual, owner, group)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn lchown(path: *const c_char, owner: uid_t, group: gid_t) -> c_int {
    let f = match ORIGINAL.lchown {
        Some(f) => f,
        None => return -1,
    };

    let _guard = match RecursionGuard::try_enter() {
        Some(g) => g,
        None => return f(path, owner, group),
    };

    let redirected = get_redirect_path(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());
    f(actual, owner, group)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn fchownat(
    dirfd: c_int,
    path: *const c_char,
    owner: uid_t,
    group: gid_t,
    flags: c_int,
) -> c_int {
    let f = match ORIGINAL.fchownat {
        Some(f) => f,
        None => return -1,
    };

    let _guard = match RecursionGuard::try_enter() {
        Some(g) => g,
        None => return f(dirfd, path, owner, group, flags),
    };

    let redirected = get_redirect_path_at(dirfd, path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());
    f(dirfd, actual, owner, group, flags)
}

//
// Other functions
//

#[unsafe(no_mangle)]
pub unsafe extern "C" fn realpath(path: *const c_char, resolved_path: *mut c_char) -> *mut c_char {
    let f = match ORIGINAL.realpath {
        Some(f) => f,
        None => return std::ptr::null_mut(),
    };

    let _guard = match RecursionGuard::try_enter() {
        Some(g) => g,
        None => return f(path, resolved_path),
    };

    let redirected = get_redirect_path(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());
    f(actual, resolved_path)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn utime(path: *const c_char, times: *const utimbuf) -> c_int {
    let f = match ORIGINAL.utime {
        Some(f) => f,
        None => return -1,
    };

    let _guard = match RecursionGuard::try_enter() {
        Some(g) => g,
        None => return f(path, times),
    };

    let redirected = get_redirect_path(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());
    f(actual, times)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn utimes(path: *const c_char, times: *const timeval) -> c_int {
    let f = match ORIGINAL.utimes {
        Some(f) => f,
        None => return -1,
    };

    let _guard = match RecursionGuard::try_enter() {
        Some(g) => g,
        None => return f(path, times),
    };

    let redirected = get_redirect_path(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());
    f(actual, times)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn utimensat(
    dirfd: c_int,
    path: *const c_char,
    times: *const timespec,
    flags: c_int,
) -> c_int {
    let f = match ORIGINAL.utimensat {
        Some(f) => f,
        None => return -1,
    };

    let _guard = match RecursionGuard::try_enter() {
        Some(g) => g,
        None => return f(dirfd, path, times, flags),
    };

    let redirected = get_redirect_path_at(dirfd, path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());
    f(dirfd, actual, times, flags)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn futimesat(
    dirfd: c_int,
    path: *const c_char,
    times: *const timeval,
) -> c_int {
    let f = match ORIGINAL.futimesat {
        Some(f) => f,
        None => return -1,
    };

    let _guard = match RecursionGuard::try_enter() {
        Some(g) => g,
        None => return f(dirfd, path, times),
    };

    let redirected = get_redirect_path_at(dirfd, path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());
    f(dirfd, actual, times)
}

//
// Extended attribute functions
//

#[unsafe(no_mangle)]
pub unsafe extern "C" fn getxattr(
    path: *const c_char,
    name: *const c_char,
    value: *mut c_void,
    size: size_t,
) -> ssize_t {
    let f = match ORIGINAL.getxattr {
        Some(f) => f,
        None => return -1,
    };

    let _guard = match RecursionGuard::try_enter() {
        Some(g) => g,
        None => return f(path, name, value, size),
    };

    let redirected = get_redirect_path(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());
    f(actual, name, value, size)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn lgetxattr(
    path: *const c_char,
    name: *const c_char,
    value: *mut c_void,
    size: size_t,
) -> ssize_t {
    let f = match ORIGINAL.lgetxattr {
        Some(f) => f,
        None => return -1,
    };

    let _guard = match RecursionGuard::try_enter() {
        Some(g) => g,
        None => return f(path, name, value, size),
    };

    let redirected = get_redirect_path(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());
    f(actual, name, value, size)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn setxattr(
    path: *const c_char,
    name: *const c_char,
    value: *const c_void,
    size: size_t,
    flags: c_int,
) -> c_int {
    let f = match ORIGINAL.setxattr {
        Some(f) => f,
        None => return -1,
    };

    let _guard = match RecursionGuard::try_enter() {
        Some(g) => g,
        None => return f(path, name, value, size, flags),
    };

    let redirected = get_redirect_path(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());
    f(actual, name, value, size, flags)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn lsetxattr(
    path: *const c_char,
    name: *const c_char,
    value: *const c_void,
    size: size_t,
    flags: c_int,
) -> c_int {
    let f = match ORIGINAL.lsetxattr {
        Some(f) => f,
        None => return -1,
    };

    let _guard = match RecursionGuard::try_enter() {
        Some(g) => g,
        None => return f(path, name, value, size, flags),
    };

    let redirected = get_redirect_path(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());
    f(actual, name, value, size, flags)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn listxattr(
    path: *const c_char,
    list: *mut c_char,
    size: size_t,
) -> ssize_t {
    let f = match ORIGINAL.listxattr {
        Some(f) => f,
        None => return -1,
    };

    let _guard = match RecursionGuard::try_enter() {
        Some(g) => g,
        None => return f(path, list, size),
    };

    let redirected = get_redirect_path(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());
    f(actual, list, size)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn llistxattr(
    path: *const c_char,
    list: *mut c_char,
    size: size_t,
) -> ssize_t {
    let f = match ORIGINAL.llistxattr {
        Some(f) => f,
        None => return -1,
    };

    let _guard = match RecursionGuard::try_enter() {
        Some(g) => g,
        None => return f(path, list, size),
    };

    let redirected = get_redirect_path(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());
    f(actual, list, size)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn removexattr(path: *const c_char, name: *const c_char) -> c_int {
    let f = match ORIGINAL.removexattr {
        Some(f) => f,
        None => return -1,
    };

    let _guard = match RecursionGuard::try_enter() {
        Some(g) => g,
        None => return f(path, name),
    };

    let redirected = get_redirect_path(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());
    f(actual, name)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn lremovexattr(path: *const c_char, name: *const c_char) -> c_int {
    let f = match ORIGINAL.lremovexattr {
        Some(f) => f,
        None => return -1,
    };

    let _guard = match RecursionGuard::try_enter() {
        Some(g) => g,
        None => return f(path, name),
    };

    let redirected = get_redirect_path(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());
    f(actual, name)
}

//
// Exec functions (to propagate LD_PRELOAD)
//

#[unsafe(no_mangle)]
pub unsafe extern "C" fn execve(
    path: *const c_char,
    argv: *const *const c_char,
    envp: *const *const c_char,
) -> c_int {
    let f = match ORIGINAL.execve {
        Some(f) => f,
        None => return -1,
    };

    let _guard = match RecursionGuard::try_enter() {
        Some(g) => g,
        None => return f(path, argv, envp),
    };

    let redirected = get_redirect_path(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());
    f(actual, argv, envp)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn execv(path: *const c_char, argv: *const *const c_char) -> c_int {
    let f = match ORIGINAL.execv {
        Some(f) => f,
        None => return -1,
    };

    let _guard = match RecursionGuard::try_enter() {
        Some(g) => g,
        None => return f(path, argv),
    };

    let redirected = get_redirect_path(path);
    let actual = redirected.as_ref().map_or(path, |v| v.as_ptr());
    f(actual, argv)
}
