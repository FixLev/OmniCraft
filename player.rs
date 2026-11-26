//! Физика игрока — перенос движения Minecraft Java Edition 1:1.
//! Всё считается фиксированными тиками по 20 в секунду (TICK = 0.05 с),
//! между тиками позиция/FOV/высота глаз интерполируются.
//!
//! Значения (как в ваниле):
//!   скорость ходьбы 0.1, бег ×1.3 (0.13), присед ×0.3 на ввод
//!   ввод ×0.98, трение земли 0.6×0.91, воздуха 0.91
//!   ускорение в воздухе 0.02 (бег 0.026), гравитация 0.08, сопротивление по Y 0.98
//!   прыжок 0.42 (+0.2 вперёд при беге), задержка прыжка 10 тиков
//!   хитбокс 0.6×1.8 (присед 1.5), глаза 1.62 (присед 1.27)
//!   защита от падения с края при приседе: шаг 0.05, глубина 0.6

use glam::{DVec3, IVec3, Vec3};

use crate::world::{World, D, PLATFORM_TOP, W};

pub const TICK: f32 = 0.05;
const HW: f64 = 0.3;
const STAND_H: f64 = 1.8;
const CROUCH_H: f64 = 1.5;
const EPS: f64 = 1e-7;

#[derive(Default, Clone, Copy)]
pub struct Input {
    pub fwd: bool,
    pub back: bool,
    pub left: bool,
    pub right: bool,
    pub jump: bool,
    pub sneak: bool,
    pub sprint_key: bool,
}

pub struct Player {
    pub pos: DVec3, // позиция ног
    pub prev_pos: DVec3,
    pub vel: DVec3, // blocks / tick
    pub yaw: f32,   // радианы; 0 = смотрим в -Z
    pub pitch: f32, // радианы; + вверх
    pub on_ground: bool,
    pub flying: bool,
    pub sprinting: bool,
    pub crouching: bool,
    horizontal_collision: bool,
    no_jump_delay: i32,
    sprint_trigger: i32,
    jump_trigger: i32,
    last_jump: bool,
    last_sneak: bool,
    last_fwd_enough: bool,
    eye: f32,
    eye_old: f32,
    fov_mod: f32,
    fov_old: f32,
}

/// Строгое пересечение бокса с твёрдыми блоками.
fn overlaps(world: &World, mn: DVec3, mx: DVec3) -> bool {
    let (x0, x1) = (mn.x.floor() as i32, mx.x.ceil() as i32 - 1);
    let (y0, y1) = (mn.y.floor() as i32, mx.y.ceil() as i32 - 1);
    let (z0, z1) = (mn.z.floor() as i32, mx.z.ceil() as i32 - 1);
    for y in y0..=y1 {
        for z in z0..=z1 {
            for x in x0..=x1 {
                if world.get(x, y, z) {
                    return true;
                }
            }
        }
    }
    false
}

/// Сдвиг бокса вдоль одной оси до первого препятствия (как Shapes.collide).
fn collide_axis(world: &World, mn: DVec3, mx: DVec3, axis: usize, d: f64) -> f64 {
    if d.abs() < EPS {
        return 0.0;
    }
    let a = (axis + 1) % 3;
    let b = (axis + 2) % 3;
    let (la, ha) = ((mn[a] + EPS).floor() as i32, (mx[a] - EPS).floor() as i32);
    let (lb, hb) = ((mn[b] + EPS).floor() as i32, (mx[b] - EPS).floor() as i32);
    let solid_slice = |c: i32| -> bool {
        for ia in la..=ha {
            for ib in lb..=hb {
                let mut p = [0i32; 3];
                p[axis] = c;
                p[a] = ia;
                p[b] = ib;
                if world.get(p[0], p[1], p[2]) {
                    return true;
                }
            }
        }
        false
    };
    if d > 0.0 {
        let c0 = (mx[axis] - EPS).ceil() as i32;
        let c1 = (mx[axis] + d).floor() as i32;
        for c in c0..=c1 {
            if solid_slice(c) {
                return (c as f64 - mx[axis]).max(0.0).min(d);
            }
        }
    } else {
        let c0 = (mn[axis] + EPS).floor() as i32 - 1;
        let c1 = (mn[axis] + d).floor() as i32;
        let mut c = c0;
        while c >= c1 {
            if solid_slice(c) {
                return ((c + 1) as f64 - mn[axis]).min(0.0).max(d);
            }
            c -= 1;
        }
    }
    d
}

/// Порядок как в ваниле: сначала Y, затем X/Z — та ось, где движение больше.
fn collide(world: &World, pos: DVec3, h: f64, mv: DVec3) -> DVec3 {
    let mut mn = DVec3::new(pos.x - HW, pos.y, pos.z - HW);
    let mut mx = DVec3::new(pos.x + HW, pos.y + h, pos.z + HW);
    let dy = collide_axis(world, mn, mx, 1, mv.y);
    mn.y += dy;
    mx.y += dy;
    let (mut dx, mut dz) = (mv.x, mv.z);
    if mv.x.abs() < mv.z.abs() {
        dz = collide_axis(world, mn, mx, 2, dz);
        mn.z += dz;
        mx.z += dz;
        dx = collide_axis(world, mn, mx, 0, dx);
    } else {
        dx = collide_axis(world, mn, mx, 0, dx);
        mn.x += dx;
        mx.x += dx;
        dz = collide_axis(world, mn, mx, 2, dz);
    }
    DVec3::new(dx, dy, dz)
}

impl Player {
    pub fn new() -> Self {
        let pos = DVec3::new(W as f64 / 2.0, PLATFORM_TOP as f64, D as f64 / 2.0);
        Self {
            pos,
            prev_pos: pos,
            vel: DVec3::ZERO,
            yaw: 0.0,
            pitch: 0.0,
            on_ground: false,
            flying: false,
            sprinting: false,
            crouching: false,
            horizontal_collision: false,
            no_jump_delay: 0,
            sprint_trigger: 0,
            jump_trigger: 0,
            last_jump: false,
            last_sneak: false,
            last_fwd_enough: false,
            eye: 1.62,
            eye_old: 1.62,
            fov_mod: 1.0,
            fov_old: 1.0,
        }
    }

    fn height(&self) -> f64 {
        if self.crouching { CROUCH_H } else { STAND_H }
    }

    pub fn forward(&self) -> Vec3 {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        Vec3::new(sy * cp, sp, -cy * cp)
    }

    /// Позиция камеры с интерполяцией между тиками.
    pub fn cam_pos(&self, a: f32) -> Vec3 {
        let p = self.prev_pos.lerp(self.pos, a as f64);
        let e = self.eye_old + (self.eye - self.eye_old) * a;
        Vec3::new(p.x as f32, p.y as f32 + e, p.z as f32)
    }

    pub fn fov_mult(&self, a: f32) -> f32 {
        self.fov_old + (self.fov_mod - self.fov_old) * a
    }

    /// Нельзя ставить блок внутри игрока.
    pub fn intersects_block(&self, c: IVec3) -> bool {
        let h = self.height();
        let (bx, by, bz) = (c.x as f64, c.y as f64, c.z as f64);
        self.pos.x - HW < bx + 1.0
            && self.pos.x + HW > bx
            && self.pos.y < by + 1.0
            && self.pos.y + h > by
            && self.pos.z - HW < bz + 1.0
            && self.pos.z + HW > bz
    }

    fn jump(&mut self) {
        self.vel.y = 0.42;
        if self.sprinting {
            // в ваниле: (-sin(yaw)*0.2, 0, cos(yaw)*0.2); у нас yaw=0 смотрит в -Z
            let (s, c) = (self.yaw.sin() as f64, self.yaw.cos() as f64);
            self.vel.x += s * 0.2;
            self.vel.z += -c * 0.2;
        }
    }

    /// Один игровой тик (1/20 с).
    pub fn tick(&mut self, world: &World, inp: &Input) {
        self.prev_pos = self.pos;
        self.eye_old = self.eye;
        self.fov_old = self.fov_mod;
        if self.sprint_trigger > 0 {
            self.sprint_trigger -= 1;
        }
        if self.jump_trigger > 0 {
            self.jump_trigger -= 1;
        }

        // двойное нажатие пробела — режим полёта
        if inp.jump && !self.last_jump {
            if self.jump_trigger == 0 {
                self.jump_trigger = 7;
            } else {
                self.flying = !self.flying;
                if self.flying && self.on_ground {
                    self.jump();
                }
                self.jump_trigger = 0;
            }
        }
        self.last_jump = inp.jump;

        // поза: присед, если зажат Shift или над головой нет места встать
        let stand_box_blocked = overlaps(
            world,
            DVec3::new(self.pos.x - HW, self.pos.y + EPS, self.pos.z - HW),
            DVec3::new(self.pos.x + HW, self.pos.y + STAND_H - EPS, self.pos.z + HW),
        );
        self.crouching = !self.flying && (inp.sneak || stand_box_blocked);

        // ввод
        let mut fwd = (inp.fwd as i32 - inp.back as i32) as f32;
        let mut strafe = (inp.left as i32 - inp.right as i32) as f32; // + = влево
        if self.crouching {
            fwd *= 0.3;
            strafe *= 0.3;
        }

        // бег: Ctrl или двойное нажатие W
        let fwd_enough = fwd >= 0.8;
        if self.on_ground && !self.last_sneak && !self.last_fwd_enough && fwd_enough && !self.sprinting {
            if self.sprint_trigger <= 0 && !inp.sprint_key {
                self.sprint_trigger = 7;
            } else {
                self.sprinting = true;
            }
        }
        if !self.sprinting && fwd_enough && inp.sprint_key {
            self.sprinting = true;
        }
        if self.sprinting && (!(fwd > 1e-5) || self.horizontal_collision) {
            self.sprinting = false;
        }
        if self.crouching {
            self.sprinting = false;
        }
        self.last_fwd_enough = fwd_enough;
        self.last_sneak = inp.sneak;

        // вертикаль в полёте
        if self.flying {
            let mut i = 0;
            if inp.sneak {
                i -= 1;
            }
            if inp.jump {
                i += 1;
            }
            if i != 0 {
                self.vel.y += i as f64 * (0.05f32 * 3.0f32) as f64;
            }
        }

        // --- aiStep ---
        if self.no_jump_delay > 0 {
            self.no_jump_delay -= 1;
        }
        if self.vel.x.abs() < 0.003 { self.vel.x = 0.0; }
        if self.vel.y.abs() < 0.003 { self.vel.y = 0.0; }
        if self.vel.z.abs() < 0.003 { self.vel.z = 0.0; }

        if inp.jump {
            if self.on_ground && self.no_jump_delay == 0 {
                self.jump();
                self.no_jump_delay = 10;
            }
        } else {
            self.no_jump_delay = 0;
        }

        let xxa = (strafe * 0.98) as f64;
        let zza = (fwd * 0.98) as f64;

        // --- travel ---
        let vy_before = self.vel.y;
        let block_friction: f32 = 0.6;
        let friction: f32 = if self.on_ground { block_friction * 0.91 } else { 0.91 };
        let speed: f32 = if self.sprinting { 0.13 } else { 0.1 };
        let accel: f32 = if self.on_ground {
            speed * (0.21600002 / (block_friction * block_friction * block_friction))
        } else if self.flying {
            if self.sprinting { 0.1 } else { 0.05 }
        } else if self.sprinting {
            0.026
        } else {
            0.02
        };

        // moveRelative
        let d = xxa * xxa + zza * zza;
        if d >= 1e-7 {
            let (mut ix, mut iz) = (xxa, zza);
            if d > 1.0 {
                let l = d.sqrt();
                ix /= l;
                iz /= l;
            }
            ix *= accel as f64;
            iz *= accel as f64;
            let (s, c) = (self.yaw.sin() as f64, self.yaw.cos() as f64);
            let fwd_v = DVec3::new(s, 0.0, -c);
            let left_v = DVec3::new(-c, 0.0, -s);
            self.vel += fwd_v * iz + left_v * ix;
        }

        self.move_entity(world, self.vel, inp.sneak);

        let v = self.vel;
        self.vel = DVec3::new(
            v.x * friction as f64,
            (v.y - 0.08) * 0.98,
            v.z * friction as f64,
        );
        if self.flying {
            self.vel.y = vy_before * 0.6;
        }

        if self.on_ground && self.flying {
            self.flying = false;
        }

        // плавные эффекты камеры
        self.eye += ((if self.crouching { 1.27 } else { 1.62 }) - self.eye) * 0.5;
        let mut f = 1.0f32;
        if self.flying {
            f *= 1.1;
        }
        let speed_attr = if self.sprinting { 0.13f32 } else { 0.1 };
        f *= (speed_attr / 0.1 + 1.0) / 2.0;
        self.fov_mod += (f.clamp(0.1, 1.5) - self.fov_mod) * 0.5;
    }

    fn move_entity(&mut self, world: &World, mv: DVec3, shift_key: bool) {
        let h = self.height();
        let mut mv = mv;
        if shift_key && self.on_ground && mv.y <= 0.0 && !self.flying {
            mv = self.back_off(world, mv, h);
        }
        let c = collide(world, self.pos, h, mv);
        self.pos += c;

        let eq = |a: f64, b: f64| (a - b).abs() < 1e-5;
        self.horizontal_collision = !eq(mv.x, c.x) || !eq(mv.z, c.z);
        let vertical = mv.y != c.y;
        self.on_ground = vertical && mv.y < 0.0;
        if mv.x != c.x {
            self.vel.x = 0.0;
        }
        if mv.z != c.z {
            self.vel.z = 0.0;
        }
        if vertical {
            self.vel.y = 0.0;
        }
    }

    /// Не даём приседающему игроку упасть с края (Entity.maybeBackOffFromEdge).
    fn back_off(&self, world: &World, mv: DVec3, _h: f64) -> DVec3 {
        let max_down = 0.6;
        let can_fall = |dx: f64, dz: f64| -> bool {
            let mn = DVec3::new(
                self.pos.x - HW + EPS + dx,
                self.pos.y - max_down - EPS,
                self.pos.z - HW + EPS + dz,
            );
            let mx = DVec3::new(self.pos.x + HW - EPS + dx, self.pos.y, self.pos.z + HW - EPS + dz);
            !overlaps(world, mn, mx)
        };
        let (mut d, mut d1) = (mv.x, mv.z);
        let (s0, s1) = (d.signum() * 0.05, d1.signum() * 0.05);
        while d != 0.0 && can_fall(d, 0.0) {
            if d.abs() <= 0.05 {
                d = 0.0;
                break;
            }
            d -= s0;
        }
        while d1 != 0.0 && can_fall(0.0, d1) {
            if d1.abs() <= 0.05 {
                d1 = 0.0;
                break;
            }
            d1 -= s1;
        }
        while d != 0.0 && d1 != 0.0 && can_fall(d, d1) {
            d = if d.abs() <= 0.05 { 0.0 } else { d - s0 };
            d1 = if d1.abs() <= 0.05 { 0.0 } else { d1 - s1 };
        }
        DVec3::new(d, mv.y, d1)
    }
}
