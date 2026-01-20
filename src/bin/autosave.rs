use anyhow::Context as _;
use libloading::{Library, Symbol};
use std::env;
use std::ffi;
use std::fs;
use std::path::Path;
use std::process::exit;
use tracing_subscriber::{
    Layer,
    filter::{EnvFilter, LevelFilter},
    prelude::*,
    registry::Registry,
    reload::Handle,
};

pub type TracingReloadHandle = Handle<Box<dyn Layer<Registry> + Send + Sync>, Registry>;

const DYLIB_BIN: &[u8] = include_bytes!(env!("DYLIB_PATH"));
const PKG_NAME: &str = env!("CARGO_PKG_NAME");
const PKG_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(target_os = "linux")]
const CDYLIB_EXT: &str = "so";
#[cfg(target_os = "macos")]
const CDYLIB_EXT: &str = "dylib";

fn main() {
    let layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr)
        .with_filter(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::WARN.into())
                .from_env_lossy(),
        )
        .boxed();
    let (layer, reload_handle) = tracing_subscriber::reload::Layer::new(layer);
    tracing_subscriber::registry().with(layer).init();

    let exe_dir = match env::current_exe().context("failed to get executable path") {
        Ok(v) => v.parent().unwrap().to_path_buf(),
        Err(e) => {
            tracing::error!("{e:?}");
            exit(1);
        }
    };
    let cdylib_path = exe_dir.join(format!("lib{PKG_NAME}.{CDYLIB_EXT}"));
    tracing::debug!("library path: {}", cdylib_path.display());

    for _ in 0..1 {
        if !cdylib_path.is_file() {
            tracing::info!(
                "library not found; output binary to: {}",
                cdylib_path.display()
            );
            write_library(&cdylib_path);
        }
        let dylib = match DyLib::load(&cdylib_path) {
            Ok(lib) => match lib.version() {
                Ok(version) => {
                    tracing::debug!("current library version: {version}");
                    if version == PKG_VERSION {
                        Some(lib)
                    } else {
                        None
                    }
                }
                Err(e) => {
                    tracing::warn!("{e:?}");
                    None
                }
            },
            Err(e) => {
                tracing::warn!("{e:?}");
                None
            }
        };
        if let Some(dylib) = dylib {
            tracing::debug!("enter main function");
            if let Err(e) = dylib.main(&reload_handle, &cdylib_path) {
                tracing::warn!("{e:?}");
            }
        }
        tracing::info!("error in loading library; remove library and retry");
        if let Err(e) = fs::remove_file(&cdylib_path).context("failed to remove library") {
            tracing::error!("{e:?}");
            exit(1);
        }
    }
    tracing::error!("max retry exceeded");
    exit(1);
}

fn write_library(path: &Path) {
    if let Err(e) = fs::write(path, DYLIB_BIN).context("failed to write dynamic link library") {
        tracing::error!("{e:?}");
        exit(1);
    }
}

struct DyLib {
    cdylib: Library,
}

impl DyLib {
    fn load(library_path: &Path) -> anyhow::Result<Self> {
        let cdylib = unsafe { Library::new(library_path).context("failed to load library") }?;
        Ok(Self { cdylib })
    }

    pub fn version(&self) -> anyhow::Result<String> {
        unsafe {
            let func: Symbol<unsafe extern "C" fn() -> *mut u8> = self
                .cdylib
                .get(b"version")
                .context("failed to load version function")?;
            let ptr = func();
            Ok(ffi::CString::from_raw(ptr).to_string_lossy().to_string())
        }
    }
    pub fn main(
        &self,
        tracing_handle: &TracingReloadHandle,
        cdylib_path: &Path,
    ) -> anyhow::Result<()> {
        let cdylib_str = cdylib_path.to_string_lossy().to_string();
        let cdylib_cstr = unsafe { ffi::CStr::from_ptr(cdylib_str.as_ptr()) };
        unsafe {
            let func: Symbol<unsafe extern "C" fn(&TracingReloadHandle, *const ffi::c_char)> = self
                .cdylib
                .get(b"main")
                .context("failed to load main function")?;
            func(tracing_handle, cdylib_cstr.as_ptr());
            exit(0);
        }
    }
}
