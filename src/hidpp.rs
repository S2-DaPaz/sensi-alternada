//! HID++ 2.0 — o protocolo que o software da Logitech usa para falar com o mouse.
//!
//! A parte de baixo (abrir o dispositivo, ler e escrever) precisa de hardware. O
//! enquadramento das mensagens não precisa, e é onde os erros moram: o byte de função
//! empacota dois campos num só, e casar resposta pelo critério errado defasa a conversa
//! inteira em um pedido, para sempre.

/// Identificador de software: qualquer nibble não-zero. Vai no byte de função e volta
/// intacto na resposta.
pub const SW_ID: u8 = 0x0A;

/// Feature "Adjustable DPI" do HID++ 2.0.
pub const FEATURE_ADJUSTABLE_DPI: u16 = 0x2201;

/// Relatório longo. No G203 medido, a coleção short aceita a escrita e nunca responde.
pub const REPORT_LONG: u8 = 0x11;

/// O que uma resposta significa para o pedido que está esperando.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    /// É a resposta deste pedido.
    Mine,
    /// O dispositivo recusou este pedido, com este código.
    Failed(u8),
    /// É de outro pedido — resto de buffer, ou resposta atrasada.
    NotMine,
}

/// Monta um pedido HID++ do tamanho exato do relatório de saída.
pub fn build_request(
    report_len: usize,
    device: u8,
    feature: u8,
    func: u8,
    params: &[u8],
) -> Vec<u8> {
    let mut report = vec![0u8; report_len];
    report[0] = REPORT_LONG;
    report[1] = device;
    report[2] = feature;
    report[3] = (func << 4) | SW_ID;
    for (i, p) in params.iter().enumerate() {
        report[4 + i] = *p;
    }
    report
}

/// Classifica uma resposta contra o pedido que a espera.
pub fn classify(response: &[u8], feature: u8, func: u8) -> Answer {
    if response.len() < 6 || (response[3] & 0x0F) != SW_ID {
        return Answer::NotMine;
    }
    let func_sw = (func << 4) | SW_ID;
    // Recusa: feature 0xFF, e o byte 4 ecoa a funcao que falhou.
    if response[2] == 0xFF {
        return if response[4] == func_sw {
            Answer::Failed(response[5])
        } else {
            Answer::NotMine
        };
    }
    if response[2] == feature && response[3] == func_sw {
        Answer::Mine
    } else {
        Answer::NotMine
    }
}

/// DPI atual, lida de uma resposta de `getSensorDpi`.
pub fn parse_dpi(response: &[u8]) -> Option<u16> {
    // [.. , 4] indice do sensor, [5..6] DPI atual big-endian, [7..8] padrao do sensor.
    if response.len() < 7 {
        return None;
    }
    Some(u16::from_be_bytes([response[5], response[6]]))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Bytes reais capturados de um G203 em 28/08/2026.
    const GET_SENSOR_DPI: [u8; 10] = [0x11, 0xFF, 0x0A, 0x2A, 0x00, 0x06, 0x40, 0x03, 0x20, 0x00];

    #[test]
    fn function_and_software_id_share_one_byte() {
        let pedido = build_request(20, 0xFF, 0x0A, 3, &[0x00, 0x06, 0x40]);
        assert_eq!(pedido.len(), 20, "o relatório vai do tamanho exato");
        assert_eq!(pedido[3], 0x3A, "função no nibble alto, swId no baixo");
    }

    /// O bug que defasou a conversa inteira: o swId é constante, então sozinho ele casa
    /// qualquer resposta com qualquer pedido.
    #[test]
    fn a_response_to_another_function_is_not_mine() {
        // Resposta de getFeature (função 0) chegando enquanto getSensorDpi (função 2) espera.
        let get_feature = [0x11, 0xFF, 0x00, 0x0A, 0x0A, 0x00, 0x01];
        assert_eq!(classify(&get_feature, 0x0A, 2), Answer::NotMine);
        assert_eq!(classify(&GET_SENSOR_DPI, 0x0A, 2), Answer::Mine);
    }

    #[test]
    fn a_refusal_is_recognised_by_the_function_it_echoes() {
        // feature 0xFF, byte 4 ecoa o funcSw que falhou, byte 5 traz o código.
        let recusa = [0x11, 0xFF, 0xFF, 0x0A, 0x3A, 0x02, 0x00];
        assert_eq!(classify(&recusa, 0x0A, 3), Answer::Failed(0x02));
        // A mesma recusa não pertence a um pedido de outra função.
        assert_eq!(classify(&recusa, 0x0A, 2), Answer::NotMine);
    }

    #[test]
    fn dpi_comes_from_the_two_bytes_after_the_sensor_index() {
        assert_eq!(parse_dpi(&GET_SENSOR_DPI), Some(1600));
    }
}
