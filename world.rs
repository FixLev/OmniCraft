use bytemuck::{Pod, Zeroable};
use glam::{IVec3, Vec3};

pub const W: usize = 64;
pub const H: usize = 16;
pub const D: usize = 64;
pub const CS: usize = 16;
pub const CX: usize = W / CS;
pub const CY: usize = H / CS;
pub const CZ: usize = D / CS;
pub const CHUNKS: usize = CX * CY * CZ;
/// Блоки с y < PLATFORM_TOP заполнены (платформа).
pub const PLATFORM_TOP: i32 = 4;
pub const BLOCK_COLOR: [f32; 3] = [0.56, 0.62, 0.70];

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub col: [f32; 3],
}

pub struct Hit {
    pub block: IVec3,
    pub prev: IVec3,
}

struct Face {
    dir: [i32; 3],
    corners: [[f32; 3]; 4],
    shade: f32,
}

const FACES: [Face; 6] = [
    Face { dir: [1, 0, 0], corners: [[1., 0., 0.], [1., 1., 0.], [1., 1., 1.], [1., 0., 1.]], shade: 0.80 },
    Face { dir: [-1, 0, 0], corners: [[0., 0., 1.], [0., 1., 1.], [0., 1., 0.], [0., 0., 0.]], shade: 0.60 },
    Face { dir: [0, 1, 0], corners: [[0., 1., 1.], [1., 1., 1.], [1., 1., 0.], [0., 1., 0.]], shade: 1.00 },
    Face { dir: [0, -1, 0], corners: [[0., 0., 0.], [1., 0., 0.], [1., 0., 1.], [0., 0., 1.]], shade: 0.45 },
    Face { dir: [0, 0, 1], corners: [[1., 0., 1.], [1., 1., 1.], [0., 1., 1.], [0., 0., 1.]], shade: 0.70 },
    Face { dir: [0, 0, -1], corners: [[0., 0., 0.], [0., 1., 0.], [1., 1., 0.], [1., 0., 0.]], shade: 0.90 },
];

pub struct World {
    blocks: Vec<bool>,
    pub dirty: [bool; CHUNKS],
}

fn hash(x: i32, y: i32, z: i32) -> f32 {
    let mut h = (x as u32).wrapping_mul(73856093)
        ^ (y as u32).wrapping_mul(19349663)
        ^ (z as u32).wrapping_mul(83492791);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1e995);
    h ^= h >> 15;
    (h & 0xffff) as f32 / 65535.0
}

impl World {
    pub fn new() -> Self {
        let mut blocks = vec![false; W * H * D];
        for y in 0..PLATFORM_TOP as usize {
            for z in 0..D {
                for x in 0..W {
                    blocks[(y * D + z) * W + x] = true;
                }
            }
        }
        Self { blocks, dirty: [true; CHUNKS] }
    }

    #[inline]
    fn in_bounds(x: i32, y: i32, z: i32) -> bool {
        x >= 0 && y >= 0 && z >= 0 && (x as usize) < W && (y as usize) < H && (z as usize) < D
    }

    #[inline]
    pub fn get(&self, x: i32, y: i32, z: i32) -> bool {
        Self::in_bounds(x, y, z) && self.blocks[(y as usize * D + z as usize) * W + x as usize]
    }

    fn mark(&mut self, cx: i32, cy: i32, cz: i32) {
        if cx >= 0 && cy >= 0 && cz >= 0 && (cx as usize) < CX && (cy as usize) < CY && (cz as usize) < CZ {
            self.dirty[(cy as usize * CZ + cz as usize) * CX + cx as usize] = true;
        }
    }

    /// Возвращает false, если клетка вне мира.
    pub fn set(&mut self, x: i32, y: i32, z: i32, v: bool) -> bool {
        if !Self::in_bounds(x, y, z) {
            return false;
        }
        self.blocks[(y as usize * D + z as usize) * W + x as usize] = v;
        let (cs, ics) = (CS as i32, CS as i32);
        let (cx, cy, cz) = (x / cs, y / cs, z / cs);
        self.mark(cx, cy, cz);
        let (lx, ly, lz) = (x % ics, y % ics, z % ics);
        if lx == 0 { self.mark(cx - 1, cy, cz); }
        if lx == ics - 1 { self.mark(cx + 1, cy, cz); }
        if ly == 0 { self.mark(cx, cy - 1, cz); }
        if ly == ics - 1 { self.mark(cx, cy + 1, cz); }
        if lz == 0 { self.mark(cx, cy, cz - 1); }
        if lz == ics - 1 { self.mark(cx, cy, cz + 1); }
        true
    }

    /// Обход вокселей по лучу (Amanatides–Woo).
    pub fn raycast(&self, origin: Vec3, dir: Vec3, max: f32) -> Option<Hit> {
        let mut p = IVec3::new(origin.x.floor() as i32, origin.y.floor() as i32, origin.z.floor() as i32);
        let step = IVec3::new(
            if dir.x > 0.0 { 1 } else { -1 },
            if dir.y > 0.0 { 1 } else { -1 },
            if dir.z > 0.0 { 1 } else { -1 },
        );
        let inv = |d: f32| if d != 0.0 { 1.0 / d.abs() } else { f32::INFINITY };
        let inv = Vec3::new(inv(dir.x), inv(dir.y), inv(dir.z));
        let first = |o: f32, d: f32, c: i32, inv: f32| {
            if d == 0.0 {
                f32::INFINITY
            } else if d > 0.0 {
                ((c + 1) as f32 - o) * inv
            } else {
                (o - c as f32) * inv
            }
        };
        let mut tmax = Vec3::new(
            first(origin.x, dir.x, p.x, inv.x),
            first(origin.y, dir.y, p.y, inv.y),
            first(origin.z, dir.z, p.z, inv.z),
        );
        let mut prev = p;
        loop {
            if self.get(p.x, p.y, p.z) {
                return Some(Hit { block: p, prev });
            }
            let axis = if tmax.x < tmax.y {
                if tmax.x < tmax.z { 0 } else { 2 }
            } else if tmax.y < tmax.z {
                1
            } else {
                2
            };
            if tmax[axis] > max {
                return None;
            }
            prev = p;
            match axis {
                0 => { p.x += step.x; tmax.x += inv.x; }
                1 => { p.y += step.y; tmax.y += inv.y; }
                _ => { p.z += step.z; tmax.z += inv.z; }
            }
        }
    }

    pub fn mesh_chunk(&self, ci: usize) -> (Vec<Vertex>, Vec<u32>) {
        let cx = ci % CX;
        let cz = (ci / CX) % CZ;
        let cy = ci / (CX * CZ);
        let (ox, oy, oz) = ((cx * CS) as i32, (cy * CS) as i32, (cz * CS) as i32);
        let mut verts: Vec<Vertex> = Vec::new();
        let mut idx: Vec<u32> = Vec::new();
        for ly in 0..CS as i32 {
            for lz in 0..CS as i32 {
                for lx in 0..CS as i32 {
                    let (x, y, z) = (ox + lx, oy + ly, oz + lz);
                    if !self.get(x, y, z) {
                        continue;
                    }
                    let tint = 0.94 + 0.12 * hash(x, y, z);
                    for f in FACES.iter() {
                        let (nx, ny, nz) = (x + f.dir[0], y + f.dir[1], z + f.dir[2]);
                        // нижнюю границу мира считаем сплошной — не рисуем дно
                        let hidden = if ny < 0 { true } else { self.get(nx, ny, nz) };
                        if hidden {
                            continue;
                        }
                        let base = verts.len() as u32;
                        let s = f.shade * tint;
                        for c in f.corners.iter() {
                            verts.push(Vertex {
                                pos: [x as f32 + c[0], y as f32 + c[1], z as f32 + c[2]],
                                col: [BLOCK_COLOR[0] * s, BLOCK_COLOR[1] * s, BLOCK_COLOR[2] * s],
                            });
                        }
                        idx.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
                    }
                }
            }
        }
        (verts, idx)
    }
}
