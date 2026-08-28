//! Conversa com mouses de chipset SINOWEALTH, por *feature report*.
//!
//! ⚠ **Nao verificado em hardware.** O enquadramento e testado byte a byte contra o
//! `driver-sinowealth.c` do libratbag, mas nenhum mouse SINOWEALTH passou por aqui. As
//! falhas sao tratadas como recusa, nunca como sucesso silencioso.
//!
//! O desenho evita de proposito o caminho que estragaria o mouse: trocar o slot de DPI
//! ativo exigiria reescrever 520 bytes na memoria persistente a cada tiro. Em vez disso,
//! **troca-se de perfil** com um comando de 6 bytes. Mouse que so expuser um perfil e
//! recusado, com a razao dita — e melhor que gastar a memoria dele.

use windows::Win32::Devices::HumanInterfaceDevice::{HidD_GetFeature, HidD_SetFeature};
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAGS_AND_ATTRIBUTES, FILE_GENERIC_READ, FILE_GENERIC_WRITE, FILE_SHARE_READ,
    FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows::core::PCWSTR;

use crate::hid::{collections, wide};
use crate::sinowealth::{
    CMD_SIZE, CONFIG_REPORT_SIZE, Config, NUM_PROFILES_MAX, REPORT_ID_CONFIG,
    REPORT_ID_CONFIG_LONG, build_config_request, build_profile_command, parse_config,
};

/// SINOWEALTH. E o fabricante do chip, nao do mouse — por isso aparece em dezenas de
/// marcas diferentes e em mouses sem marca nenhuma.
pub const VID_SINOWEALTH: u16 = 0x258A;

pub struct SinowealthMouse {
    handle: HANDLE,
    feature_len: usize,
    /// Identifica o modelo. Vale ouro num dispositivo que nao pude testar: com ele o
    /// usuario consegue procurar o mouse no libratbag ou abrir uma issue nomeando-o.
    product_id: u16,
    /// Perfis que responderam com configuracao valida.
    pub profiles: Vec<Config>,
}

// O handle e usado so pela thread do motor, que e dona da struct.
unsafe impl Send for SinowealthMouse {}

impl SinowealthMouse {
    pub fn find() -> Option<SinowealthMouse> {
        for collection in collections() {
            if collection.vendor_id != VID_SINOWEALTH || collection.feature_len == 0 {
                continue;
            }
            let Some(mut mouse) = SinowealthMouse::open(
                &collection.path,
                collection.feature_len,
                collection.product_id,
            ) else {
                continue;
            };
            mouse.profiles = mouse.read_profiles();
            if !mouse.profiles.is_empty() {
                return Some(mouse);
            }
        }
        None
    }

    fn open(path: &str, feature_len: u16, product_id: u16) -> Option<SinowealthMouse> {
        unsafe {
            let handle = CreateFileW(
                PCWSTR(wide(path).as_ptr()),
                (FILE_GENERIC_READ | FILE_GENERIC_WRITE).0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                FILE_FLAGS_AND_ATTRIBUTES(0),
                None,
            )
            .ok()?;
            Some(SinowealthMouse {
                handle,
                feature_len: feature_len as usize,
                product_id,
                profiles: Vec::new(),
            })
        }
    }

    /// Le a configuracao de cada perfil que o mouse expuser.
    fn read_profiles(&self) -> Vec<Config> {
        let mut found = Vec::new();
        for index in 0..NUM_PROFILES_MAX {
            let Some(request) = build_config_request(index) else {
                break;
            };
            if self.set_feature(&request).is_err() {
                break;
            }
            // Mouses "curtos" respondem no relatório 0x04 e os "longos" no 0x06. Fixar
            // um só faria metade dos dispositivos falhar na leitura, sem erro nenhum.
            let Some(config) = [REPORT_ID_CONFIG, REPORT_ID_CONFIG_LONG]
                .into_iter()
                .filter_map(|id| self.get_feature(id, CONFIG_REPORT_SIZE))
                .find_map(|raw| parse_config(raw.as_slice()))
            else {
                break;
            };
            // Um perfil sem slot de DPI algum nao existe de fato.
            if config.dpis.is_empty() {
                break;
            }
            found.push(config);
        }
        found
    }

    /// Quais DPIs o mouse consegue entregar, por perfil.
    pub fn dpi_of_profile(&self, index: usize) -> Option<u32> {
        let config = self.profiles.get(index)?;
        let slot = config.active_dpi.checked_sub(1)? as usize;
        config.dpis.get(slot).copied()
    }

    /// Alternar exige dois perfis: e a unica troca que nao grava na memoria do mouse.
    pub fn can_switch(&self) -> bool {
        self.profiles.len() >= 2
    }

    pub fn select_profile(&self, index: usize) -> Result<(), String> {
        if index >= self.profiles.len() {
            return Err(format!("perfil {} nao existe neste mouse", index + 1));
        }
        self.set_feature(&build_profile_command(index))
    }

    fn set_feature(&self, command: &[u8; CMD_SIZE]) -> Result<(), String> {
        // O Windows exige o buffer do tamanho declarado pela colecao, com o id do
        // relatorio no primeiro byte; o roteamento e feito por esse id.
        let mut buffer = vec![0u8; self.feature_len.max(CMD_SIZE)];
        buffer[..CMD_SIZE].copy_from_slice(command);
        unsafe {
            if HidD_SetFeature(
                self.handle,
                buffer.as_ptr() as *const _,
                buffer.len() as u32,
            ) {
                Ok(())
            } else {
                Err("o mouse nao aceitou o comando".into())
            }
        }
    }

    fn get_feature(&self, report_id: u8, expected: usize) -> Option<Vec<u8>> {
        let mut buffer = vec![0u8; self.feature_len.max(expected)];
        // O id do relatório pedido vai no primeiro byte da leitura.
        buffer[0] = report_id;
        unsafe {
            if HidD_GetFeature(
                self.handle,
                buffer.as_mut_ptr() as *mut _,
                buffer.len() as u32,
            ) {
                Some(buffer)
            } else {
                None
            }
        }
    }

    pub fn describe(&self) -> String {
        let id = format!("258a:{:04x}", self.product_id);
        match self.profiles.len() {
            0 => format!("SINOWEALTH {id} sem perfil legível"),
            1 => format!("SINOWEALTH {id} com um perfil só — alternar exige dois"),
            n => format!("SINOWEALTH {id}, {n} perfis"),
        }
    }
}

impl Drop for SinowealthMouse {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.handle);
        }
    }
}
