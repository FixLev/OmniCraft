struct U {
    vp: mat4x4<f32>,
    fog: vec4<f32>,   // rgb — цвет тумана, w — дальность
    cam: vec4<f32>,
};
@group(0) @binding(0) var<uniform> u: U;

struct VIn {
    @location(0) pos: vec3<f32>,
    @location(1) col: vec3<f32>,
};

struct VOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) col: vec3<f32>,
    @location(1) wp: vec3<f32>,
};

@vertex
fn vs(v: VIn) -> VOut {
    var o: VOut;
    o.clip = u.vp * vec4<f32>(v.pos, 1.0);
    o.col = v.col;
    o.wp = v.pos;
    return o;
}

@fragment
fn fs(i: VOut) -> @location(0) vec4<f32> {
    // Расстояние до ближайшего ребра грани: медиана из трёх «расстояний до целой координаты»
    // (ось, перпендикулярная грани, даёт ~0, остальные две — позиция внутри грани).
    let f = fract(i.wp);
    let d = min(f, vec3<f32>(1.0) - f);
    let m = max(min(d.x, d.y), min(max(d.x, d.y), d.z));
    let e = smoothstep(0.0, 0.07, m);
    var c = i.col * mix(0.72, 1.0, e);

    let dist = length(i.wp - u.cam.xyz);
    let t = clamp((dist - u.fog.w * 0.4) / (u.fog.w * 0.6), 0.0, 1.0);
    c = mix(c, u.fog.rgb, t * t);
    return vec4<f32>(c, 1.0);
}

@fragment
fn fs_line(i: VOut) -> @location(0) vec4<f32> {
    return vec4<f32>(i.col, 1.0);
}
