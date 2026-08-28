//! Escolhe o motor de DPI pela marca. Acrescentar marca e acrescentar um braco aqui.

use crate::brand::Brand;
use crate::mouse::Mouse;
use crate::rawaccel::RawAccel;

pub enum Engine {
    /// HID++, direto no mouse. Medido: 3,74 ms por troca.
    Logitech(Mouse),
    /// Driver de filtro, para qualquer marca — com a ressalva do atraso de 1 s.
    SemMarca(RawAccel),
    /// A marca escolhida nao encontrou dispositivo que atenda.
    Ausente(Brand),
}

impl Engine {
    pub fn for_brand(brand: Brand) -> Engine {
        match brand {
            Brand::Logitech => match Mouse::find() {
                Some(mouse) => Engine::Logitech(mouse),
                None => Engine::Ausente(brand),
            },
            Brand::SemMarca => Engine::SemMarca(RawAccel::detect()),
        }
    }

    /// Se o motor consegue mesmo trocar a DPI. O painel usa isto para habilitar o ATIVAR.
    pub fn usable(&self) -> bool {
        match self {
            Engine::Logitech(_) => true,
            // Instalado ou nao, o atraso de 1 s o torna inutil no gatilho.
            Engine::SemMarca(_) => false,
            Engine::Ausente(_) => false,
        }
    }

    pub fn per_axis(&self) -> bool {
        match self {
            Engine::Logitech(mouse) => mouse.per_axis,
            _ => false,
        }
    }

    pub fn set_dpi(&self, dpi: u16) -> Result<(), String> {
        match self {
            Engine::Logitech(mouse) => mouse.set_dpi(dpi),
            Engine::SemMarca(raw) => raw.set_dpi(dpi),
            Engine::Ausente(brand) => Err(format!("nenhum mouse {} encontrado", brand.label())),
        }
    }

    /// O que o painel mostra sobre o estado do motor.
    pub fn describe(&self) -> String {
        match self {
            Engine::Logitech(mouse) => match mouse.current_dpi() {
                Some(dpi) => format!("Logitech encontrado · DPI atual {dpi}"),
                None => "Logitech encontrado, mas nao respondeu a leitura de DPI".to_string(),
            },
            Engine::SemMarca(raw) => raw.describe(),
            Engine::Ausente(brand) => format!("nenhum mouse {} encontrado", brand.label()),
        }
    }
}
