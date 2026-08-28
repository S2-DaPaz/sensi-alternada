//! Enumeracao das colecoes HID da maquina. Compartilhada pelos motores de cada marca:
//! todos precisam achar o dispositivo antes de falar com ele, e so o protocolo difere.

use windows::Win32::Devices::DeviceAndDriverInstallation::{
    DIGCF_DEVICEINTERFACE, DIGCF_PRESENT, SP_DEVICE_INTERFACE_DATA,
    SP_DEVICE_INTERFACE_DETAIL_DATA_W, SetupDiEnumDeviceInterfaces, SetupDiGetClassDevsW,
    SetupDiGetDeviceInterfaceDetailW,
};
use windows::Win32::Devices::HumanInterfaceDevice::{
    HIDD_ATTRIBUTES, HIDP_CAPS, HidD_FreePreparsedData, HidD_GetAttributes, HidD_GetHidGuid,
    HidD_GetPreparsedData, HidP_GetCaps, PHIDP_PREPARSED_DATA,
};
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows::core::PCWSTR;

#[derive(Debug, Clone)]
pub struct Collection {
    pub path: String,
    pub vendor_id: u16,
    pub product_id: u16,
    pub usage_page: u16,
    pub usage: u16,
    pub input_len: u16,
    pub output_len: u16,
    pub feature_len: u16,
}

pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Todas as colecoes HID presentes, com identificacao e tamanhos de relatorio.
pub fn collections() -> Vec<Collection> {
    let mut found = Vec::new();
    unsafe {
        let guid = HidD_GetHidGuid();
        let Ok(set) = SetupDiGetClassDevsW(
            Some(&guid),
            PCWSTR::null(),
            None,
            DIGCF_PRESENT | DIGCF_DEVICEINTERFACE,
        ) else {
            return found;
        };

        let mut index = 0u32;
        loop {
            let mut data = SP_DEVICE_INTERFACE_DATA {
                cbSize: std::mem::size_of::<SP_DEVICE_INTERFACE_DATA>() as u32,
                ..Default::default()
            };
            if SetupDiEnumDeviceInterfaces(set, None, &guid, index, &mut data).is_err() {
                break;
            }
            index += 1;

            let mut needed = 0u32;
            let _ = SetupDiGetDeviceInterfaceDetailW(set, &data, None, 0, Some(&mut needed), None);
            if needed == 0 {
                continue;
            }
            let mut buffer = vec![0u8; needed as usize];
            let detail = buffer.as_mut_ptr() as *mut SP_DEVICE_INTERFACE_DETAIL_DATA_W;
            (*detail).cbSize = std::mem::size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>() as u32;
            if SetupDiGetDeviceInterfaceDetailW(
                set,
                &data,
                Some(detail),
                needed,
                Some(&mut needed),
                None,
            )
            .is_err()
            {
                continue;
            }
            let ptr = std::ptr::addr_of!((*detail).DevicePath) as *const u16;
            let mut len = 0usize;
            while *ptr.add(len) != 0 {
                len += 1;
            }
            let path = String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len));

            // Acesso zero: consulta capacidades sem disputar o dispositivo com ninguem.
            let Ok(handle) = CreateFileW(
                PCWSTR(wide(&path).as_ptr()),
                0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                FILE_FLAGS_AND_ATTRIBUTES(0),
                None,
            ) else {
                continue;
            };

            let mut attrs = HIDD_ATTRIBUTES {
                Size: std::mem::size_of::<HIDD_ATTRIBUTES>() as u32,
                ..Default::default()
            };
            let got_attrs = HidD_GetAttributes(handle, &mut attrs);

            let mut pre: PHIDP_PREPARSED_DATA = Default::default();
            if HidD_GetPreparsedData(handle, &mut pre) {
                let mut caps = HIDP_CAPS::default();
                if HidP_GetCaps(pre, &mut caps).is_ok() {
                    found.push(Collection {
                        path: path.clone(),
                        vendor_id: if got_attrs { attrs.VendorID } else { 0 },
                        product_id: if got_attrs { attrs.ProductID } else { 0 },
                        usage_page: caps.UsagePage,
                        usage: caps.Usage,
                        input_len: caps.InputReportByteLength,
                        output_len: caps.OutputReportByteLength,
                        feature_len: caps.FeatureReportByteLength,
                    });
                }
                let _ = HidD_FreePreparsedData(pre);
            }
            let _ = CloseHandle(handle);
        }
    }
    found
}
