use std::{collections::HashSet, time::Instant};

use winit::keyboard::KeyCode;

use crate::{
    player::{Input, Player, TICK},
    world::World,
};

/// Дальность взаимодействия с блоками (как в Minecraft: 4.5).
const REACH: f32 = 4.5;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Main,
    /// true — открыты из паузы
    Settings(bool),
    Playing,
    Paused,
}

pub struct Settings {
    pub fov: f32,
    /// чувствительность в процентах, как в Minecraft (по умолчанию 100% = значение 0.5)
    pub sens: f32,
    pub vsync: bool,
    pub fullscreen: bool,
    pub show_fps: bool,
}

pub struct Game {
    pub screen: Screen,
    pub settings: Settings,
    pub world: World,
    pub player: Player,
    pub keys: HashSet<KeyCode>,
    pub lmb: bool,
    pub rmb: bool,
    break_cd: i32,
    place_cd: i32,
    tick_acc: f32,
    pub alpha: f32,
    pub menu_t: f32,
    pub last: Instant,
    pub frames: u32,
    pub frame_acc: f32,
    pub fps: f32,
    pub grabbed: bool,
    pub is_fullscreen: bool,
    pub quit: bool,
}

impl Game {
    pub fn new() -> Self {
        Self {
            screen: Screen::Main,
            settings: Settings { fov: 70.0, sens: 100.0, vsync: false, fullscreen: false, show_fps: true },
            world: World::new(),
            player: Player::new(),
            keys: HashSet::new(),
            lmb: false,
            rmb: false,
            break_cd: 0,
            place_cd: 0,
            tick_acc: 0.0,
            alpha: 0.0,
            menu_t: 0.0,
            last: Instant::now(),
            frames: 0,
            frame_acc: 0.0,
            fps: 0.0,
            grabbed: false,
            is_fullscreen: false,
            quit: false,
        }
    }

    pub fn start(&mut self) {
        self.world = World::new();
        self.player = Player::new();
        self.keys.clear();
        self.lmb = false;
        self.rmb = false;
        self.tick_acc = 0.0;
        self.alpha = 0.0;
        self.screen = Screen::Playing;
    }

    pub fn pause_if_playing(&mut self) {
        if self.screen == Screen::Playing {
            self.screen = Screen::Paused;
            self.keys.clear();
            self.lmb = false;
            self.rmb = false;
        }
    }

    pub fn key(&mut self, code: KeyCode, pressed: bool, repeat: bool) {
        if !pressed {
            self.keys.remove(&code);
            return;
        }
        if repeat {
            return;
        }
        match code {
            KeyCode::Escape => match self.screen {
                Screen::Playing => self.pause_if_playing(),
                Screen::Paused => self.screen = Screen::Playing,
                Screen::Settings(from_pause) => {
                    self.screen = if from_pause { Screen::Paused } else { Screen::Main };
                }
                Screen::Main => {}
            },
            _ => {
                if self.screen == Screen::Playing {
                    self.keys.insert(code);
                }
            }
        }
    }

    /// Формула чувствительности из Minecraft: градусов на пиксель = 0.15 * 8 * (v*0.6+0.2)^3.
    pub fn mouse_look(&mut self, dx: f32, dy: f32) {
        let v = self.settings.sens / 200.0;
        let mult = 8.0 * (v * 0.6 + 0.2).powi(3);
        let k = (0.15 * mult).to_radians();
        self.player.yaw += dx * k;
        self.player.pitch = (self.player.pitch - dy * k).clamp(-1.5707, 1.5707);
    }

    pub fn mouse_button(&mut self, left: bool, pressed: bool) {
        if left {
            self.lmb = pressed;
        } else {
            self.rmb = pressed;
        }
        if pressed && self.screen == Screen::Playing {
            self.act(left);
            if left {
                self.break_cd = 5;
            } else {
                self.place_cd = 4;
            }
        }
    }

    fn act(&mut self, left: bool) {
        let eye = self.player.cam_pos(self.alpha);
        let Some(hit) = self.world.raycast(eye, self.player.forward(), REACH) else {
            return;
        };
        if left {
            self.world.set(hit.block.x, hit.block.y, hit.block.z, false);
        } else {
            let c = hit.prev;
            if c == hit.block || self.player.intersects_block(c) {
                return;
            }
            self.world.set(c.x, c.y, c.z, true);
        }
    }

    /// Вызывается каждый кадр: прокручивает фиксированные тики по 20 Гц.
    pub fn update(&mut self, dt: f32) {
        self.tick_acc += dt;
        let mut n = 0;
        while self.tick_acc >= TICK && n < 5 {
            self.tick();
            self.tick_acc -= TICK;
            n += 1;
        }
        if n == 5 {
            self.tick_acc = self.tick_acc.min(TICK);
        }
        self.alpha = (self.tick_acc / TICK).clamp(0.0, 1.0);
    }

    fn tick(&mut self) {
        let k = |c: KeyCode| self.keys.contains(&c);
        let inp = Input {
            fwd: k(KeyCode::KeyW),
            back: k(KeyCode::KeyS),
            left: k(KeyCode::KeyA),
            right: k(KeyCode::KeyD),
            jump: k(KeyCode::Space),
            sneak: k(KeyCode::ShiftLeft),
            sprint_key: k(KeyCode::ControlLeft),
        };
        self.player.tick(&self.world, &inp);

        if self.break_cd > 0 {
            self.break_cd -= 1;
        }
        if self.place_cd > 0 {
            self.place_cd -= 1;
        }
        if self.lmb && self.break_cd == 0 {
            self.act(true);
            self.break_cd = 5;
        }
        if self.rmb && self.place_cd == 0 {
            self.act(false);
            self.place_cd = 4;
        }

        if self.player.pos.y < -64.0 {
            self.player = Player::new();
        }
    }
}
