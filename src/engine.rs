//! Escolhe o motor de DPI pela marca. Acrescentar marca e acrescentar um braco aqui.
//!
//! O motor nao recebe "DPI 400": recebe **qual nivel aplicar**. A diferenca importa porque
//! nem toda marca define DPI arbitraria — o SINOWEALTH so troca de perfil, e cada perfil
//! carrega a DPI que ja tem. Quem sabe o que "atirando" significa e o motor, nao quem
//! aperta o gatilho.

use crate::brand::Brand;
use crate::mouse::Mouse;
use crate::rawaccel::RawAccel;
use crate::shared::SHARED;
use crate::sinowealth_mouse::SinowealthMouse;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Base,
    Atirando,
}

pub enum Engine {
    /// HID++, direto no mouse. Medido: 3,74 ms por troca.
    Logitech(Mouse),
    /// Troca de perfil por feature report. Nao verificado em hardware.
    Sinowealth(SinowealthMouse),
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
            Brand::Sinowealth => match SinowealthMouse::find() {
                Some(mouse) => Engine::Sinowealth(mouse),
                None => Engine::Ausente(brand),
            },
            Brand::SemMarca => Engine::SemMarca(RawAccel::detect()),
        }
    }

    /// Se o motor consegue mesmo alternar. O painel usa isto para habilitar o ATIVAR.
    pub fn usable(&self) -> bool {
        match self {
            Engine::Logitech(_) => true,
            // Um perfil so significaria reescrever 520 bytes na memoria do mouse a cada
            // tiro. Recusar e melhor que gastar o dispositivo.
            Engine::Sinowealth(mouse) => mouse.can_switch(),
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

    pub fn apply(&self, level: Level) -> Result<(), String> {
        match self {
            Engine::Logitech(mouse) => {
                let dpi = match level {
                    Level::Base => SHARED.dpi_base(),
                    Level::Atirando => SHARED.dpi_shooting(),
                };
                mouse.set_dpi(dpi)
            }
            Engine::Sinowealth(mouse) => {
                // Perfil 1 e a base, perfil 2 e o de tiro. Os valores vem do proprio mouse.
                let index = match level {
                    Level::Base => 0,
                    Level::Atirando => 1,
                };
                mouse.select_profile(index)
            }
            Engine::SemMarca(raw) => raw.set_dpi(0),
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
            Engine::Sinowealth(mouse) => {
                let base = mouse.dpi_of_profile(0);
                let tiro = mouse.dpi_of_profile(1);
                match (base, tiro) {
                    (Some(b), Some(t)) => {
                        format!("{} · perfis {b} e {t} DPI", mouse.describe())
                    }
                    _ => mouse.describe(),
                }
            }
            Engine::SemMarca(raw) => raw.describe(),
            Engine::Ausente(brand) => format!("nenhum mouse {} encontrado", brand.label()),
        }
    }

    /// Se as DPIs sao editaveis no painel. No SINOWEALTH elas vem dos perfis do mouse.
    pub fn dpi_editable(&self) -> bool {
        !matches!(self, Engine::Sinowealth(_))
    }
}
