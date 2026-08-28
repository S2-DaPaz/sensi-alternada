//! A conversa com o mouse. O enquadramento das mensagens vive em [`crate::hidpp`] e é
//! testado sem hardware; aqui é só abrir o dispositivo, escrever e ler.
//!
//! Toda E/S é sobreposta e com prazo. Em E/S síncrona, um relatório que o dispositivo não
//! aceita trava o processo para sempre — e "travou" não diz em que etapa parou.

use std::time::{Duration, Instant};

use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAG_OVERLAPPED, FILE_GENERIC_READ, FILE_GENERIC_WRITE, FILE_SHARE_READ,
    FILE_SHARE_WRITE, OPEN_EXISTING, ReadFile, WriteFile,
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
                PCWSTR(crate::hid::wide(path).as_ptr()),
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
    crate::hid::collections()
        .into_iter()
        .filter(|c| {
            c.path.to_lowercase().contains(VID_LOGITECH)
                && c.usage_page == VENDOR_USAGE_PAGE
                && c.usage == LONG_COLLECTION_USAGE
        })
        .map(|c| c.path)
        .collect()
}

/// Tamanhos dos relatórios, se o caminho for a coleção HID++ longa.
fn capabilities(path: &str) -> Option<(u16, u16)> {
    crate::hid::collections()
        .into_iter()
        .find(|c| {
            c.path == path && c.usage_page == VENDOR_USAGE_PAGE && c.usage == LONG_COLLECTION_USAGE
        })
        .map(|c| (c.input_len, c.output_len))
}
