//! A conversa com o mouse. O enquadramento das mensagens vive em [`crate::hidpp`] e é
//! testado sem hardware; aqui é só abrir o dispositivo, escrever e ler.
//!
//! Toda E/S é sobreposta e com prazo. Em E/S síncrona, um relatório que o dispositivo não
//! aceita trava o processo para sempre — e "travou" não diz em que etapa parou.

use std::time::{Duration, Instant};

use windows::Win32::Devices::DeviceAndDriverInstallation::{
    DIGCF_DEVICEINTERFACE, DIGCF_PRESENT, SP_DEVICE_INTERFACE_DATA,
    SP_DEVICE_INTERFACE_DETAIL_DATA_W, SetupDiEnumDeviceInterfaces, SetupDiGetClassDevsW,
    SetupDiGetDeviceInterfaceDetailW,
};
use windows::Win32::Devices::HumanInterfaceDevice::{
    HIDP_CAPS, HidD_FreePreparsedData, HidD_GetHidGuid, HidD_GetPreparsedData, HidP_GetCaps,
    PHIDP_PREPARSED_DATA,
};
use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAG_OVERLAPPED, FILE_FLAGS_AND_ATTRIBUTES, FILE_GENERIC_READ,
    FILE_GENERIC_WRITE, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING, ReadFile, WriteFile,
};
use windows::Win32::System::IO::{CancelIoEx, GetOverlappedResult, OVERLAPPED};
use windows::Win32::System::Threading::{CreateEventW, WaitForSingleObject};
use windows::core::PCWSTR;

use crate::hidpp::{Answer, FEATURE_ADJUSTABLE_DPI, build_request, classify, parse_dpi};

/// HID++ é o protocolo da Logitech. Outras marcas têm o seu, e nenhum é coberto aqui.
const VID_LOGITECH: &str = "vid_046d";
/// Página de uso reservada ao fabricante, onde o HID++ vive.
const VENDOR_USAGE_PAGE: u16 = 0xFF00;
/// Coleção longa. A curta aceita a escrita e nunca responde — medido no G203.
const LONG_COLLECTION_USAGE: u16 = 0x02;
const ROOT_FEATURE: u8 = 0x00;
/// DPI separada por eixo. Ausente no G203, e é o que decide se X e Y podem divergir.
const FEATURE_EXTENDED_DPI: u16 = 0x2202;
const ERROR_IO_PENDING: u32 = 997;
/// Dispositivo ligado direto, sem receptor.
const DIRECT_DEVICE: u8 = 0xFF;

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub struct Mouse {
    handle: HANDLE,
    input_len: usize,
    output_len: usize,
    dpi_feature: u8,
    /// Verdadeiro só se o mouse tiver a feature 0x2202.
    pub per_axis: bool,
}

// O handle é usado só pela thread do hook, que é dona do Mouse.
unsafe impl Send for Mouse {}

impl Mouse {
    /// Procura um mouse Logitech que fale HID++ e saiba trocar DPI.
    pub fn find() -> Option<Mouse> {
        for path in vendor_collections() {
            let Some(mut mouse) = Mouse::open(&path) else {
                continue;
            };
            let Ok(answer) = mouse.root_feature(FEATURE_ADJUSTABLE_DPI) else {
                continue;
            };
            if answer == 0 {
                continue;
            }
            mouse.dpi_feature = answer;
            mouse.per_axis = mouse.root_feature(FEATURE_EXTENDED_DPI).unwrap_or(0) != 0;
            return Some(mouse);
        }
        None
    }

    fn root_feature(&self, feature_id: u16) -> Result<u8, String> {
        let hi = (feature_id >> 8) as u8;
        let lo = (feature_id & 0xFF) as u8;
        let answer = self.request(ROOT_FEATURE, 0, &[hi, lo])?;
        Ok(answer[4])
    }

    fn open(path: &str) -> Option<Mouse> {
        let (input_len, output_len) = capabilities(path)?;
        unsafe {
            let handle = CreateFileW(
                PCWSTR(wide(path).as_ptr()),
                (FILE_GENERIC_READ | FILE_GENERIC_WRITE).0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                FILE_FLAG_OVERLAPPED,
                None,
            )
            .ok()?;
            Some(Mouse {
                handle,
                input_len: input_len as usize,
                output_len: output_len as usize,
                dpi_feature: 0,
                per_axis: false,
            })
        }
    }

    pub fn current_dpi(&self) -> Option<u16> {
        let answer = self.request(self.dpi_feature, 2, &[0x00]).ok()?;
        parse_dpi(&answer)
    }

    pub fn set_dpi(&self, dpi: u16) -> Result<(), String> {
        let bytes = dpi.to_be_bytes();
        self.request(self.dpi_feature, 3, &[0x00, bytes[0], bytes[1]])
            .map(|_| ())
    }

    fn request(&self, feature: u8, func: u8, params: &[u8]) -> Result<Vec<u8>, String> {
        // Drena respostas velhas. Sem isto cada pedido casa com a resposta do ANTERIOR e a
        // defasagem nunca mais se corrige.
        while self.read_timeout(0).is_some() {}

        let report = build_request(self.output_len, DIRECT_DEVICE, feature, func, params);
        self.write_timeout(&report, 200)?;

        let deadline = Instant::now() + Duration::from_millis(250);
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Err("sem resposta do mouse".into());
            }
            let Some(answer) = self.read_timeout(left.as_millis() as u32) else {
                return Err("leitura do mouse falhou".into());
            };
            match classify(&answer, feature, func) {
                Answer::Mine => return Ok(answer),
                Answer::Failed(code) => return Err(format!("mouse recusou (0x{code:02X})")),
                Answer::NotMine => continue,
            }
        }
    }

    fn write_timeout(&self, buffer: &[u8], ms: u32) -> Result<(), String> {
        unsafe {
            let event =
                CreateEventW(None, true, false, PCWSTR::null()).map_err(|e| e.to_string())?;
            let mut ov = OVERLAPPED {
                hEvent: event,
                ..Default::default()
            };
            if let Err(e) = WriteFile(self.handle, Some(buffer), None, Some(&mut ov)) {
                if (e.code().0 as u32) & 0xFFFF != ERROR_IO_PENDING {
                    let _ = CloseHandle(event);
                    return Err(format!("escrita: {e}"));
                }
            }
            if WaitForSingleObject(event, ms) != WAIT_OBJECT_0 {
                let _ = CancelIoEx(self.handle, Some(&ov));
                let _ = CloseHandle(event);
                return Err("mouse nao aceitou o comando".into());
            }
            let mut moved = 0u32;
            let ok = GetOverlappedResult(self.handle, &ov, &mut moved, false);
            let _ = CloseHandle(event);
            ok.map_err(|e| e.to_string())?;
            Ok(())
        }
    }

    fn read_timeout(&self, ms: u32) -> Option<Vec<u8>> {
        unsafe {
            let event = CreateEventW(None, true, false, PCWSTR::null()).ok()?;
            let mut ov = OVERLAPPED {
                hEvent: event,
                ..Default::default()
            };
            let mut buffer = vec![0u8; self.input_len];
            if let Err(e) = ReadFile(self.handle, Some(&mut buffer), None, Some(&mut ov)) {
                if (e.code().0 as u32) & 0xFFFF != ERROR_IO_PENDING {
                    let _ = CloseHandle(event);
                    return None;
                }
            }
            if WaitForSingleObject(event, ms) != WAIT_OBJECT_0 {
                let _ = CancelIoEx(self.handle, Some(&ov));
                let _ = CloseHandle(event);
                return None;
            }
            let mut moved = 0u32;
            let ok = GetOverlappedResult(self.handle, &ov, &mut moved, false).is_ok();
            let _ = CloseHandle(event);
            if !ok {
                return None;
            }
            buffer.truncate(moved as usize);
            Some(buffer)
        }
    }
}

/// Caminhos das coleções HID++ longas dos dispositivos Logitech presentes.
fn vendor_collections() -> Vec<String> {
    let mut paths = Vec::new();
    unsafe {
        let guid = HidD_GetHidGuid();
        let Ok(set) = SetupDiGetClassDevsW(
            Some(&guid),
            PCWSTR::null(),
            None,
            DIGCF_PRESENT | DIGCF_DEVICEINTERFACE,
        ) else {
            return paths;
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
            if path.to_lowercase().contains(VID_LOGITECH) && capabilities(&path).is_some() {
                paths.push(path);
            }
        }
    }
    paths
}

/// Tamanhos dos relatórios, se o caminho for a coleção HID++ longa.
fn capabilities(path: &str) -> Option<(u16, u16)> {
    unsafe {
        let handle = CreateFileW(
            PCWSTR(wide(path).as_ptr()),
            0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_FLAGS_AND_ATTRIBUTES(0),
            None,
        )
        .ok()?;
        let mut pre: PHIDP_PREPARSED_DATA = Default::default();
        let mut result = None;
        if HidD_GetPreparsedData(handle, &mut pre) {
            let mut caps = HIDP_CAPS::default();
            if HidP_GetCaps(pre, &mut caps).is_ok()
                && caps.UsagePage == VENDOR_USAGE_PAGE
                && caps.Usage == LONG_COLLECTION_USAGE
            {
                result = Some((caps.InputReportByteLength, caps.OutputReportByteLength));
            }
            let _ = HidD_FreePreparsedData(pre);
        }
        let _ = CloseHandle(handle);
        result
    }
}
