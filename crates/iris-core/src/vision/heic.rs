//! Offline HEVC-in-HEIF decoding through the bundled LGPL libheif/libde265 DLLs.
//! Only stable C functions are loaded; no system codec or external process is used.
use super::{check_raster_dimensions, resize_preview, DecodedPreview};
use anyhow::{bail, Context, Result};
use image::RgbImage;
use std::{
    collections::HashMap,
    ffi::{c_char, c_void, CStr},
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    ptr,
    sync::{Arc, Mutex, OnceLock},
};

#[repr(C)]
struct HeifError {
    code: i32,
    subcode: i32,
    message: *const c_char,
}
impl HeifError {
    fn check(self) -> Result<()> {
        if self.code == 0 {
            return Ok(());
        }
        let message = if self.message.is_null() {
            "unknown libheif error".into()
        } else {
            unsafe { CStr::from_ptr(self.message) }.to_string_lossy()
        };
        bail!(
            "HEIC decode error {}:{}: {}",
            self.code,
            self.subcode,
            message
        )
    }
}

struct Api {
    _library: libloading::Library,
    alloc: unsafe extern "C" fn() -> *mut c_void,
    free: unsafe extern "C" fn(*mut c_void),
    read: unsafe extern "C" fn(*mut c_void, *const c_void, usize, *const c_void) -> HeifError,
    primary: unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> HeifError,
    thumbnail_count: unsafe extern "C" fn(*const c_void) -> i32,
    thumbnail_ids: unsafe extern "C" fn(*const c_void, *mut u32, i32) -> i32,
    thumbnail: unsafe extern "C" fn(*const c_void, u32, *mut *mut c_void) -> HeifError,
    release_handle: unsafe extern "C" fn(*mut c_void),
    width: unsafe extern "C" fn(*const c_void) -> i32,
    height: unsafe extern "C" fn(*const c_void) -> i32,
    limit: unsafe extern "C" fn(*mut c_void, i32),
    threads: unsafe extern "C" fn(*mut c_void, i32),
    decode:
        unsafe extern "C" fn(*const c_void, *mut *mut c_void, i32, i32, *const c_void) -> HeifError,
    release_image: unsafe extern "C" fn(*mut c_void),
    image_width: unsafe extern "C" fn(*const c_void, i32) -> i32,
    image_height: unsafe extern "C" fn(*const c_void, i32) -> i32,
    plane: unsafe extern "C" fn(*const c_void, i32, *mut i32) -> *const u8,
}

fn runtime_directory(explicit: Option<&Path>) -> Result<PathBuf> {
    if let Some(value) = std::env::var_os("IRIS_MEDIA_RUNTIME_DIR") {
        anyhow::ensure!(
            !value.is_empty(),
            "IRIS_MEDIA_RUNTIME_DIR must not be empty"
        );
        return Ok(value.into());
    }
    if let Some(path) = explicit {
        return Ok(path.to_owned());
    }
    let installed = std::env::current_exe()?
        .parent()
        .context("executable has no parent")?
        .join("models/media");
    if installed.is_dir() {
        return Ok(installed);
    }
    #[cfg(debug_assertions)]
    {
        return Ok(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../models/media"));
    }
    #[cfg(not(debug_assertions))]
    Ok(installed)
}

impl Api {
    fn load(directory: Option<&Path>) -> Result<Arc<Self>> {
        static LOADED: OnceLock<Mutex<HashMap<PathBuf, Arc<Api>>>> = OnceLock::new();
        let directory = runtime_directory(directory)?.canonicalize()
            .context("HEIC runtime missing; run python tools/setup-heif-runtime.py or configure IRIS_MEDIA_RUNTIME_DIR")?;
        let mut loaded = LOADED
            .get_or_init(Default::default)
            .lock()
            .map_err(|_| anyhow::anyhow!("HEIC runtime lock poisoned"))?;
        if let Some(api) = loaded.get(&directory) {
            return Ok(api.clone());
        }
        #[cfg(windows)]
        let filename = "libheif.dll";
        #[cfg(not(windows))]
        let filename = "libheif.so";
        let path = directory.join(filename);
        // Search dependencies only beside the explicit DLL and Windows system/default
        // directories, never through cwd or arbitrary PATH entries.
        #[cfg(windows)]
        let library: libloading::Library = unsafe {
            libloading::os::windows::Library::load_with_flags(&path, 0x00000100 | 0x00001000)
        }
        .with_context(|| format!("HEIC runtime could not load {}", path.display()))?
        .into();
        #[cfg(not(windows))]
        let library = unsafe { libloading::Library::new(&path) }
            .with_context(|| format!("HEIC runtime could not load {}", path.display()))?;
        unsafe {
            let version =
                library.get::<unsafe extern "C" fn() -> u32>(b"heif_get_version_number\0")?;
            let version = version();
            anyhow::ensure!(
                version >> 24 == 1 && version >= 0x01170000,
                "HEIC runtime requires compatible libheif 1.x >= 1.23"
            );
            let available = library
                .get::<unsafe extern "C" fn(i32) -> i32>(b"heif_have_decoder_for_format\0")?;
            anyhow::ensure!(available(1) != 0, "HEIC runtime has no HEVC decoder");
            macro_rules! sym {
                ($name:literal) => {
                    *library.get(concat!($name, "\0").as_bytes())?
                };
            }
            let api = Arc::new(Self {
                alloc: sym!("heif_context_alloc"),
                free: sym!("heif_context_free"),
                read: sym!("heif_context_read_from_memory_without_copy"),
                primary: sym!("heif_context_get_primary_image_handle"),
                thumbnail_count: sym!("heif_image_handle_get_number_of_thumbnails"),
                thumbnail_ids: sym!("heif_image_handle_get_list_of_thumbnail_IDs"),
                thumbnail: sym!("heif_image_handle_get_thumbnail"),
                release_handle: sym!("heif_image_handle_release"),
                width: sym!("heif_image_handle_get_width"),
                height: sym!("heif_image_handle_get_height"),
                limit: sym!("heif_context_set_maximum_image_size_limit"),
                threads: sym!("heif_context_set_max_decoding_threads"),
                decode: sym!("heif_decode_image"),
                release_image: sym!("heif_image_release"),
                image_width: sym!("heif_image_get_width"),
                image_height: sym!("heif_image_get_height"),
                plane: sym!("heif_image_get_plane_readonly"),
                _library: library,
            });
            loaded.insert(directory, api.clone());
            Ok(api)
        }
    }
}

struct Opened {
    api: Arc<Api>,
    _bytes: Vec<u8>,
    context: *mut c_void,
    handle: *mut c_void,
}
impl Drop for Opened {
    fn drop(&mut self) {
        unsafe {
            if !self.handle.is_null() {
                (self.api.release_handle)(self.handle);
            }
            if !self.context.is_null() {
                (self.api.free)(self.context);
            }
        }
    }
}
impl Opened {
    fn open(path: &Path, directory: Option<&Path>) -> Result<Self> {
        let api = Api::load(directory)?;
        let mut bytes = Vec::new();
        File::open(path)?
            .take(super::MAX_DECODE_BYTES + 1)
            .read_to_end(&mut bytes)?;
        anyhow::ensure!(
            bytes.len() as u64 <= super::MAX_DECODE_BYTES,
            "HEIC file exceeds compressed input safety limit"
        );
        let mut opened = Self {
            api,
            _bytes: bytes,
            context: ptr::null_mut(),
            handle: ptr::null_mut(),
        };
        unsafe {
            opened.context = (opened.api.alloc)();
            anyhow::ensure!(!opened.context.is_null(), "HEIC context allocation failed");
            // This API sets an area limit (7746 squared), not a per-axis limit.
            (opened.api.limit)(opened.context, 7746);
            (opened.api.threads)(opened.context, 1);
            (opened.api.read)(
                opened.context,
                opened._bytes.as_ptr().cast(),
                opened._bytes.len(),
                ptr::null(),
            )
            .check()?;
            (opened.api.primary)(opened.context, &mut opened.handle).check()?;
            anyhow::ensure!(!opened.handle.is_null(), "HEIC primary image missing");
        }
        opened.dimensions()?;
        Ok(opened)
    }
    fn dimensions(&self) -> Result<(u32, u32)> {
        let (width, height) = unsafe {
            (
                (self.api.width)(self.handle),
                (self.api.height)(self.handle),
            )
        };
        anyhow::ensure!(width > 0 && height > 0, "HEIC dimensions invalid");
        check_raster_dimensions(width as u32, height as u32)?;
        Ok((width as u32, height as u32))
    }
}

pub(super) fn dimensions(path: &Path, directory: Option<&Path>) -> Result<(u32, u32)> {
    Opened::open(path, directory)?.dimensions()
}

pub(super) fn decode(
    path: &Path,
    max_edge: u32,
    directory: Option<&Path>,
) -> Result<DecodedPreview> {
    decode_inner(path, max_edge, directory, true)
}
fn decode_inner(
    path: &Path,
    max_edge: u32,
    directory: Option<&Path>,
    prefer_thumbnail: bool,
) -> Result<DecodedPreview> {
    let mut opened = Opened::open(path, directory)?;
    let (original_width, original_height) = opened.dimensions()?;
    // Select the smallest adequate native thumbnail. Never upscale a tiny
    // thumbnail for quality inference, and reject aspect-ratio mismatches.
    let mut best: Option<(*mut c_void, u32)> = None;
    unsafe {
        let count = (opened.api.thumbnail_count)(opened.handle);
        if prefer_thumbnail && (1..=1024).contains(&count) {
            let mut ids = vec![0; count as usize];
            let actual = (opened.api.thumbnail_ids)(opened.handle, ids.as_mut_ptr(), count);
            for id in ids.into_iter().take(actual.max(0).min(count) as usize) {
                let mut handle = ptr::null_mut();
                if (opened.api.thumbnail)(opened.handle, id, &mut handle)
                    .check()
                    .is_err()
                    || handle.is_null()
                {
                    continue;
                }
                let w = (opened.api.width)(handle);
                let h = (opened.api.height)(handle);
                let ratio =
                    (w as f64 / h as f64) / (original_width as f64 / original_height as f64);
                let edge = w.max(h).max(0) as u32;
                if w > 0
                    && h > 0
                    && edge >= max_edge
                    && edge < original_width.max(original_height)
                    && (0.99..=1.01).contains(&ratio)
                    && best.is_none_or(|(_, old)| edge < old)
                {
                    if let Some((old, _)) = best.replace((handle, edge)) {
                        (opened.api.release_handle)(old);
                    }
                } else {
                    (opened.api.release_handle)(handle);
                }
            }
        }
        if let Some((handle, _)) = best {
            (opened.api.release_handle)(opened.handle);
            opened.handle = handle;
        }
    }
    let (width, height) = opened.dimensions()?;
    struct ImageGuard<'a> {
        pointer: *mut c_void,
        api: &'a Api,
    }
    impl Drop for ImageGuard<'_> {
        fn drop(&mut self) {
            if !self.pointer.is_null() {
                unsafe {
                    (self.api.release_image)(self.pointer);
                }
            }
        }
    }
    let mut decoded = ImageGuard {
        pointer: ptr::null_mut(),
        api: &opened.api,
    };
    unsafe {
        // RGB + interleaved RGBA, default options apply HEIF crop/mirror/rotation.
        if let Err(error) =
            (opened.api.decode)(opened.handle, &mut decoded.pointer, 1, 11, ptr::null()).check()
        {
            if best.is_some() {
                return decode_inner(path, max_edge, directory, false);
            }
            return Err(error);
        }
        anyhow::ensure!(!decoded.pointer.is_null(), "HEIC decode returned no image");
        let dw = (opened.api.image_width)(decoded.pointer, 10);
        let dh = (opened.api.image_height)(decoded.pointer, 10);
        anyhow::ensure!(
            dw == width as i32 && dh == height as i32,
            "HEIC decoded dimensions differ from image handle"
        );
        let mut stride = 0;
        let pixels = (opened.api.plane)(decoded.pointer, 10, &mut stride);
        anyhow::ensure!(
            !pixels.is_null() && stride >= dw * 4,
            "HEIC RGBA plane invalid"
        );
        let mut rgb = RgbImage::new(width, height);
        for y in 0..height as usize {
            let row =
                std::slice::from_raw_parts(pixels.add(y * stride as usize), width as usize * 4);
            for (x, pixel) in row.chunks_exact(4).enumerate() {
                let alpha = u32::from(pixel[3]);
                let out = rgb.get_pixel_mut(x as u32, y as u32);
                for channel in 0..3 {
                    out[channel] = ((u32::from(pixel[channel]) * alpha + 255 * (255 - alpha) + 127)
                        / 255) as u8;
                }
            }
        }
        // HEIF item transformations already applied; do not apply EXIF twice.
        Ok(DecodedPreview {
            source: if best.is_some() {
                "heic_thumbnail"
            } else {
                "heic_primary"
            },
            image: resize_preview(rgb, max_edge),
            original_width,
            original_height,
            orientation: 1,
        })
    }
}
