use egui::{vec2, Align2, Color32, FontId, Id, LayerId, Order, RichText, Stroke};

use crate::game::{Game, Screen};
use crate::world::BLOCK_COLOR;

const ACCENT: Color32 = Color32::from_rgb(0x34, 0x9B, 0xFF);

pub fn setup_style(ctx: &egui::Context) {
    let mut v = egui::Visuals::dark();
    v.window_rounding = 18.0.into();
    v.widgets.inactive.rounding = 12.0.into();
    v.widgets.hovered.rounding = 12.0.into();
    v.widgets.active.rounding = 12.0.into();
    v.selection.bg_fill = ACCENT;
    v.widgets.hovered.bg_stroke = Stroke::new(1.5, ACCENT);
    v.slider_trailing_fill = true;
    ctx.set_visuals(v);
    ctx.style_mut(|s| {
        s.spacing.item_spacing = vec2(10.0, 12.0);
        s.spacing.slider_width = 240.0;
        s.spacing.interact_size.y = 30.0;
    });
}

fn big_button(ui: &mut egui::Ui, text: &str, accent: bool) -> bool {
    let fill = if accent { ACCENT } else { Color32::from_rgba_unmultiplied(255, 255, 255, 16) };
    let btn = egui::Button::new(RichText::new(text).size(22.0).color(Color32::WHITE))
        .min_size(vec2(340.0, 58.0))
        .rounding(14.0)
        .fill(fill)
        .stroke(Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 40)));
    ui.add(btn).on_hover_cursor(egui::CursorIcon::PointingHand).clicked()
}

fn title(ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.label(RichText::new("OMNI").size(84.0).strong().color(Color32::WHITE));
        ui.label(RichText::new("WARS").size(84.0).strong().color(ACCENT));
    });
}

pub fn draw(ctx: &egui::Context, g: &mut Game) {
    match g.screen {
        Screen::Main => main_menu(ctx, g),
        Screen::Settings(from_pause) => {
            if from_pause {
                dim(ctx, 170);
            } else {
                side_panel(ctx);
            }
            settings(ctx, g, from_pause);
        }
        Screen::Playing => hud(ctx, g),
        Screen::Paused => {
            hud(ctx, g);
            dim(ctx, 150);
            pause_menu(ctx, g);
        }
    }
}

fn dim(ctx: &egui::Context, alpha: u8) {
    let p = ctx.layer_painter(LayerId::new(Order::Middle, Id::new("dim")));
    p.rect_filled(ctx.screen_rect(), 0.0, Color32::from_rgba_unmultiplied(4, 6, 16, alpha));
}

fn side_panel(ctx: &egui::Context) {
    let r = ctx.screen_rect();
    let p = ctx.layer_painter(LayerId::background());
    p.rect_filled(
        egui::Rect::from_min_size(r.min, vec2(600.0, r.height())),
        0.0,
        Color32::from_rgba_unmultiplied(6, 10, 24, 190),
    );
}

fn main_menu(ctx: &egui::Context, g: &mut Game) {
    side_panel(ctx);
    egui::Area::new(Id::new("main_menu"))
        .anchor(Align2::LEFT_CENTER, vec2(90.0, 0.0))
        .show(ctx, |ui| {
            title(ui);
            ui.label(
                RichText::new("Все мини-режимы. Умные боты. Одна игра.")
                    .size(18.0)
                    .color(Color32::from_gray(170)),
            );
            ui.add_space(28.0);
            if big_button(ui, "Играть", true) {
                g.start();
            }
            if big_button(ui, "Настройки", false) {
                g.screen = Screen::Settings(false);
            }
            if big_button(ui, "Выход", false) {
                g.quit = true;
            }
        });
    egui::Area::new(Id::new("version"))
        .anchor(Align2::LEFT_BOTTOM, vec2(90.0, -30.0))
        .show(ctx, |ui| {
            ui.label(
                RichText::new("OmniWars · первый набросок v0.1")
                    .size(14.0)
                    .color(Color32::from_gray(120)),
            );
        });
}

fn settings(ctx: &egui::Context, g: &mut Game, from_pause: bool) {
    egui::Area::new(Id::new("settings"))
        .anchor(
            if from_pause { Align2::CENTER_CENTER } else { Align2::LEFT_CENTER },
            if from_pause { vec2(0.0, 0.0) } else { vec2(90.0, 0.0) },
        )
        .show(ctx, |ui| {
            egui::Frame::none()
                .fill(Color32::from_rgba_unmultiplied(14, 18, 34, 240))
                .rounding(20.0)
                .stroke(Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 30)))
                .inner_margin(32.0)
                .show(ui, |ui| {
                    ui.set_width(440.0);
                    ui.label(RichText::new("Настройки").size(34.0).strong().color(Color32::WHITE));
                    ui.add_space(8.0);
                    ui.add(egui::Slider::new(&mut g.settings.fov, 30.0..=110.0).text("Поле зрения"));
                    ui.add(
                        egui::Slider::new(&mut g.settings.sens, 0.0..=200.0)
                            .suffix("%")
                            .text("Чувствительность"),
                    );
                    ui.checkbox(&mut g.settings.vsync, "Вертикальная синхронизация (в игре)");
                    ui.checkbox(&mut g.settings.fullscreen, "Полный экран");
                    ui.checkbox(&mut g.settings.show_fps, "Показывать FPS");
                    ui.add_space(10.0);
                    if big_button(ui, "Назад", true) {
                        g.screen = if from_pause { Screen::Paused } else { Screen::Main };
                    }
                });
        });
}

fn pause_menu(ctx: &egui::Context, g: &mut Game) {
    egui::Area::new(Id::new("pause"))
        .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.label(RichText::new("ПАУЗА").size(54.0).strong().color(Color32::WHITE));
                ui.add_space(14.0);
                if big_button(ui, "Продолжить", true) {
                    g.screen = Screen::Playing;
                }
                if big_button(ui, "Настройки", false) {
                    g.screen = Screen::Settings(true);
                }
                if big_button(ui, "В главное меню", false) {
                    g.screen = Screen::Main;
                }
                if big_button(ui, "Выход", false) {
                    g.quit = true;
                }
            });
        });
}

fn hud(ctx: &egui::Context, g: &Game) {
    let p = ctx.layer_painter(LayerId::new(Order::Foreground, Id::new("hud")));
    let r = ctx.screen_rect();
    let c = r.center();

    // прицел
    let col = Color32::from_rgba_unmultiplied(255, 255, 255, 220);
    let st = Stroke::new(2.0, col);
    p.line_segment([c - vec2(9.0, 0.0), c + vec2(9.0, 0.0)], st);
    p.line_segment([c - vec2(0.0, 9.0), c + vec2(0.0, 9.0)], st);

    // хотбар с единственным блоком
    let slot = egui::Rect::from_center_size(egui::pos2(c.x, r.bottom() - 54.0), vec2(64.0, 64.0));
    p.rect_filled(slot, 10.0, Color32::from_rgba_unmultiplied(8, 12, 26, 190));
    p.rect_stroke(slot, 10.0, Stroke::new(2.0, ACCENT));
    let bc = Color32::from_rgb(
        (BLOCK_COLOR[0] * 255.0) as u8,
        (BLOCK_COLOR[1] * 255.0) as u8,
        (BLOCK_COLOR[2] * 255.0) as u8,
    );
    p.rect_filled(egui::Rect::from_center_size(slot.center(), vec2(38.0, 38.0)), 6.0, bc);

    if g.settings.show_fps {
        let pos = g.player.pos;
        let text = format!(
            "FPS {:.0}\nXYZ {:.1} {:.1} {:.1}{}",
            g.fps,
            pos.x,
            pos.y,
            pos.z,
            if g.player.flying {
                "\nПолёт"
            } else if g.player.crouching {
                "\nПрисед"
            } else if g.player.sprinting {
                "\nБег"
            } else {
                ""
            }
        );
        p.text(
            egui::pos2(14.0, 12.0),
            Align2::LEFT_TOP,
            text,
            FontId::monospace(16.0),
            Color32::from_rgba_unmultiplied(255, 255, 255, 235),
        );
    }
}
