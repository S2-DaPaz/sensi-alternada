//! Motor "Sem marca", por cima do driver do RawAccel.
//!
//! O RawAccel e um driver de filtro que transforma as contagens do mouse antes do sistema
//! ve-las. Isso serve para qualquer marca, e e por isso que ele e o candidato natural do
//! caminho generico.
//!
//! Ele tem, porem, um limite que vem do proprio fonte e nao de uma opiniao. Em
//! `common/rawaccel-io-def.h` existem tres IOCTLs e nada mais: `READ`, `WRITE` e
//! `GET_VERSION`. Nao ha liga-desliga rapido nem canal separado de sensibilidade. E em
//! `driver/driver.cpp`, o `case ra::WRITE` chama `WriteDelay()` **antes de ler o buffer**:
//!
//! ```text
//! interval.QuadPart = static_cast<LONGLONG>(ra::WRITE_DELAY) * -10000;
//! KeDelayExecutionThread(KernelMode, FALSE, &interval);   // WRITE_DELAY = 1000
//! ```
//!
//! Um segundo por escrita, no kernel, incondicional, e de proposito: a FAQ oficial diz
//! *"it is fully signed and has a one-second delay on write, so it cannot be used to
//! cheat"*.
//!
//! Consequencia pratica: segurar o gatilho mudaria a sensibilidade **um segundo depois**,
//! e solta-lo a devolveria outro segundo depois. Numa troca de tiro isso chega quando a
//! rajada ja acabou. Por isso este motor **detecta** o driver e diz o que encontrou, em vez
//! de fingir que atende.

use windows::Win32::Foundation::CloseHandle;
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_NONE, OPEN_EXISTING,
};
use windows::core::PCWSTR;

/// Nome do dispositivo que o driver expoe, igual ao usado em `rawaccel-io.hpp`.
const DEVICE: &str = r"\.\rawaccel";

/// Atraso que o driver aplica em toda escrita de configuracao, em milissegundos.
pub const WRITE_DELAY_MS: u32 = 1000;

pub struct RawAccel {
    pub installed: bool,
}

impl RawAccel {
    pub fn detect() -> RawAccel {
        RawAccel {
            installed: Self::device_present(),
        }
    }

    fn device_present() -> bool {
        let name: Vec<u16> = DEVICE.encode_utf16().chain(std::iter::once(0)).collect();
        unsafe {
            match CreateFileW(
                PCWSTR(name.as_ptr()),
                0,
                FILE_SHARE_NONE,
                None,
                OPEN_EXISTING,
                FILE_FLAGS_AND_ATTRIBUTES(0),
                None,
            ) {
                Ok(handle) => {
                    let _ = CloseHandle(handle);
                    true
                }
                Err(_) => false,
            }
        }
    }

    pub fn describe(&self) -> String {
        if self.installed {
            format!(
                "RawAccel instalado, mas ele atrasa {} ms toda escrita, por design",
                WRITE_DELAY_MS
            )
        } else {
            "driver RawAccel nao instalado".to_string()
        }
    }

    /// Nao troca nada, e diz por que. Executar a escrita real bloquearia o laco de
    /// mensagens por um segundo a cada clique.
    pub fn set_dpi(&self, _dpi: u16) -> Result<(), String> {
        if !self.installed {
            return Err("driver RawAccel nao instalado".into());
        }
        Err(format!(
            "RawAccel atrasa {WRITE_DELAY_MS} ms por escrita — chegaria depois da rajada"
        ))
    }
}
