//! The panel.

use std::sync::atomic::Ordering;
use std::time::Duration;

use eframe::egui;

use crate::config::Settings;
use crate::fire_button::FireButton;
use crate::hook;
use crate::scaling::factor_from_dpi;
use crate::shared::SHARED;
use crate::theme;

/// Semantic, not decorative: green means "acting now" regardless of the chosen palette.
const LIVE: egui::Color32 = egui::Color32::from_rgb(102, 214, 142);
const WAITING: egui::Color32 = egui::Color32::from_rgb(214, 176, 102);

/// Window heights for the two shapes of the panel: joined axes shows two DPI fields,
/// split shows four.
const HEIGHT_JOINED: f32 = 462.0;
const HEIGHT_SPLIT: f32 = 538.0;

/// Colours in use this frame, all derived from the two the user picks.
struct Palette {
    background: egui::Color32,
    ink: egui::Color32,
    muted: egui::Color32,
    accent: egui::Color32,
    surface: egui::Color32,
    surface_hover: egui::Color32,
}

impl Palette {
    /// Two choices drive everything: one colour for the buttons and the lettering, one
    /// for the background. The rest are blends of those two, so the panel stays coherent
    /// whatever the user picks.
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

    /// Text printed *on top of* the filled button. It cannot be the accent — that is the
    /// fill — so it is derived from how bright the accent is.
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
    /// Last height asked of the window manager, so the request is sent on change only.
    requested_height: f32,
    /// Last palette pushed into the egui style, for the same reason.
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
        SHARED.enabled.store(app.settings.enabled, Ordering::SeqCst);
        hook::request_enabled(app.settings.enabled);
        app
    }

    /// Pushes the panel values to the hook and persists them.
    fn publish(&self) {
        let (fx, fy) = self.factors();
        SHARED.set_factors(fx, fy);
        SHARED.set_fire_button(self.settings.fire_button);
        self.settings.save();
    }

    /// With the axes joined, the Y fields do not exist conceptually: X answers for both.
    fn factors(&self) -> (f32, f32) {
        let fx = factor_from_dpi(self.settings.base_dpi, self.settings.shooting_dpi_x);
        let fy = if self.settings.split_axes {
            factor_from_dpi(self.settings.base_dpi_y, self.settings.shooting_dpi_y)
        } else {
            fx
        };
        (fx, fy)
    }

    fn apply_palette(&mut self, ctx: &egui::Context) {
        let chosen = (self.settings.background, self.settings.accent);
        if self.applied_colours == Some(chosen) {
            return;
        }
        self.applied_colours = Some(chosen);

        let palette = Palette::from(&self.settings);
        let light_background = theme::ink_for(self.settings.background) == theme::INK_DARK;
        ctx.all_styles_mut(|style| {
            // Start from the base whose shadows and strokes suit the chosen background.
            style.visuals = if light_background {
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
            // `override_text_color` does not reach `strong()` text, which reads the
            // active widget stroke — a white title over a light background otherwise.
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
                    .add(egui::DragValue::new(value).speed(10).range(1..=32_000))
                    .changed();
            });
        });
        changed
    }
}

impl eframe::App for App {
    /// eframe's default clears the window with a hardcoded near-black at alpha 180 and
    /// ignores `panel_fill` entirely — without this the chosen background never shows,
    /// and the window is faintly translucent.
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        rgb(self.settings.background).to_normalized_gamma_f32()
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // The global hotkey can flip the state from the hook thread.
        let live = SHARED.enabled.load(Ordering::SeqCst);
        self.settings.enabled = live;
        ui.ctx().request_repaint_after(Duration::from_millis(200));
        self.apply_palette(ui.ctx());

        let wanted_height = if self.settings.split_axes {
            HEIGHT_SPLIT
        } else {
            HEIGHT_JOINED
        };
        if self.requested_height != wanted_height {
            self.requested_height = wanted_height;
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
                    340.0,
                    wanted_height,
                )));
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
                    egui::RichText::new("A mira muda enquanto o gatilho está pressionado")
                        .size(11.5)
                        .color(muted),
                );

                ui.add_space(14.0);
                let mut changed = ui
                    .checkbox(&mut self.settings.split_axes, "Separar eixos X e Y")
                    .changed();

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
                    // Joined: the Y fields mirror X, so checking the box starts from parity
                    // instead of from a stale number the user never sees.
                    self.settings.base_dpi_y = self.settings.base_dpi;
                    self.settings.shooting_dpi_y = self.settings.shooting_dpi_x;
                }

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
                let (fx, fy) = self.factors();
                let factor_text = if self.settings.split_axes {
                    format!("Fator  X {fx:.2}×   Y {fy:.2}×")
                } else {
                    format!("Fator  {fx:.2}×")
                };
                ui.label(egui::RichText::new(factor_text).size(11.5).color(muted));

                ui.add_space(14.0);
                let button_label = if live { "DESATIVAR" } else { "ATIVAR" };
                let (button_fill, button_text) = if live {
                    (palette.surface_hover, palette.ink)
                } else {
                    (palette.accent, palette.on_accent(self.settings.accent))
                };
                let button = egui::Button::new(
                    egui::RichText::new(button_label)
                        .size(14.0)
                        .strong()
                        .color(button_text),
                )
                .fill(button_fill);

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

                ui.add_space(10.0);
                let focused = SHARED.target_focused.load(Ordering::Relaxed);
                let holding = SHARED.holding_fire.load(Ordering::Relaxed);
                let (dot, status) = match (live, focused, holding) {
                    (false, _, _) => (muted, "Desligado"),
                    (true, false, _) => (WAITING, "Aguardando o BlueStacks ganhar foco"),
                    (true, true, false) => (LIVE, "Pronto — segure o gatilho"),
                    (true, true, true) => (LIVE, "Aplicando agora"),
                };
                ui.horizontal(|ui| {
                    let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(9.0, 9.0), egui::Sense::hover());
                    ui.painter().circle_filled(rect.center(), 4.5, dot);
                    ui.label(egui::RichText::new(status).size(11.5).color(muted));
                });

                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new("Ctrl + Alt + S  liga e desliga sem sair do jogo")
                        .size(11.0)
                        .color(muted.gamma_multiply(0.85)),
                );

                if changed {
                    self.publish();
                }
            });
    }
}
