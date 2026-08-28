//! Qual marca de mouse, e por qual caminho a DPI muda.
//!
//! Cada fabricante tem protocolo proprio: nao existe comando HID padronizado para DPI.
//! Acrescentar marca e acrescentar uma variante aqui e um motor que a atenda.

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Brand {
    Logitech,
    /// Chipset SINOWEALTH (VID 258a) — o de boa parte dos mouses de jogo baratos e sem
    /// marca. E o fabricante do chip, nao do mouse.
    Sinowealth,
    SemMarca,
}

impl Default for Brand {
    fn default() -> Self {
        Brand::Logitech
    }
}

impl Brand {
    pub const ALL: [Brand; 3] = [Brand::Logitech, Brand::Sinowealth, Brand::SemMarca];

    pub fn label(self) -> &'static str {
        match self {
            Brand::Logitech => "Logitech",
            Brand::Sinowealth => "Genérico (SINOWEALTH)",
            Brand::SemMarca => "Sem marca",
        }
    }

    /// Como a DPI muda nessa marca. Aparece no painel para o usuario saber o que esperar.
    pub fn method(self) -> &'static str {
        match self {
            Brand::Logitech => "HID++ direto no mouse",
            Brand::Sinowealth => "troca de perfil no mouse",
            Brand::SemMarca => "driver RawAccel",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// Duas marcas com o mesmo rotulo no seletor sao indistinguiveis para quem escolhe, e
    /// e o erro que uma lista copiada e colada produz.
    #[test]
    fn every_brand_has_a_distinct_label_and_method() {
        let labels: HashSet<&str> = Brand::ALL.iter().map(|b| b.label()).collect();
        assert_eq!(labels.len(), Brand::ALL.len(), "rotulo repetido");
        let methods: HashSet<&str> = Brand::ALL.iter().map(|b| b.method()).collect();
        assert_eq!(methods.len(), Brand::ALL.len(), "metodo repetido");
    }

    #[test]
    fn all_lists_every_variant() {
        // Se uma variante nova nao entrar em ALL, ela nunca aparece no seletor.
        for brand in [Brand::Logitech, Brand::Sinowealth, Brand::SemMarca] {
            assert!(Brand::ALL.contains(&brand), "{brand:?} fora de ALL");
        }
    }
}
