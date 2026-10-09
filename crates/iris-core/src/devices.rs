//! Read-only DXGI inventory. Enumeration does not initialize ORT or test a model.
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Debug, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GpuEnumerationStatus {
    Available,
    Unavailable,
    Unsupported,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct GpuAdapter {
    /// Pass unchanged as AnalysisSettings.directml_device_id. Never reindex a filtered list.
    pub device_id: u32,
    pub name: String,
    pub vendor_id: u32,
    pub hardware_device_id: u32,
    /// DXGI adapter LUID, useful within the current Windows boot; not a persistent ID.
    pub luid: String,
    /// Adapter capacities, not current memory usage or free-memory budgets.
    pub dedicated_video_memory_bytes: u64,
    pub dedicated_system_memory_bytes: u64,
    pub shared_system_memory_bytes: u64,
    pub is_software: bool,
    pub is_remote: bool,
    /// D3D12 UMA query; absent when this adapter cannot create a D3D12 device.
    pub is_integrated: Option<bool>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct GpuDevices {
    pub status: GpuEnumerationStatus,
    /// "dxgi_enum_adapters" on Windows, "unsupported" elsewhere.
    pub source: String,
    /// DirectML's default index 0 when it exists, not a fastest-device recommendation.
    pub default_device_id: Option<u32>,
    pub adapters: Vec<GpuAdapter>,
    /// Always false: presence does not establish runtime/model compatibility or speed.
    pub inference_verified: bool,
    pub reason: Option<String>,
}

fn collect_adapters(
    mut next: impl FnMut(u32) -> anyhow::Result<Option<GpuAdapter>>,
) -> anyhow::Result<Vec<GpuAdapter>> {
    let mut adapters = Vec::new();
    for index in 0..1024 {
        match next(index)? {
            Some(adapter) => adapters.push(adapter),
            None => return Ok(adapters),
        }
    }
    anyhow::bail!("DXGI enumeration exceeded 1024 adapters")
}

fn inventory(result: anyhow::Result<Vec<GpuAdapter>>) -> GpuDevices {
    match result {
        Ok(adapters) => GpuDevices {
            status: GpuEnumerationStatus::Available,
            source: "dxgi_enum_adapters".into(),
            default_device_id: adapters
                .iter()
                .find(|a| a.device_id == 0)
                .map(|a| a.device_id),
            adapters,
            inference_verified: false,
            reason: None,
        },
        Err(error) => GpuDevices {
            status: GpuEnumerationStatus::Unavailable,
            source: "dxgi_enum_adapters".into(),
            default_device_id: None,
            adapters: vec![],
            inference_verified: false,
            reason: Some(format!("DXGI enumeration failed: {error:#}")),
        },
    }
}

/// Refresh each request; hot-plug/reboot can change indices. Does not change settings.
pub fn gpu_devices() -> GpuDevices {
    #[cfg(windows)]
    {
        inventory(windows_adapters())
    }
    #[cfg(not(windows))]
    {
        GpuDevices {
            status: GpuEnumerationStatus::Unsupported,
            source: "unsupported".into(),
            default_device_id: None,
            adapters: vec![],
            inference_verified: false,
            reason: Some("DirectML adapter enumeration requires Windows".into()),
        }
    }
}

/// Prefer a verified non-UMA hardware adapter, then UMA, without changing DXGI IDs.
pub fn preferred_auto_device(devices: &GpuDevices) -> Option<u32> {
    devices
        .adapters
        .iter()
        .filter(|adapter| {
            !adapter.is_software && !adapter.is_remote && adapter.is_integrated.is_some()
        })
        .max_by_key(|adapter| {
            (
                adapter.is_integrated == Some(false),
                adapter.dedicated_video_memory_bytes,
                std::cmp::Reverse(adapter.device_id),
            )
        })
        .map(|adapter| adapter.device_id)
}

#[cfg(windows)]
fn windows_adapters() -> anyhow::Result<Vec<GpuAdapter>> {
    use windows::{
        core::Interface,
        Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL_11_0,
        Win32::Graphics::Direct3D12::{
            D3D12CreateDevice, ID3D12Device, D3D12_FEATURE_ARCHITECTURE,
            D3D12_FEATURE_DATA_ARCHITECTURE,
        },
        Win32::Graphics::Dxgi::{
            CreateDXGIFactory1, IDXGIAdapter1, IDXGIFactory1, DXGI_ADAPTER_FLAG_REMOTE,
            DXGI_ADAPTER_FLAG_SOFTWARE, DXGI_ERROR_NOT_FOUND,
        },
    };
    // SAFETY: typed Windows bindings own/release every COM reference; objects
    // remain on this thread. Use EnumAdapters itself, matching ORT's device_id.
    let factory: IDXGIFactory1 = unsafe { CreateDXGIFactory1()? };
    collect_adapters(|index| {
        let adapter = match unsafe { factory.EnumAdapters(index) } {
            Ok(adapter) => adapter,
            Err(error) if error.code() == DXGI_ERROR_NOT_FOUND => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let adapter: IDXGIAdapter1 = adapter.cast()?;
        let desc = unsafe { adapter.GetDesc1()? };
        let mut device: Option<ID3D12Device> = None;
        // Only query hardware memory architecture; no ORT/model inference is performed.
        let is_integrated = if desc.Flags
            & (DXGI_ADAPTER_FLAG_SOFTWARE.0 | DXGI_ADAPTER_FLAG_REMOTE.0) as u32
            == 0
            && unsafe { D3D12CreateDevice(&adapter, D3D_FEATURE_LEVEL_11_0, &mut device) }.is_ok()
        {
            device.and_then(|device| {
                let mut architecture = D3D12_FEATURE_DATA_ARCHITECTURE::default();
                unsafe {
                    device.CheckFeatureSupport(
                        D3D12_FEATURE_ARCHITECTURE,
                        std::ptr::from_mut(&mut architecture).cast(),
                        std::mem::size_of_val(&architecture) as u32,
                    )
                }
                .ok()
                .map(|_| architecture.UMA.as_bool())
            })
        } else {
            None
        };
        let length = desc
            .Description
            .iter()
            .position(|c| *c == 0)
            .unwrap_or(desc.Description.len());
        Ok(Some(GpuAdapter {
            device_id: index,
            name: String::from_utf16_lossy(&desc.Description[..length]),
            vendor_id: desc.VendorId,
            hardware_device_id: desc.DeviceId,
            luid: format!(
                "{:08x}:{:08x}",
                desc.AdapterLuid.HighPart as u32, desc.AdapterLuid.LowPart
            ),
            dedicated_video_memory_bytes: desc.DedicatedVideoMemory as u64,
            dedicated_system_memory_bytes: desc.DedicatedSystemMemory as u64,
            shared_system_memory_bytes: desc.SharedSystemMemory as u64,
            is_software: desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32 != 0,
            is_remote: desc.Flags & DXGI_ADAPTER_FLAG_REMOTE.0 as u32 != 0,
            is_integrated,
        }))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn adapter(index: u32, software: bool) -> GpuAdapter {
        GpuAdapter {
            device_id: index,
            name: format!("adapter {index}"),
            vendor_id: 0,
            hardware_device_id: 0,
            luid: "00000000:00000000".into(),
            dedicated_video_memory_bytes: 0,
            dedicated_system_memory_bytes: 0,
            shared_system_memory_bytes: 0,
            is_software: software,
            is_remote: false,
            is_integrated: Some(true),
        }
    }

    #[test]
    fn software_entries_do_not_shift_directml_indices() {
        let result = inventory(collect_adapters(|index| {
            Ok(match index {
                0 => Some(adapter(index, true)),
                1 => Some(adapter(index, false)),
                _ => None,
            })
        }));
        assert_eq!(result.status, GpuEnumerationStatus::Available);
        assert_eq!(result.default_device_id, Some(0));
        assert!(result.adapters[0].is_software);
        assert_eq!(result.adapters[1].device_id, 1);
        assert!(!result.inference_verified);
    }

    #[test]
    fn driver_error_is_not_an_empty_success_or_partial_inventory() {
        let result = inventory(collect_adapters(|index| match index {
            0 => Ok(Some(adapter(index, false))),
            _ => anyhow::bail!("driver disconnected"),
        }));
        assert_eq!(result.status, GpuEnumerationStatus::Unavailable);
        assert!(result.adapters.is_empty());
        assert_eq!(result.default_device_id, None);
        assert!(result.reason.unwrap().contains("driver disconnected"));
    }

    #[test]
    fn no_adapters_has_no_default_device() {
        let result = inventory(collect_adapters(|_| Ok(None)));
        assert_eq!(result.status, GpuEnumerationStatus::Available);
        assert!(result.adapters.is_empty());
        assert_eq!(result.default_device_id, None);
    }
    #[test]
    fn auto_prefers_discrete_and_keeps_real_device_indices() {
        let mut integrated = adapter(2, false);
        integrated.dedicated_video_memory_bytes = 16_000;
        let mut discrete = adapter(4, false);
        discrete.is_integrated = Some(false);
        let mut software = adapter(0, true);
        software.is_integrated = Some(false);
        let devices = inventory(Ok(vec![software, integrated, discrete]));
        assert_eq!(preferred_auto_device(&devices), Some(4));
        assert_eq!(preferred_auto_device(&inventory(Ok(vec![]))), None);
    }
}
