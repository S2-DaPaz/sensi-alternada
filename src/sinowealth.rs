//! Protocolo SINOWEALTH — o chipset da maior parte dos mouses de jogo sem marca (VID
//! `258a`). Enquadramento puro, testado sem hardware; a conversa com o dispositivo fica em
//! [`crate::sinowealth_mouse`].
//!
//! Derivado de `libratbag/src/driver-sinowealth.c`.
//!
//! # A decisao que define o desenho
//!
//! Mudar o slot de DPI ativo mora dentro da configuracao, e escreve-la custa um *feature
//! report* de **520 bytes** que vai para a memoria persistente do mouse. Fazer isso a cada
//! tiro gastaria ciclos de gravacao do dispositivo — estragaria o mouse com o tempo.
//!
//! Por isso a alternancia se faz por **troca de perfil**, que e um comando de 6 bytes. Cada
//! perfil tem a sua propria lista de DPI e o seu proprio slot ativo. E o analogo do
//! `SetMouseDPITableIndex` da Logitech.

pub const REPORT_ID_CMD: u8 = 0x05;
pub const CMD_SIZE: usize = 6;
pub const CONFIG_REPORT_SIZE: usize = 520;

pub const CMD_PROFILE: u8 = 0x02;
/// Le a configuracao do perfil 1, 2 e 3 respectivamente.
pub const CMD_GET_CONFIG: [u8; 3] = [0x11, 0x21, 0x31];
/// Id do relatorio de configuracao. Mouses "longos" usam 0x06.
pub const REPORT_ID_CONFIG: u8 = 0x04;
pub const REPORT_ID_CONFIG_LONG: u8 = 0x06;

pub const NUM_DPI_SLOTS: usize = 8;
pub const NUM_PROFILES_MAX: usize = 3;

/// Deslocamentos dentro do relatorio de configuracao, contados da struct de referencia:
/// report_id(1) command_id(1) unknown1(1) config_write(1) unknown2(5) = 9 bytes antes do
/// sensor.
const OFF_SENSOR: usize = 9;
const OFF_DPI_COUNTS: usize = 11;
const OFF_DISABLED_SLOTS: usize = 12;
const OFF_DPIS: usize = 13;

/// Sensores conhecidos. Dois deles deslocam a escala de DPI em um passo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sensor {
    Pmw3360,
    Pmw3212,
    Pmw3327,
    Pmw3389,
    Desconhecido(u8),
}

impl Sensor {
    pub fn from_byte(b: u8) -> Sensor {
        match b {
            0x06 => Sensor::Pmw3360,
            0x08 => Sensor::Pmw3212,
            0x0e => Sensor::Pmw3327,
            0x0f => Sensor::Pmw3389,
            other => Sensor::Desconhecido(other),
        }
    }

    /// PMW3327 e PMW3360 contam a partir de 1, os demais a partir de 0.
    fn offset(self) -> u32 {
        match self {
            Sensor::Pmw3360 | Sensor::Pmw3327 => 1,
            _ => 0,
        }
    }
}

/// Passo do sensor, em DPI.
pub const DPI_STEP: u32 = 100;

pub fn raw_to_dpi(raw: u8, sensor: Sensor) -> u32 {
    (raw as u32 + sensor.offset()) * DPI_STEP
}

/// O que interessa da configuracao de um perfil.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub sensor: Sensor,
    /// Quantos slots de DPI o perfil declara.
    pub dpi_count: u8,
    /// Slot ativo, contando a partir de 1 e so entre os habilitados.
    pub active_dpi: u8,
    /// DPI de cada slot habilitado, na ordem.
    pub dpis: Vec<u32>,
}

/// Le a configuracao crua de um perfil. `None` se o buffer nao tiver o tamanho esperado.
pub fn parse_config(raw: &[u8]) -> Option<Config> {
    if raw.len() < OFF_DPIS + NUM_DPI_SLOTS {
        return None;
    }
    let sensor = Sensor::from_byte(raw[OFF_SENSOR]);
    // Bitfield de um byte: o campo declarado primeiro ocupa os bits menos significativos.
    let dpi_count = raw[OFF_DPI_COUNTS] & 0x0F;
    let active_dpi = raw[OFF_DPI_COUNTS] >> 4;
    let disabled = raw[OFF_DISABLED_SLOTS];

    let mut dpis = Vec::new();
    for slot in 0..NUM_DPI_SLOTS {
        // Bit ligado significa desabilitado.
        if disabled & (1 << slot) != 0 {
            continue;
        }
        dpis.push(raw_to_dpi(raw[OFF_DPIS + slot], sensor));
    }

    Some(Config {
        sensor,
        dpi_count,
        active_dpi,
        dpis,
    })
}

/// Comando de 6 bytes que torna ativo o perfil de indice `index`, contado de zero.
pub fn build_profile_command(index: usize) -> [u8; CMD_SIZE] {
    let mut buf = [0u8; CMD_SIZE];
    buf[0] = REPORT_ID_CMD;
    buf[1] = CMD_PROFILE;
    buf[2] = (index as u8).saturating_add(1);
    buf
}

/// Comando que pede a configuracao do perfil `index`, contado de zero.
pub fn build_config_request(index: usize) -> Option<[u8; CMD_SIZE]> {
    let cmd = *CMD_GET_CONFIG.get(index)?;
    let mut buf = [0u8; CMD_SIZE];
    buf[0] = REPORT_ID_CMD;
    buf[1] = cmd;
    Some(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config_de_teste() -> Vec<u8> {
        let mut raw = vec![0u8; CONFIG_REPORT_SIZE];
        raw[OFF_SENSOR] = 0x06; // PMW3360
        raw[OFF_DPI_COUNTS] = 0x26; // dpi_count = 6 nos bits baixos, active_dpi = 2 nos altos
        raw[OFF_DISABLED_SLOTS] = 0b1100_0000; // slots 6 e 7 desabilitados
        for (slot, raw_dpi) in [7u8, 15, 23, 31, 39, 47].into_iter().enumerate() {
            raw[OFF_DPIS + slot] = raw_dpi;
        }
        raw
    }

    #[test]
    fn profile_command_is_six_bytes_and_counts_from_one() {
        // O dispositivo conta perfis a partir de 1; passar o indice cru selecionaria o
        // perfil errado, ou nenhum.
        assert_eq!(build_profile_command(0), [0x05, 0x02, 0x01, 0, 0, 0]);
        assert_eq!(build_profile_command(1), [0x05, 0x02, 0x02, 0, 0, 0]);
    }

    #[test]
    fn count_and_active_slot_share_one_byte() {
        let config = parse_config(&config_de_teste()).unwrap();
        assert_eq!(config.dpi_count, 6, "contagem vem dos bits baixos");
        assert_eq!(config.active_dpi, 2, "slot ativo vem dos bits altos");
    }

    #[test]
    fn disabled_slots_are_left_out_of_the_dpi_list() {
        let config = parse_config(&config_de_teste()).unwrap();
        assert_eq!(config.dpis.len(), 6, "dois slots estavam desabilitados");
    }

    /// PMW3360 e PMW3327 contam a partir de 1: ler sem o deslocamento erra 100 DPI em
    /// todos os valores, silenciosamente.
    #[test]
    fn two_sensors_shift_the_dpi_scale_by_one_step() {
        assert_eq!(raw_to_dpi(7, Sensor::Pmw3360), 800);
        assert_eq!(raw_to_dpi(7, Sensor::Pmw3327), 800);
        assert_eq!(raw_to_dpi(8, Sensor::Pmw3212), 800);
        assert_eq!(raw_to_dpi(8, Sensor::Pmw3389), 800);
    }

    #[test]
    fn a_short_buffer_is_refused_instead_of_read_out_of_bounds() {
        assert_eq!(parse_config(&[0u8; 8]), None);
    }
}
