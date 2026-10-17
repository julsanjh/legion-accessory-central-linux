use crate::hid::{detect_connected_devices, unlock_with_pkexec, HidConnection};
use crate::models::{
    DeviceKind, LegionModel, BUTTON_ACTIONS, DEFAULT_PROFILE_RAW, SUPPORTED_MODELS,
    UI_BTN_TO_HW_OFFSET,
};
use crate::protocol::{
    build_chunked_packets, build_dpi_active_packet, build_dpi_max_level_packet,
    build_dpi_value_packet, build_key_action_packet, build_lighting_packet,
    build_report_rate_packet,
};
use eframe::egui::{
    self, Color32, FontId, Margin, Pos2, Rect, RichText, Rounding, Sense, Stroke, TextureHandle,
    TextureOptions, Vec2,
};

fn load_png_texture(ctx: &egui::Context, name: &str, bytes: &[u8]) -> TextureHandle {
    let img = image::load_from_memory(bytes)
        .expect("valid embedded PNG")
        .to_rgba8();
    let size = [img.width() as usize, img.height() as usize];
    let pixels = img.into_raw();
    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, &pixels);
    ctx.load_texture(name, color_image, TextureOptions::LINEAR)
}

pub struct LegionApp {
    tex_logo: Option<TextureHandle>,
    tex_mouse_icon: Option<TextureHandle>,
    tex_kb_icon: Option<TextureHandle>,
    tex_m300_lighting: Option<TextureHandle>,
    tex_m300_action: Option<TextureHandle>,

    connected: Vec<(usize, String)>,
    selected_model_idx: usize,
    override_device_path: Option<String>,
    selected_profile_idx: usize,
    profile_names: [String; 3],

    // Mouse state
    lighting_effect: String,
    color1: [u8; 3],
    color2: [u8; 3],
    dual_breathing: bool,
    dpi_levels: usize,
    active_dpi_idx: usize,
    dpi_values: [u32; 5],
    report_rate_hz: u32,
    ui_button_codes: [u8; 8],

    // Keyboard state
    kb_effect: String,
    kb_speed: u8,
    kb_disable_win: bool,
    kb_disable_shift_tab: bool,
    kb_disable_alt_tab: bool,
    kb_disable_alt_f4: bool,

    status_msg: String,
}

impl LegionApp {
    pub fn new(override_device_path: Option<String>) -> Self {
        let connected = detect_connected_devices();
        let selected_model_idx = connected.first().map(|(i, _)| *i).unwrap_or(0);
        let status_msg = if let Some((idx, path)) = connected.first() {
            format!("Connected: {} at {}", SUPPORTED_MODELS[*idx].name, path)
        } else {
            "No device detected on USB — Preview / All-Models Mode".to_string()
        };

        Self {
            tex_logo: None,
            tex_mouse_icon: None,
            tex_kb_icon: None,
            tex_m300_lighting: None,
            tex_m300_action: None,
            connected,
            selected_model_idx,
            override_device_path,
            selected_profile_idx: 0,
            profile_names: [
                "Profile 1".to_string(),
                "Profile 2".to_string(),
                "Profile 3".to_string(),
            ],
            lighting_effect: "rainbow".to_string(),
            color1: [0x25, 0x7E, 0xFF],
            color2: [0xFF, 0x00, 0x55],
            dual_breathing: true,
            dpi_levels: 3,
            active_dpi_idx: 1,
            dpi_values: [800, 1600, 3200, 4800, 6400],
            report_rate_hz: 1000,
            ui_button_codes: [0xB0, 0xB1, 0xB2, 0xC1, 0xB3, 0xB4, 0x01, 0x00],
            kb_effect: "surfing".to_string(),
            kb_speed: 2,
            kb_disable_win: true,
            kb_disable_shift_tab: false,
            kb_disable_alt_tab: false,
            kb_disable_alt_f4: false,
            status_msg,
        }
    }

    fn active_hidraw_path(&self) -> Option<String> {
        if let Some(p) = &self.override_device_path {
            return Some(p.clone());
        }
        self.connected
            .iter()
            .find(|(i, _)| *i == self.selected_model_idx)
            .or_else(|| self.connected.first())
            .map(|(_, p)| p.clone())
    }

    fn with_device<F>(&mut self, op: F)
    where
        F: FnOnce(&mut HidConnection) -> Result<String, String>,
    {
        let Some(path) = self.active_hidraw_path() else {
            self.status_msg =
                "Tidak ada perangkat Lenovo Legion yang terhubung di bus USB.".to_string();
            return;
        };
        match HidConnection::open(&path) {
            Ok(mut conn) => match op(&mut conn) {
                Ok(msg) => self.status_msg = msg,
                Err(err) => self.status_msg = format!("Error: {err}"),
            },
            Err(err) => self.status_msg = err,
        }
    }

    fn sync_from_device(&mut self) {
        let Some(path) = self.active_hidraw_path() else {
            self.status_msg = "Perangkat tidak ditemukan.".to_string();
            return;
        };
        match HidConnection::open(&path).and_then(|mut c| c.sync()) {
            Ok(prof) => {
                self.lighting_effect = prof.lighting_effect;
                self.color1 = prof.color1;
                self.color2 = prof.color2;
                self.dpi_levels = prof.dpi_levels;
                self.active_dpi_idx = prof.active_dpi_index;
                self.dpi_values = prof.dpi_stages;
                self.report_rate_hz = prof.report_rate_hz;
                for (ui_i, &(hw_off, _)) in UI_BTN_TO_HW_OFFSET.iter().enumerate() {
                    self.ui_button_codes[ui_i] = prof.button_codes[hw_off];
                }
                self.status_msg = format!(
                    "Synced from {}: {} | {} Hz | DPI Stage {}",
                    path,
                    prof.name,
                    prof.report_rate_hz,
                    prof.active_dpi_index + 1
                );
            }
            Err(e) => self.status_msg = e,
        }
    }

    fn render_mouse_cards(
        &mut self,
        ui: &mut egui::Ui,
        model: &LegionModel,
        card_bg: Color32,
        accent_blue: Color32,
        muted_gray: Color32,
    ) {
        let card_frame = egui::Frame::none()
            .fill(card_bg)
            .rounding(Rounding::same(8.0))
            .inner_margin(Margin::symmetric(36.0, 20.0));

        // CARD 1: MOUSE LIGHTING
        card_frame.show(ui, |ui| {
            ui.set_min_width(820.0);
            ui.label(
                RichText::new("Mouse Lighting")
                    .font(FontId::proportional(20.0))
                    .strong()
                    .color(Color32::WHITE),
            );
            ui.label(
                RichText::new("Set lighting effects for lighting zones.")
                    .size(13.0)
                    .color(muted_gray),
            );
            ui.add_space(14.0);

            ui.columns(2, |cols| {
                cols[0].vertical_centered(|ui| {
                    if let Some(tex) = &self.tex_m300_lighting {
                        let img_size = Vec2::new(320.0, 240.0);
                        let (rect, _) = ui.allocate_exact_size(img_size, Sense::hover());
                        ui.painter().image(
                            tex.id(),
                            rect,
                            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                            Color32::WHITE,
                        );
                        let badge_center = rect.min + Vec2::new(72.0, 145.0);
                        let glow_color = match self.lighting_effect.as_str() {
                            "off" => Color32::from_rgb(0x40, 0x40, 0x40),
                            "static" | "breathing" => {
                                Color32::from_rgb(self.color1[0], self.color1[1], self.color1[2])
                            }
                            _ => accent_blue,
                        };
                        ui.painter().circle_filled(badge_center, 14.0, glow_color);
                        ui.painter().circle_stroke(
                            badge_center,
                            14.0,
                            Stroke::new(2.0, Color32::WHITE),
                        );
                        ui.painter().text(
                            badge_center,
                            egui::Align2::CENTER_CENTER,
                            "1",
                            FontId::proportional(13.0),
                            Color32::WHITE,
                        );
                    }
                });

                cols[1].vertical(|ui| {
                    ui.add_space(36.0);
                    ui.horizontal(|ui| {
                        let (b_rect, _) =
                            ui.allocate_exact_size(Vec2::new(28.0, 28.0), Sense::hover());
                        ui.painter()
                            .circle_filled(b_rect.center(), 13.0, accent_blue);
                        ui.painter().text(
                            b_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "1",
                            FontId::proportional(13.0),
                            Color32::WHITE,
                        );

                        ui.add_space(8.0);
                        egui::ComboBox::from_id_source("mouse_lighting_effect")
                            .width(190.0)
                            .selected_text(match self.lighting_effect.as_str() {
                                "rainbow" => "Rainbow",
                                "static" => "Static",
                                "breathing" => "Breathing",
                                "random" => "Static (random color)",
                                "off" => "Off",
                                _ => "Rainbow",
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut self.lighting_effect,
                                    "rainbow".to_string(),
                                    "Rainbow",
                                );
                                ui.selectable_value(
                                    &mut self.lighting_effect,
                                    "static".to_string(),
                                    "Static",
                                );
                                ui.selectable_value(
                                    &mut self.lighting_effect,
                                    "breathing".to_string(),
                                    "Breathing",
                                );
                                ui.selectable_value(
                                    &mut self.lighting_effect,
                                    "random".to_string(),
                                    "Static (random color)",
                                );
                                ui.selectable_value(
                                    &mut self.lighting_effect,
                                    "off".to_string(),
                                    "Off",
                                );
                            });
                    });

                    ui.add_space(16.0);
                    if self.lighting_effect == "static" || self.lighting_effect == "breathing" {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Primary Color:").color(Color32::WHITE));
                            ui.color_edit_button_srgb(&mut self.color1);
                            if self.lighting_effect == "breathing" {
                                ui.add_space(12.0);
                                ui.checkbox(&mut self.dual_breathing, "Dual Color");
                                if self.dual_breathing {
                                    ui.color_edit_button_srgb(&mut self.color2);
                                }
                            }
                        });
                        ui.add_space(12.0);
                    }

                    if ui
                        .add_sized(
                            [140.0, 34.0],
                            egui::Button::new(
                                RichText::new("APPLY LIGHTING").strong().color(Color32::WHITE),
                            )
                            .fill(accent_blue)
                            .rounding(Rounding::same(5.0)),
                        )
                        .clicked()
                    {
                        let eff = self.lighting_effect.clone();
                        let c1 = self.color1;
                        let c2 = if eff == "breathing" && self.dual_breathing {
                            Some(self.color2)
                        } else {
                            None
                        };
                        let pkt = build_lighting_packet(&eff, c1, c2);
                        self.with_device(move |conn| {
                            conn.write_raw(&pkt, 50)?;
                            Ok(format!("Lighting updated: {eff}"))
                        });
                    }
                });
            });
        });

        ui.add_space(12.0);

        // CARD 2: MOUSE ACTION
        card_frame.show(ui, |ui| {
            ui.set_min_width(820.0);
            ui.label(
                RichText::new("Mouse Action")
                    .font(FontId::proportional(20.0))
                    .strong()
                    .color(Color32::WHITE),
            );
            ui.label(
                RichText::new("Set special actions for mouse buttons.")
                    .size(13.0)
                    .color(muted_gray),
            );
            ui.add_space(14.0);

            ui.columns(2, |cols| {
                cols[0].vertical(|ui| {
                    let mut changed_slot: Option<(u8, u8, &'static str)> = None;
                    for ui_idx in 0..model.buttons.min(8) {
                        let (hw_offset, slot_title) = UI_BTN_TO_HW_OFFSET[ui_idx];
                        let current_code = self.ui_button_codes[ui_idx];
                        let current_label = BUTTON_ACTIONS
                            .iter()
                            .find(|a| a.code == current_code)
                            .map(|a| a.label)
                            .unwrap_or("Custom");

                        ui.horizontal(|ui| {
                            let (b_rect, _) =
                                ui.allocate_exact_size(Vec2::new(26.0, 26.0), Sense::hover());
                            ui.painter().circle_filled(
                                b_rect.center(),
                                12.0,
                                Color32::from_rgb(0x29, 0x37, 0x57),
                            );
                            ui.painter().circle_stroke(
                                b_rect.center(),
                                12.0,
                                Stroke::new(1.5, accent_blue),
                            );
                            ui.painter().text(
                                b_rect.center(),
                                egui::Align2::CENTER_CENTER,
                                format!("{}", ui_idx + 1),
                                FontId::proportional(12.0),
                                Color32::WHITE,
                            );

                            ui.add_sized(
                                [135.0, 24.0],
                                egui::Label::new(
                                    RichText::new(&slot_title[3..])
                                        .size(13.0)
                                        .color(Color32::WHITE),
                                ),
                            );

                            egui::ComboBox::from_id_source(format!("btn_action_{ui_idx}"))
                                .width(155.0)
                                .selected_text(current_label)
                                .show_ui(ui, |ui| {
                                    for act in BUTTON_ACTIONS {
                                        if ui
                                            .selectable_value(
                                                &mut self.ui_button_codes[ui_idx],
                                                act.code,
                                                act.label,
                                            )
                                            .changed()
                                        {
                                            changed_slot =
                                                Some((hw_offset as u8, act.code, act.label));
                                        }
                                    }
                                });
                        });
                        ui.add_space(6.0);
                    }

                    if let Some((hw_off, code, lbl)) = changed_slot {
                        if !self.ui_button_codes.contains(&0xB0) {
                            self.ui_button_codes[0] = 0xB0;
                            self.status_msg =
                                "Cannot apply configuration: Please ensure Left button function exists."
                                    .to_string();
                        } else {
                            let pkt = build_key_action_packet(hw_off, code);
                            self.with_device(move |conn| {
                                conn.write_raw(&pkt, 60)?;
                                let _ = conn.read_packet(250);
                                Ok(format!("Mapped slot {hw_off} -> {lbl}"))
                            });
                        }
                    }
                });

                cols[1].vertical_centered(|ui| {
                    if let Some(tex) = &self.tex_m300_action {
                        let img_size = Vec2::new(350.0, 262.0);
                        let (rect, _) = ui.allocate_exact_size(img_size, Sense::hover());
                        ui.painter().image(
                            tex.id(),
                            rect,
                            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                            Color32::WHITE,
                        );
                    }
                });
            });
        });

        ui.add_space(12.0);

        // CARD 3: MOUSE DPI
        card_frame.show(ui, |ui| {
            ui.set_min_width(820.0);
            ui.label(
                RichText::new("Mouse DPI")
                    .font(FontId::proportional(20.0))
                    .strong()
                    .color(Color32::WHITE),
            );
            ui.label(
                RichText::new("Set DPI stages and DPI values for your mouse.")
                    .size(13.0)
                    .color(muted_gray),
            );
            ui.add_space(14.0);

            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Number of DPI stages")
                        .size(14.0)
                        .color(Color32::WHITE),
                );
                ui.add_space(12.0);
                if ui
                    .add_sized(
                        [28.0, 28.0],
                        egui::Button::new(RichText::new("−").size(16.0).strong())
                            .rounding(Rounding::same(14.0)),
                    )
                    .clicked()
                    && self.dpi_levels > 1
                {
                    self.dpi_levels -= 1;
                    if self.active_dpi_idx >= self.dpi_levels {
                        self.active_dpi_idx = self.dpi_levels - 1;
                    }
                    let lv = self.dpi_levels as u8;
                    let pkt = build_dpi_max_level_packet(lv);
                    self.with_device(move |conn| {
                        conn.write_raw(&pkt, 60)?;
                        let _ = conn.read_packet(200);
                        Ok(format!("DPI stages set to {lv}"))
                    });
                }

                ui.add_space(6.0);
                ui.label(
                    RichText::new(format!("{}", self.dpi_levels))
                        .size(16.0)
                        .strong()
                        .color(accent_blue),
                );
                ui.add_space(6.0);

                if ui
                    .add_sized(
                        [28.0, 28.0],
                        egui::Button::new(RichText::new("+").size(16.0).strong())
                            .rounding(Rounding::same(14.0)),
                    )
                    .clicked()
                    && self.dpi_levels < model.max_stages
                {
                    self.dpi_levels += 1;
                    let lv = self.dpi_levels as u8;
                    let pkt = build_dpi_max_level_packet(lv);
                    self.with_device(move |conn| {
                        conn.write_raw(&pkt, 60)?;
                        let _ = conn.read_packet(200);
                        Ok(format!("DPI stages set to {lv}"))
                    });
                }
            });

            ui.add_space(12.0);

            for stage_idx in 0..self.dpi_levels {
                ui.horizontal(|ui| {
                    let is_active = self.active_dpi_idx == stage_idx;
                    if ui
                        .radio(
                            is_active,
                            RichText::new(format!("DPI {}", stage_idx + 1)).color(Color32::WHITE),
                        )
                        .clicked()
                    {
                        self.active_dpi_idx = stage_idx;
                        let pkt = build_dpi_active_packet(stage_idx as u8);
                        self.with_device(move |conn| {
                            conn.write_raw(&pkt, 60)?;
                            let _ = conn.read_packet(200);
                            Ok(format!("Active DPI stage -> DPI {}", stage_idx + 1))
                        });
                    }

                    ui.add_space(16.0);
                    if ui
                        .add_sized(
                            [26.0, 26.0],
                            egui::Button::new("−").rounding(Rounding::same(13.0)),
                        )
                        .clicked()
                    {
                        self.dpi_values[stage_idx] =
                            (self.dpi_values[stage_idx].saturating_sub(100)).max(200);
                    }

                    let slider = egui::Slider::new(
                        &mut self.dpi_values[stage_idx],
                        200..=model.max_dpi,
                    )
                    .step_by(100.0)
                    .suffix(" DPI");
                    ui.spacing_mut().slider_width = 340.0;
                    ui.add(slider);

                    if ui
                        .add_sized(
                            [26.0, 26.0],
                            egui::Button::new("+").rounding(Rounding::same(13.0)),
                        )
                        .clicked()
                    {
                        self.dpi_values[stage_idx] =
                            (self.dpi_values[stage_idx] + 100).min(model.max_dpi);
                    }

                    ui.add_space(8.0);
                    if ui
                        .add(
                            egui::Button::new(RichText::new("APPLY").color(Color32::WHITE))
                                .fill(accent_blue)
                                .rounding(Rounding::same(4.0)),
                        )
                        .clicked()
                    {
                        let dpi_val = (self.dpi_values[stage_idx] / 100) * 100;
                        self.dpi_values[stage_idx] = dpi_val;
                        let pkt = build_dpi_value_packet(stage_idx as u8, dpi_val);
                        self.with_device(move |conn| {
                            conn.write_raw(&pkt, 60)?;
                            let _ = conn.read_packet(200);
                            Ok(format!("DPI {} set to {} DPI", stage_idx + 1, dpi_val))
                        });
                    }
                });
                ui.add_space(6.0);
            }
        });

        ui.add_space(12.0);

        // CARD 4: POLLING RATE
        card_frame.show(ui, |ui| {
            ui.set_min_width(820.0);
            ui.label(
                RichText::new("Polling Rate")
                    .font(FontId::proportional(20.0))
                    .strong()
                    .color(Color32::WHITE),
            );
            ui.label(
                RichText::new(
                    "Select a polling rate for your mouse. The polling rate determines how frequently the mouse reports its position to the computer.",
                )
                .size(13.0)
                .color(muted_gray),
            );
            ui.add_space(14.0);

            ui.horizontal(|ui| {
                for hz in [125u32, 250, 500, 1000] {
                    if ui
                        .radio_value(
                            &mut self.report_rate_hz,
                            hz,
                            RichText::new(format!("{hz} Hz")).size(14.0).color(Color32::WHITE),
                        )
                        .clicked()
                    {
                        let pkt = build_report_rate_packet(hz);
                        self.with_device(move |conn| {
                            conn.write_raw(&pkt, 60)?;
                            Ok(format!("Polling rate set to {hz} Hz"))
                        });
                    }
                    ui.add_space(65.0);
                }
            });
        });
    }

    fn render_keyboard_cards(
        &mut self,
        ui: &mut egui::Ui,
        model: &LegionModel,
        card_bg: Color32,
        accent_blue: Color32,
        muted_gray: Color32,
    ) {
        let card_frame = egui::Frame::none()
            .fill(card_bg)
            .rounding(Rounding::same(8.0))
            .inner_margin(Margin::symmetric(36.0, 20.0));

        card_frame.show(ui, |ui| {
            ui.set_min_width(820.0);
            ui.label(
                RichText::new(format!("{} — Keyboard Lighting", model.name))
                    .font(FontId::proportional(20.0))
                    .strong()
                    .color(Color32::WHITE),
            );
            ui.label(
                RichText::new(
                    "Set a lighting effect for your keyboard. Press Fn+F4 / Lighting key on your keyboard to adjust brightness.",
                )
                .size(13.0)
                .color(muted_gray),
            );
            ui.add_space(14.0);

            ui.horizontal(|ui| {
                ui.label(RichText::new("Effect:").color(Color32::WHITE));
                egui::ComboBox::from_id_source("kb_effect_combo")
                    .width(220.0)
                    .selected_text(&self.kb_effect)
                    .show_ui(ui, |ui| {
                        for eff in [
                            "surfing",
                            "breathing_rainbow",
                            "drifting",
                            "static",
                            "static_area",
                            "breathing_area",
                            "spiral",
                            "laser",
                            "fading_trace",
                            "starlight",
                        ] {
                            ui.selectable_value(&mut self.kb_effect, eff.to_string(), eff);
                        }
                    });
                ui.add_space(20.0);
                ui.label(RichText::new("Frequency:").color(Color32::WHITE));
                ui.add(egui::Slider::new(&mut self.kb_speed, 1..=4).text("Speed"));
                ui.add_space(16.0);
                ui.color_edit_button_srgb(&mut self.color1);
            });
        });

        ui.add_space(12.0);

        card_frame.show(ui, |ui| {
            ui.set_min_width(820.0);
            ui.label(
                RichText::new("Game Mode")
                    .font(FontId::proportional(20.0))
                    .strong()
                    .color(Color32::WHITE),
            );
            ui.label(
                RichText::new(
                    "Select shortcut keys to disable when game mode is on. Press Fn+F12 / Legion key to toggle game mode.",
                )
                .size(13.0)
                .color(muted_gray),
            );
            ui.add_space(12.0);
            ui.checkbox(&mut self.kb_disable_win, "Disable Windows logo key");
            ui.checkbox(&mut self.kb_disable_shift_tab, "Disable Shift+Tab");
            ui.checkbox(&mut self.kb_disable_alt_tab, "Disable Alt+Tab");
            ui.checkbox(&mut self.kb_disable_alt_f4, "Disable Alt+F4");
            let _ = accent_blue;
        });
    }
}

impl eframe::App for LegionApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.tex_logo.is_none() {
            self.tex_logo = Some(load_png_texture(
                ctx,
                "legion_logo",
                include_bytes!("../assets/legion_logo.png"),
            ));
            self.tex_mouse_icon = Some(load_png_texture(
                ctx,
                "mouse_icon",
                include_bytes!("../assets/mouse_icon.png"),
            ));
            self.tex_kb_icon = Some(load_png_texture(
                ctx,
                "kb_icon",
                include_bytes!("../assets/kb_icon.png"),
            ));
            self.tex_m300_lighting = Some(load_png_texture(
                ctx,
                "m300_lighting",
                include_bytes!("../assets/m300_lighting.png"),
            ));
            self.tex_m300_action = Some(load_png_texture(
                ctx,
                "m300_action",
                include_bytes!("../assets/m300_action.png"),
            ));
        }

        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = Color32::from_rgb(0x11, 0x19, 0x2C);
        visuals.window_fill = Color32::from_rgb(0x11, 0x19, 0x2C);
        visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(0x1F, 0x2A, 0x42);
        visuals.widgets.inactive.bg_fill = Color32::from_rgb(0x29, 0x37, 0x57);
        visuals.widgets.hovered.bg_fill = Color32::from_rgb(0x01, 0x60, 0xD6);
        visuals.widgets.active.bg_fill = Color32::from_rgb(0x25, 0x7E, 0xFF);
        visuals.selection.bg_fill = Color32::from_rgb(0x25, 0x7E, 0xFF);
        ctx.set_visuals(visuals);

        let bg_dark = Color32::from_rgb(0x11, 0x19, 0x2C);
        let card_bg = Color32::from_rgb(0x1F, 0x2A, 0x42);
        let accent_blue = Color32::from_rgb(0x25, 0x7E, 0xFF);
        let muted_gray = Color32::from_rgb(0x9D, 0xA2, 0xAA);

        egui::TopBottomPanel::top("header_bar")
            .frame(egui::Frame::none().fill(bg_dark).inner_margin(Margin::symmetric(28.0, 12.0)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if let Some(logo) = &self.tex_logo {
                        ui.image((logo.id(), Vec2::new(190.0, 38.0)));
                    }
                    ui.add_space(16.0);
                    ui.label(
                        RichText::new("ACCESSORY CENTRAL")
                            .font(FontId::proportional(15.0))
                            .color(muted_gray),
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new("Restore to Factory").color(Color32::WHITE),
                                )
                                .fill(Color32::from_rgb(0x29, 0x37, 0x57))
                                .rounding(Rounding::same(4.0)),
                            )
                            .clicked()
                        {
                            self.with_device(|conn| {
                                conn.write_raw(&[0x07, 0x05, 0x00], 80)?;
                                conn.write_raw(&[0x07, 0x05, 0x01], 80)?;
                                for pkt in build_chunked_packets(0x01, 0x00, &DEFAULT_PROFILE_RAW) {
                                    conn.write_raw(&pkt, 50)?;
                                }
                                conn.write_raw(&[0x05, 0x00, 0x00], 60)?;
                                Ok("Restored device to factory defaults.".to_string())
                            });
                        }

                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new("Unlock USB (Polkit)").color(Color32::WHITE),
                                )
                                .fill(accent_blue)
                                .rounding(Rounding::same(4.0)),
                            )
                            .clicked()
                        {
                            if let Some(path) = self.active_hidraw_path() {
                                if unlock_with_pkexec(&path) {
                                    self.sync_from_device();
                                } else {
                                    self.status_msg = "Polkit unlock dibatalkan.".to_string();
                                }
                            }
                        }

                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new("Sync from Device").color(Color32::WHITE),
                                )
                                .fill(Color32::from_rgb(0x0F, 0xA0, 0xB5))
                                .rounding(Rounding::same(4.0)),
                            )
                            .clicked()
                        {
                            self.sync_from_device();
                        }
                    });
                });

                ui.add_space(10.0);
                egui::ScrollArea::horizontal()
                    .id_source("device_nav_scroll")
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            for (idx, model) in SUPPORTED_MODELS.iter().enumerate() {
                                let is_conn = self.connected.iter().any(|(i, _)| *i == idx);
                                let is_sel = self.selected_model_idx == idx;
                                let fill = if is_sel {
                                    accent_blue
                                } else if is_conn {
                                    Color32::from_rgb(0x1E, 0x3A, 0x5F)
                                } else {
                                    card_bg
                                };
                                let label_txt = if is_conn {
                                    format!("● {}", model.id.to_uppercase())
                                } else {
                                    model.id.to_uppercase()
                                };
                                let btn = egui::Button::new(
                                    RichText::new(label_txt)
                                        .size(13.0)
                                        .strong()
                                        .color(Color32::WHITE),
                                )
                                .fill(fill)
                                .rounding(Rounding::same(6.0))
                                .min_size(Vec2::new(82.0, 32.0));
                                if ui.add(btn).on_hover_text(model.name).clicked() {
                                    self.selected_model_idx = idx;
                                }
                            }
                        });
                    });
            });

        egui::TopBottomPanel::bottom("status_bar")
            .frame(egui::Frame::none().fill(bg_dark).inner_margin(Margin::symmetric(28.0, 8.0)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let model = &SUPPORTED_MODELS[self.selected_model_idx];
                    ui.label(
                        RichText::new(format!("Model: {}  |  {}", model.name, self.status_msg))
                            .size(12.5)
                            .color(muted_gray),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new("Package v2.0.9.10231 (Native Rust Linux Port)")
                                .size(12.0)
                                .color(muted_gray),
                        );
                    });
                });
            });

        egui::SidePanel::left("profile_sidebar")
            .resizable(false)
            .exact_width(210.0)
            .frame(egui::Frame::none().fill(bg_dark).inner_margin(Margin::symmetric(20.0, 12.0)))
            .show(ctx, |ui| {
                let model = &SUPPORTED_MODELS[self.selected_model_idx];
                egui::Frame::none()
                    .fill(card_bg)
                    .rounding(Rounding::same(8.0))
                    .inner_margin(Margin::same(14.0))
                    .show(ui, |ui| {
                        ui.vertical_centered(|ui| {
                            let icon = match model.kind {
                                DeviceKind::Mouse => &self.tex_mouse_icon,
                                DeviceKind::Keyboard => &self.tex_kb_icon,
                            };
                            if let Some(tex) = icon {
                                ui.image((tex.id(), Vec2::new(56.0, 56.0)));
                            }
                            ui.add_space(6.0);
                            ui.label(
                                RichText::new(model.name)
                                    .size(13.0)
                                    .strong()
                                    .color(Color32::WHITE),
                            );
                            let conn_badge = if self
                                .connected
                                .iter()
                                .any(|(i, _)| *i == self.selected_model_idx)
                            {
                                ("● USB Connected", Color32::from_rgb(0x2E, 0xCC, 0x71))
                            } else {
                                ("○ Offline / Preview", muted_gray)
                            };
                            ui.label(RichText::new(conn_badge.0).size(11.5).color(conn_badge.1));
                        });

                        ui.add_space(16.0);
                        ui.label(
                            RichText::new("Onboard profile")
                                .size(12.5)
                                .color(muted_gray),
                        );
                        ui.add_space(6.0);
                        for p_idx in 0..3 {
                            let is_sel = self.selected_profile_idx == p_idx;
                            let fill = if is_sel {
                                accent_blue
                            } else {
                                Color32::from_rgb(0x29, 0x37, 0x57)
                            };
                            if ui
                                .add_sized(
                                    [160.0, 32.0],
                                    egui::Button::new(
                                        RichText::new(&self.profile_names[p_idx])
                                            .color(Color32::WHITE),
                                    )
                                    .fill(fill)
                                    .rounding(Rounding::same(5.0)),
                                )
                                .clicked()
                            {
                                self.selected_profile_idx = p_idx;
                            }
                            ui.add_space(4.0);
                        }
                    });
            });

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(bg_dark).inner_margin(Margin::symmetric(16.0, 10.0)))
            .show(ctx, |ui| {
                let model = SUPPORTED_MODELS[self.selected_model_idx].clone();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    if model.kind == DeviceKind::Mouse {
                        self.render_mouse_cards(ui, &model, card_bg, accent_blue, muted_gray);
                    } else {
                        self.render_keyboard_cards(ui, &model, card_bg, accent_blue, muted_gray);
                    }
                });
            });
    }
}
