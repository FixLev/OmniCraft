#![cfg_attr(all(not(debug_assertions), target_os = "windows"), windows_subsystem = "windows")]

mod game;
mod player;
mod render;
mod ui;
mod world;

use std::{sync::Arc, time::Instant};

use glam::{Mat4, Vec3};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{DeviceEvent, DeviceId, ElementState, MouseButton, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::PhysicalKey,
    window::{CursorGrabMode, Fullscreen, Window, WindowId},
};

use game::{Game, Screen};
use render::Gfx;
use world::{D, W};

struct App {
    gfx: Option<Gfx>,
    game: Game,
}

fn apply_grab(window: &Window, grab: bool) {
    if grab {
        let _ = window
            .set_cursor_grab(CursorGrabMode::Locked)
            .or_else(|_| window.set_cursor_grab(CursorGrabMode::Confined));
        window.set_cursor_visible(false);
    } else {
        let _ = window.set_cursor_grab(CursorGrabMode::None);
        window.set_cursor_visible(true);
    }
}

impl App {
    fn redraw(&mut self, el: &ActiveEventLoop) {
        let Some(gfx) = self.gfx.as_mut() else { return };
        let g = &mut self.game;

        // время и FPS
        let now = Instant::now();
        let dt = (now - g.last).as_secs_f32().min(0.05);
        g.last = now;
        g.menu_t += dt;
        g.frames += 1;
        g.frame_acc += dt;
        if g.frame_acc >= 0.5 {
            g.fps = g.frames as f32 / g.frame_acc;
            g.frames = 0;
            g.frame_acc = 0.0;
        }

        if g.screen == Screen::Playing {
            g.update(dt);
        }

        let size = gfx.window.inner_size();
        if size.width == 0 || size.height == 0 {
            return;
        }
        let aspect = size.width as f32 / size.height as f32;
        let in_world = matches!(g.screen, Screen::Playing | Screen::Paused);
        let fov = if in_world {
            (g.settings.fov * g.player.fov_mult(g.alpha)).clamp(1.0, 170.0)
        } else {
            g.settings.fov
        };
        let proj = Mat4::perspective_rh(fov.to_radians(), aspect, 0.05, 400.0);
        let (view, eye) = if in_world {
            let eye = g.player.cam_pos(g.alpha);
            (Mat4::look_to_rh(eye, g.player.forward(), Vec3::Y), eye)
        } else {
            // фон меню: камера медленно облетает платформу
            let a = g.menu_t * 0.12;
            let c = Vec3::new(W as f32 / 2.0, 3.0, D as f32 / 2.0);
            let eye = c + Vec3::new(a.cos() * 55.0, 16.0, a.sin() * 55.0);
            (Mat4::look_at_rh(eye, c, Vec3::Y), eye)
        };
        let target = if g.screen == Screen::Playing {
            g.world
                .raycast(eye, g.player.forward(), 4.5)
                .map(|h| h.block)
        } else {
            None
        };

        // UI
        let raw = gfx.egui_state.take_egui_input(&gfx.window);
        let out = gfx.egui_ctx.run(raw, |ctx| ui::draw(ctx, g));
        gfx.egui_state.handle_platform_output(&gfx.window, out.platform_output);

        if g.quit {
            el.exit();
            return;
        }

        // применяем настройки / состояние окна
        gfx.set_vsync(g.settings.vsync || g.screen != Screen::Playing);
        if g.settings.fullscreen != g.is_fullscreen {
            g.is_fullscreen = g.settings.fullscreen;
            gfx.window
                .set_fullscreen(if g.is_fullscreen { Some(Fullscreen::Borderless(None)) } else { None });
        }
        let want_grab = g.screen == Screen::Playing;
        if want_grab != g.grabbed {
            g.grabbed = want_grab;
            apply_grab(&gfx.window, want_grab);
        }

        gfx.sync_world(&mut g.world);
        gfx.render(
            proj * view,
            eye,
            target,
            out.textures_delta,
            out.shapes,
            out.pixels_per_point,
        );
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.gfx.is_some() {
            return;
        }
        let window = el
            .create_window(
                Window::default_attributes()
                    .with_title("OmniWars")
                    .with_inner_size(LogicalSize::new(1280.0, 720.0)),
            )
            .expect("не удалось создать окно");
        self.gfx = Some(Gfx::new(Arc::new(window)));
        self.game.last = Instant::now();
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(gfx) = self.gfx.as_mut() else { return };

        // в меню события отдаём egui; в игре — только игре
        let consumed = if self.game.screen != Screen::Playing {
            gfx.egui_state.on_window_event(&gfx.window, &event).consumed
        } else {
            false
        };

        match event {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::Resized(s) => gfx.resize(s.width, s.height),
            WindowEvent::Focused(false) => self.game.pause_if_playing(),
            WindowEvent::KeyboardInput { event: ke, .. } => {
                if let PhysicalKey::Code(code) = ke.physical_key {
                    let pressed = ke.state == ElementState::Pressed;
                    if code == winit::keyboard::KeyCode::Escape || !consumed {
                        self.game.key(code, pressed, ke.repeat);
                    }
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if !consumed && (button == MouseButton::Left || button == MouseButton::Right) {
                    self.game
                        .mouse_button(button == MouseButton::Left, state == ElementState::Pressed);
                }
            }
            WindowEvent::RedrawRequested => self.redraw(el),
            _ => {}
        }
    }

    fn device_event(&mut self, _el: &ActiveEventLoop, _id: DeviceId, event: DeviceEvent) {
        if let DeviceEvent::MouseMotion { delta } = event {
            if self.game.screen == Screen::Playing {
                self.game.mouse_look(delta.0 as f32, delta.1 as f32);
            }
        }
    }

    fn about_to_wait(&mut self, _el: &ActiveEventLoop) {
        if let Some(gfx) = &self.gfx {
            gfx.window.request_redraw();
        }
    }
}

fn main() {
    let event_loop = EventLoop::new().expect("event loop");
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App { gfx: None, game: Game::new() };
    event_loop.run_app(&mut app).expect("run");
}
