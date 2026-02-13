//! C FFI layer for Swift/iOS interop.
//!
//! This module provides a C-compatible API for using immerse-core from Swift.
//! All functions are `extern "C"` and use raw pointers for interop.

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::ptr;
use std::sync::Arc;

use crate::config::{ConfigLoader, EnvironmentConfig};
use crate::engines::{AtmosphereEngine, LightsEngine, SoundEngine, SpotifyEngine};

/// Opaque handle for ConfigLoader.
pub struct FfiConfigLoader(ConfigLoader);

/// Opaque handle for SoundEngine.
pub struct FfiSoundEngine(SoundEngine);

/// Opaque handle for LightsEngine.
pub struct FfiLightsEngine(tokio::sync::Mutex<LightsEngine>);

/// Opaque handle for AtmosphereEngine.
pub struct FfiAtmosphereEngine(AtmosphereEngine);

// ============================================================================
// Config Loader
// ============================================================================

/// Creates a new config loader for the given directory.
///
/// # Safety
/// - `config_dir` must be a valid null-terminated UTF-8 string
/// - The returned pointer must be freed with `immerse_config_loader_free`
#[no_mangle]
pub unsafe extern "C" fn immerse_config_loader_new(
    config_dir: *const c_char,
) -> *mut FfiConfigLoader {
    if config_dir.is_null() {
        return ptr::null_mut();
    }

    let config_dir = match CStr::from_ptr(config_dir).to_str() {
        Ok(s) => s,
        Err(_) => return ptr::null_mut(),
    };

    let loader = ConfigLoader::new(config_dir);
    Box::into_raw(Box::new(FfiConfigLoader(loader)))
}

/// Frees a config loader.
///
/// # Safety
/// - `loader` must be a valid pointer returned by `immerse_config_loader_new`
/// - `loader` must not be used after this call
#[no_mangle]
pub unsafe extern "C" fn immerse_config_loader_free(loader: *mut FfiConfigLoader) {
    if !loader.is_null() {
        drop(Box::from_raw(loader));
    }
}

/// Loads a config file and returns it as JSON.
///
/// # Safety
/// - `loader` must be a valid pointer
/// - `filename` must be a valid null-terminated UTF-8 string
/// - The returned string must be freed with `immerse_free_string`
#[no_mangle]
pub unsafe extern "C" fn immerse_config_load(
    loader: *const FfiConfigLoader,
    filename: *const c_char,
) -> *mut c_char {
    if loader.is_null() || filename.is_null() {
        return ptr::null_mut();
    }

    let loader = &(*loader).0;
    let filename = match CStr::from_ptr(filename).to_str() {
        Ok(s) => s,
        Err(_) => return ptr::null_mut(),
    };

    match loader.load(filename) {
        Ok(config) => {
            match serde_json::to_string(&config) {
                Ok(json) => {
                    match CString::new(json) {
                        Ok(cstr) => cstr.into_raw(),
                        Err(_) => ptr::null_mut(),
                    }
                }
                Err(_) => ptr::null_mut(),
            }
        }
        Err(_) => ptr::null_mut(),
    }
}

/// Discovers all config files and returns them as a JSON array of filenames.
///
/// # Safety
/// - `loader` must be a valid pointer
/// - The returned string must be freed with `immerse_free_string`
#[no_mangle]
pub unsafe extern "C" fn immerse_config_discover_all(
    loader: *const FfiConfigLoader,
) -> *mut c_char {
    if loader.is_null() {
        return ptr::null_mut();
    }

    let loader = &(*loader).0;

    match loader.discover_all() {
        Ok(files) => {
            match serde_json::to_string(&files) {
                Ok(json) => {
                    match CString::new(json) {
                        Ok(cstr) => cstr.into_raw(),
                        Err(_) => ptr::null_mut(),
                    }
                }
                Err(_) => ptr::null_mut(),
            }
        }
        Err(_) => ptr::null_mut(),
    }
}

// ============================================================================
// Sound Engine
// ============================================================================

/// Creates a new sound engine.
///
/// # Safety
/// - `project_root` must be a valid null-terminated UTF-8 string
/// - The returned pointer must be freed with `immerse_sound_engine_free`
#[no_mangle]
pub unsafe extern "C" fn immerse_sound_engine_new(
    project_root: *const c_char,
) -> *mut FfiSoundEngine {
    if project_root.is_null() {
        return ptr::null_mut();
    }

    let project_root = match CStr::from_ptr(project_root).to_str() {
        Ok(s) => s,
        Err(_) => return ptr::null_mut(),
    };

    let engine = SoundEngine::new(project_root);
    Box::into_raw(Box::new(FfiSoundEngine(engine)))
}

/// Frees a sound engine.
///
/// # Safety
/// - `engine` must be a valid pointer returned by `immerse_sound_engine_new`
#[no_mangle]
pub unsafe extern "C" fn immerse_sound_engine_free(engine: *mut FfiSoundEngine) {
    if !engine.is_null() {
        drop(Box::from_raw(engine));
    }
}

/// Plays a sound file asynchronously.
///
/// # Safety
/// - `engine` must be a valid pointer
/// - `file` must be a valid null-terminated UTF-8 string
///
/// Returns true on success, false on failure.
#[no_mangle]
pub unsafe extern "C" fn immerse_sound_play(
    engine: *const FfiSoundEngine,
    file: *const c_char,
) -> bool {
    if engine.is_null() || file.is_null() {
        return false;
    }

    let engine = &(*engine).0;
    let file = match CStr::from_ptr(file).to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };

    engine.play_async(file).is_ok()
}

/// Stops all playing sounds.
///
/// # Safety
/// - `engine` must be a valid pointer
///
/// Returns the number of sounds stopped.
#[no_mangle]
pub unsafe extern "C" fn immerse_sound_stop_all(engine: *const FfiSoundEngine) -> u32 {
    if engine.is_null() {
        return 0;
    }

    let engine = &(*engine).0;
    engine.stop_all() as u32
}

/// Returns whether an audio player is available.
///
/// # Safety
/// - `engine` must be a valid pointer
#[no_mangle]
pub unsafe extern "C" fn immerse_sound_is_available(engine: *const FfiSoundEngine) -> bool {
    if engine.is_null() {
        return false;
    }

    let engine = &(*engine).0;
    engine.is_available()
}

// ============================================================================
// Atmosphere Engine
// ============================================================================

/// Creates a new atmosphere engine.
///
/// # Safety
/// - `project_root` must be a valid null-terminated UTF-8 string
/// - The returned pointer must be freed with `immerse_atmosphere_engine_free`
#[no_mangle]
pub unsafe extern "C" fn immerse_atmosphere_engine_new(
    project_root: *const c_char,
) -> *mut FfiAtmosphereEngine {
    if project_root.is_null() {
        return ptr::null_mut();
    }

    let project_root = match CStr::from_ptr(project_root).to_str() {
        Ok(s) => s,
        Err(_) => return ptr::null_mut(),
    };

    let engine = AtmosphereEngine::new(project_root);
    Box::into_raw(Box::new(FfiAtmosphereEngine(engine)))
}

/// Frees an atmosphere engine.
///
/// # Safety
/// - `engine` must be a valid pointer returned by `immerse_atmosphere_engine_new`
#[no_mangle]
pub unsafe extern "C" fn immerse_atmosphere_engine_free(engine: *mut FfiAtmosphereEngine) {
    if !engine.is_null() {
        drop(Box::from_raw(engine));
    }
}

/// Starts a single atmosphere sound.
///
/// # Safety
/// - `engine` must be a valid pointer
/// - `url` must be a valid null-terminated UTF-8 string
///
/// Returns true on success, false on failure.
#[no_mangle]
pub unsafe extern "C" fn immerse_atmosphere_start(
    engine: *const FfiAtmosphereEngine,
    url: *const c_char,
    volume: u8,
) -> bool {
    if engine.is_null() || url.is_null() {
        return false;
    }

    let engine = &(*engine).0;
    let url = match CStr::from_ptr(url).to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };

    engine.start_single(url, volume).is_ok()
}

/// Stops a single atmosphere sound.
///
/// # Safety
/// - `engine` must be a valid pointer
/// - `url` must be a valid null-terminated UTF-8 string
#[no_mangle]
pub unsafe extern "C" fn immerse_atmosphere_stop_single(
    engine: *const FfiAtmosphereEngine,
    url: *const c_char,
) -> bool {
    if engine.is_null() || url.is_null() {
        return false;
    }

    let engine = &(*engine).0;
    let url = match CStr::from_ptr(url).to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };

    engine.stop_single(url).is_ok()
}

/// Stops all atmosphere sounds.
///
/// # Safety
/// - `engine` must be a valid pointer
///
/// Returns the number of sounds stopped.
#[no_mangle]
pub unsafe extern "C" fn immerse_atmosphere_stop_all(engine: *const FfiAtmosphereEngine) -> u32 {
    if engine.is_null() {
        return 0;
    }

    let engine = &(*engine).0;
    engine.stop_all() as u32
}

// ============================================================================
// Utility Functions
// ============================================================================

/// Frees a string returned by an FFI function.
///
/// # Safety
/// - `s` must be a valid pointer returned by an immerse FFI function, or null
#[no_mangle]
pub unsafe extern "C" fn immerse_free_string(s: *mut c_char) {
    if !s.is_null() {
        drop(CString::from_raw(s));
    }
}

/// Returns the library version as a string.
///
/// # Safety
/// - The returned string must be freed with `immerse_free_string`
#[no_mangle]
pub extern "C" fn immerse_version() -> *mut c_char {
    let version = env!("CARGO_PKG_VERSION");
    match CString::new(version) {
        Ok(cstr) => cstr.into_raw(),
        Err(_) => ptr::null_mut(),
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;
    use tempfile::TempDir;

    #[test]
    fn test_config_loader_create_free() {
        let temp_dir = TempDir::new().unwrap();
        let path = CString::new(temp_dir.path().to_str().unwrap()).unwrap();

        unsafe {
            let loader = immerse_config_loader_new(path.as_ptr());
            assert!(!loader.is_null());
            immerse_config_loader_free(loader);
        }
    }

    #[test]
    fn test_sound_engine_create_free() {
        let temp_dir = TempDir::new().unwrap();
        let path = CString::new(temp_dir.path().to_str().unwrap()).unwrap();

        unsafe {
            let engine = immerse_sound_engine_new(path.as_ptr());
            assert!(!engine.is_null());
            immerse_sound_engine_free(engine);
        }
    }

    #[test]
    fn test_free_null_string() {
        unsafe {
            // Should not crash
            immerse_free_string(ptr::null_mut());
        }
    }

    #[test]
    fn test_version() {
        unsafe {
            let version = immerse_version();
            assert!(!version.is_null());

            let version_str = CStr::from_ptr(version).to_str().unwrap();
            assert!(!version_str.is_empty());

            immerse_free_string(version);
        }
    }
}
