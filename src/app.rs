//! O painel.

use std::sync::atomic::Ordering;
use std::time::Duration;

use eframe::egui;

use crate::brand::Brand;
use crate::config::Settings;
use crate::fire_button::FireButton;
use crate::hook;
use crate::shared::SHARED;
use crate::theme;

/// Semântico, não decorativo: verde é "agindo agora", qualquer que seja a paleta.
const LIVE: egui::Color32 = egui::Color32::from_rgb(102, 214, 142);
const WAITING: egui::Color32 = egui::Color32::from_rgb(214, 176, 102);
const TROUBLE: egui::Color32 = egui::Color32::from_rgb(226, 118, 118);

const HEIGHT_JOINED: f32 = 556.0;
const HEIGHT_SPLIT: f32 = 632.0;

struct Palette {
    background: egui::Color32,
    ink: egui::Color32,
    muted: egui::Color32,
    accent: egui::Color32,
    surface: egui::Color32,
    surface_hover: egui::Color32,
}

impl Palette {
    /// Duas escolhas comandam tudo: uma cor para os botões e as letras, outra para o
    /// fundo. O resto são misturas das duas, então o painel não desmonta.
    fn from(settings: &Settings) -> Self {
        Self {
            background: rgb(settings.background),
            ink: rgb(settings.accent),
            muted: mix(settings.background, settings.accent, 0.62),
            accent: rgb(settings.accent),
            surface: mix(settings.background, settings.accent, 0.12),
            surface_hover: mix(settings.background, settings.accent, 0.20),
        }
    }

    /// Texto **sobre** o botão preenchido. Não pode ser a cor de destaque — ela é o
    /// preenchimento —, então sai do brilho dela.
    fn on_accent(&self, accent: [u8; 3]) -> egui::Color32 {
        rgb(theme::ink_for(accent))
    }
}

fn rgb(c: [u8; 3]) -> egui::Color32 {
    egui::Color32::from_rgb(c[0], c[1], c[2])
}

fn mix(a: [u8; 3], b: [u8; 3], t: f32) -> egui::Color32 {
    let blend = |i: usize| (a[i] as f32 + (b[i] as f32 - a[i] as f32) * t).round() as u8;
    egui::Color32::from_rgb(blend(0), blend(1), blend(2))
}

pub struct App {
    settings: Settings,
    requested_height: f32,
    applied_colours: Option<([u8; 3], [u8; 3])>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let settings = Settings::load();
        cc.egui_ctx.set_theme(egui::ThemePreference::Dark);

        let app = Self {
            settings,
            requested_height: 0.0,
            applied_colours: None,
        };
        app.publish();
        // A thread do motor sobe antes do painel e monta o motor com a marca padrão.
        // Sem este pedido, a marca salva em disco era ignorada até o usuário mexer no
        // seletor — e o painel mostrava um motor que não era o escolhido.
        hook::request_brand_change();
        SHARED.enabled.store(app.settings.enabled, Ordering::SeqCst);
        hook::request_enabled(app.settings.enabled);
        app
    }

    /// Leva os valores do painel para o motor e grava em disco.
    fn publish(&self) {
        SHARED.set_dpi(
            self.settings.base_dpi.clamp(50, 32_000) as u16,
            self.settings.shooting_dpi_x.clamp(50, 32_000) as u16,
        );
        SHARED.set_fire_button(self.settings.fire_button);
        SHARED.set_brand(self.settings.brand);
        self.settings.save();
    }

    fn apply_palette(&mut self, ctx: &egui::Context) {
        let chosen = (self.settings.background, self.settings.accent);
        if self.applied_colours == Some(chosen) {
            return;
        }
        self.applied_colours = Some(chosen);

        let palette = Palette::from(&self.settings);
        let light = theme::ink_for(self.settings.background) == theme::INK_DARK;
        ctx.all_styles_mut(|style| {
            style.visuals = if light {
                egui::Visuals::light()
            } else {
                egui::Visuals::dark()
            };
            style.visuals.panel_fill = palette.background;
            style.visuals.window_fill = palette.background;
            style.visuals.extreme_bg_color = palette.surface;
            style.visuals.override_text_color = Some(palette.ink);
            style.visuals.widgets.inactive.bg_fill = palette.surface;
            style.visuals.widgets.inactive.weak_bg_fill = palette.surface;
            style.visuals.widgets.hovered.bg_fill = palette.surface_hover;
            style.visuals.widgets.hovered.weak_bg_fill = palette.surface_hover;
            style.visuals.widgets.active.bg_fill = palette.surface_hover;
            style.visuals.selection.bg_fill = palette.accent.gamma_multiply(0.35);
            style.visuals.selection.stroke.color = palette.ink;
            // `override_text_color` não alcança o texto `strong()`, que lê o traço do
            // widget ativo — sem isto o título sai branco sobre fundo claro.
            for widget in [
                &mut style.visuals.widgets.noninteractive,
                &mut style.visuals.widgets.inactive,
                &mut style.visuals.widgets.hovered,
                &mut style.visuals.widgets.active,
                &mut style.visuals.widgets.open,
            ] {
                widget.fg_stroke.color = palette.ink;
                widget.bg_stroke.color = palette.surface_hover;
            }
            style.spacing.item_spacing = egui::vec2(10.0, 10.0);
            style.spacing.interact_size.y = 28.0;
        });
    }

    fn dpi_row(ui: &mut egui::Ui, label: &str, value: &mut u32, muted: egui::Color32) -> bool {
        let mut changed = false;
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(label).color(muted));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                changed = ui
                    .add(egui::DragValue::new(value).speed(10).range(50..=32_000))
                    .changed();
            });
        });
        changed
    }
}

impl eframe::App for App {
    /// O padrão do eframe limpa a janela com um cinza fixo e ignora `panel_fill`.
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        rgb(self.settings.background).to_normalized_gamma_f32()
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let live = SHARED.enabled.load(Ordering::SeqCst);
        self.settings.enabled = live;
        ui.ctx().request_repaint_after(Duration::from_millis(200));
        self.apply_palette(ui.ctx());

        let per_axis_ok = SHARED.per_axis.load(Ordering::Relaxed);
        if !per_axis_ok {
            self.settings.split_axes = false;
        }
        let wanted = if self.settings.split_axes {
            HEIGHT_SPLIT
        } else {
            HEIGHT_JOINED
        };
        if self.requested_height != wanted {
            self.requested_height = wanted;
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(340.0, wanted)));
        }

        let palette = Palette::from(&self.settings);
        let muted = palette.muted;

        egui::Frame::NONE
            .inner_margin(egui::Margin::symmetric(20, 16))
            .show(ui, |ui| {
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new("Sensibilidade alternada")
                        .size(19.0)
                        .strong(),
                );
                ui.label(
                    egui::RichText::new("Troca a DPI do mouse enquanto o gatilho está pressionado")
                        .size(11.5)
                        .color(muted),
                );

                ui.add_space(14.0);
                let mut changed = false;

                ui.add_enabled_ui(per_axis_ok, |ui| {
                    changed |= ui
                        .checkbox(&mut self.settings.split_axes, "Separar eixos X e Y")
                        .changed();
                });
                if !per_axis_ok {
                    ui.label(
                        egui::RichText::new(
                            "seu mouse não tem DPI por eixo (feature HID++ 0x2202)",
                        )
                        .size(10.5)
                        .color(muted.gamma_multiply(0.85)),
                    );
                }

                ui.add_space(2.0);
                if self.settings.split_axes {
                    changed |=
                        Self::dpi_row(ui, "DPI base — X", &mut self.settings.base_dpi, muted);
                    changed |=
                        Self::dpi_row(ui, "DPI base — Y", &mut self.settings.base_dpi_y, muted);
                    changed |= Self::dpi_row(
                        ui,
                        "DPI atirando — X",
                        &mut self.settings.shooting_dpi_x,
                        muted,
                    );
                    changed |= Self::dpi_row(
                        ui,
                        "DPI atirando — Y",
                        &mut self.settings.shooting_dpi_y,
                        muted,
                    );
                } else {
                    changed |= Self::dpi_row(ui, "DPI base", &mut self.settings.base_dpi, muted);
                    changed |=
                        Self::dpi_row(ui, "DPI atirando", &mut self.settings.shooting_dpi_x, muted);
                    self.settings.base_dpi_y = self.settings.base_dpi;
                    self.settings.shooting_dpi_y = self.settings.shooting_dpi_x;
                }

                ui.add_space(4.0);
                let mut brand_changed = false;
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Marca do mouse").color(muted));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        egui::ComboBox::from_id_salt("brand")
                            .selected_text(self.settings.brand.label())
                            .width(110.0)
                            .show_ui(ui, |ui| {
                                for option in Brand::ALL {
                                    brand_changed |= ui
                                        .selectable_value(
                                            &mut self.settings.brand,
                                            option,
                                            option.label(),
                                        )
                                        .changed();
                                }
                            });
                    });
                });
                ui.label(
                    egui::RichText::new(self.settings.brand.method())
                        .size(10.5)
                        .color(muted.gamma_multiply(0.85)),
                );
                changed |= brand_changed;

                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Botão de tiro").color(muted));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        egui::ComboBox::from_id_salt("fire_button")
                            .selected_text(self.settings.fire_button.label())
                            .width(110.0)
                            .show_ui(ui, |ui| {
                                for option in FireButton::ALL {
                                    changed |= ui
                                        .selectable_value(
                                            &mut self.settings.fire_button,
                                            option,
                                            option.label(),
                                        )
                                        .changed();
                                }
                            });
                    });
                });

                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Cores").color(muted));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        changed |= ui
                            .color_edit_button_srgb(&mut self.settings.background)
                            .on_hover_text("Cor do fundo")
                            .changed();
                        changed |= ui
                            .color_edit_button_srgb(&mut self.settings.accent)
                            .on_hover_text("Cor dos botões e das letras")
                            .changed();
                        if ui
                            .small_button("Padrão")
                            .on_hover_text("Voltar às cores originais")
                            .clicked()
                        {
                            let fresh = Settings::default();
                            self.settings.accent = fresh.accent;
                            self.settings.background = fresh.background;
                            changed = true;
                        }
                    });
                });

                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new(format!(
                        "{} para {} ao atirar",
                        self.settings.base_dpi, self.settings.shooting_dpi_x
                    ))
                    .size(11.5)
                    .color(muted),
                );

                ui.add_space(12.0);
                let engine_ok = SHARED.engine_usable.load(Ordering::Relaxed);
                let label = if live { "DESATIVAR" } else { "ATIVAR" };
                let (fill, text) = if live {
                    (palette.surface_hover, palette.ink)
                } else {
                    (palette.accent, palette.on_accent(self.settings.accent))
                };
                let button =
                    egui::Button::new(egui::RichText::new(label).size(14.0).strong().color(text))
                        .fill(fill);

                ui.add_enabled_ui(engine_ok, |ui| {
                    if ui
                        .add_sized(egui::vec2(ui.available_width(), 40.0), button)
                        .clicked()
                    {
                        let want = !live;
                        SHARED.enabled.store(want, Ordering::SeqCst);
                        self.settings.enabled = want;
                        hook::request_enabled(want);
                        changed = true;
                    }
                });

                ui.add_space(10.0);
                let focused = SHARED.target_focused.load(Ordering::Relaxed);
                let holding = SHARED.holding_fire.load(Ordering::Relaxed);
                let (dot, status) = if !engine_ok {
                    (TROUBLE, "Sem motor de DPI para esta marca".to_string())
                } else if !live {
                    (muted, "Desligado".to_string())
                } else if !focused {
                    (WAITING, "Aguardando o BlueStacks ganhar foco".to_string())
                } else if holding {
                    (LIVE, "Aplicando agora".to_string())
                } else {
                    (LIVE, "Pronto — segure o gatilho".to_string())
                };
                ui.horizontal(|ui| {
                    let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(9.0, 9.0), egui::Sense::hover());
                    ui.painter().circle_filled(rect.center(), 4.5, dot);
                    ui.label(egui::RichText::new(status).size(11.5).color(muted));
                });

                let message = SHARED.last_message();
                if !message.is_empty() {
                    ui.label(
                        egui::RichText::new(message)
                            .size(10.5)
                            .color(muted.gamma_multiply(0.85)),
                    );
                }

                ui.add_space(2.0);
                ui.label(
                    egui::RichText::new("Ctrl + Alt + S  liga e desliga sem sair do jogo")
                        .size(11.0)
                        .color(muted.gamma_multiply(0.8)),
                );

                if changed {
                    self.publish();
                }
                if brand_changed {
                    // O motor e outro: refazer na thread que fala com o dispositivo.
                    hook::request_brand_change();
                }
            });
    }
}
