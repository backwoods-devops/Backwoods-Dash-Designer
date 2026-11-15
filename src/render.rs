// 3D viewport: camera maths, CPU picking and the OpenGL renderer used through an egui paint callback.
use eframe::glow::{self, HasContext};
use std::sync::{Arc, Mutex};

pub type V3 = [f32; 3];

// ------------------------------------------------------------------ vectors / matrices (column-major)
pub fn vsub(a: V3, b: V3) -> V3 { [a[0] - b[0], a[1] - b[1], a[2] - b[2]] }
pub fn vadd(a: V3, b: V3) -> V3 { [a[0] + b[0], a[1] + b[1], a[2] + b[2]] }
pub fn vmul(a: V3, s: f32) -> V3 { [a[0] * s, a[1] * s, a[2] * s] }
pub fn dot(a: V3, b: V3) -> f32 { a[0] * b[0] + a[1] * b[1] + a[2] * b[2] }
pub fn cross(a: V3, b: V3) -> V3 { [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]] }
pub fn normalize(a: V3) -> V3 {
    let l = dot(a, a).sqrt();
    if l < 1e-9 { [0.0, 0.0, 1.0] } else { vmul(a, 1.0 / l) }
}

pub type Mat4 = [f32; 16];

pub fn mat_mul(a: &Mat4, b: &Mat4) -> Mat4 {
    let mut o = [0.0f32; 16];
    for c in 0..4 {
        for r in 0..4 {
            let mut s = 0.0;
            for k in 0..4 {
                s += a[k * 4 + r] * b[c * 4 + k];
            }
            o[c * 4 + r] = s;
        }
    }
    o
}

pub fn perspective(fov: f32, aspect: f32, near: f32, far: f32) -> Mat4 {
    let t = 1.0 / (fov / 2.0).tan();
    let mut o = [0.0f32; 16];
    o[0] = t / aspect;
    o[5] = t;
    o[10] = (far + near) / (near - far);
    o[11] = -1.0;
    o[14] = 2.0 * far * near / (near - far);
    o
}

pub struct Basis {
    pub m: Mat4,
    pub x: V3,
    pub y: V3,
    pub z: V3,
}

pub fn look_at(eye: V3, target: V3, up: V3) -> Basis {
    let z = normalize(vsub(eye, target));
    let x = normalize(cross(up, z));
    let y = cross(z, x);
    let mut o = [0.0f32; 16];
    o[0] = x[0];
    o[4] = x[1];
    o[8] = x[2];
    o[12] = -dot(x, eye);
    o[1] = y[0];
    o[5] = y[1];
    o[9] = y[2];
    o[13] = -dot(y, eye);
    o[2] = z[0];
    o[6] = z[1];
    o[10] = z[2];
    o[14] = -dot(z, eye);
    o[15] = 1.0;
    Basis { m: o, x, y, z }
}

// ------------------------------------------------------------------ camera
#[derive(Clone, Copy)]
pub struct Camera {
    pub yaw: f32,
    pub pitch: f32,
    pub dist: f32,
    pub target: V3,
}

pub const FOV: f32 = 0.62;

impl Camera {
    pub fn home() -> Camera {
        Camera { yaw: -0.5, pitch: 0.42, dist: 1750.0, target: [0.0, 90.0, 330.0] }
    }
    pub fn eye(&self) -> V3 {
        let cp = self.pitch.cos();
        [
            self.target[0] + self.dist * self.yaw.sin() * cp,
            self.target[1] - self.dist * self.yaw.cos() * cp,
            self.target[2] + self.dist * self.pitch.sin(),
        ]
    }
    pub fn basis(&self) -> Basis {
        look_at(self.eye(), self.target, [0.0, 0.0, 1.0])
    }
    pub fn view_proj(&self, aspect: f32) -> (Mat4, Mat4) {
        let b = self.basis();
        (mat_mul(&perspective(FOV, aspect, 20.0, 20000.0), &b.m), b.m)
    }
    /// World ray through a point inside a viewport of the given size (points, origin top-left).
    pub fn ray(&self, px: f32, py: f32, w: f32, h: f32) -> (V3, V3) {
        let b = self.basis();
        let tx = (FOV / 2.0).tan();
        let nx = (2.0 * px / w - 1.0) * tx * (w / h);
        let ny = (1.0 - 2.0 * py / h) * tx;
        let d = [
            -b.z[0] + nx * b.x[0] + ny * b.y[0],
            -b.z[1] + nx * b.x[1] + ny * b.y[1],
            -b.z[2] + nx * b.x[2] + ny * b.y[2],
        ];
        (self.eye(), normalize(d))
    }
    /// Project a world point to viewport points (origin top-left). None when behind the camera.
    pub fn project(&self, p: V3, w: f32, h: f32) -> Option<[f32; 2]> {
        let (vp, _) = self.view_proj(w / h);
        let m = &vp;
        let x = m[0] * p[0] + m[4] * p[1] + m[8] * p[2] + m[12];
        let y = m[1] * p[0] + m[5] * p[1] + m[9] * p[2] + m[13];
        let wv = m[3] * p[0] + m[7] * p[1] + m[11] * p[2] + m[15];
        if wv <= 1.0 {
            return None;
        }
        Some([(x / wv * 0.5 + 0.5) * w, (1.0 - (y / wv * 0.5 + 0.5)) * h])
    }
    pub fn orbit(&mut self, dx: f32, dy: f32) {
        self.yaw -= dx * 0.006;
        self.pitch = (self.pitch + dy * 0.006).clamp(-1.5, 1.5);
    }
    pub fn pan(&mut self, dx: f32, dy: f32) {
        let b = self.basis();
        let k = self.dist * 0.0011;
        for i in 0..3 {
            self.target[i] += (-b.x[i] * dx + b.y[i] * dy) * k;
        }
    }
    pub fn zoom(&mut self, scroll: f32) {
        self.dist = (self.dist * (-scroll * 0.0012).exp()).clamp(200.0, 9000.0);
    }
}

// ------------------------------------------------------------------ CPU mesh for picking and placement
pub struct CpuMesh {
    pub pos: Vec<f32>,
    pub nrm: Vec<f32>,
    pub idx: Vec<u32>,
}

#[derive(Clone, Copy, Debug)]
pub struct Hit {
    pub p: V3,
    pub n: V3,
}

impl CpuMesh {
    fn v(&self, i: u32) -> V3 {
        let k = i as usize * 3;
        [self.pos[k], self.pos[k + 1], self.pos[k + 2]]
    }
    fn n(&self, i: u32) -> V3 {
        let k = i as usize * 3;
        [self.nrm[k], self.nrm[k + 1], self.nrm[k + 2]]
    }
    /// Nearest front hit of a ray (Moller-Trumbore, both faces).
    pub fn raycast(&self, o: V3, d: V3) -> Option<Hit> {
        let mut best: Option<(f32, f32, f32, usize)> = None;
        for t in (0..self.idx.len()).step_by(3) {
            let (a, b, c) = (self.v(self.idx[t]), self.v(self.idx[t + 1]), self.v(self.idx[t + 2]));
            let e1 = vsub(b, a);
            let e2 = vsub(c, a);
            let pv = cross(d, e2);
            let det = dot(e1, pv);
            if det.abs() < 1e-9 {
                continue;
            }
            let inv = 1.0 / det;
            let tv = vsub(o, a);
            let u = dot(tv, pv) * inv;
            if !(0.0..=1.0).contains(&u) {
                continue;
            }
            let qv = cross(tv, e1);
            let v = dot(d, qv) * inv;
            if v < 0.0 || u + v > 1.0 {
                continue;
            }
            let tt = dot(e2, qv) * inv;
            if tt > 0.0 && best.map_or(true, |bb| tt < bb.0) {
                best = Some((tt, u, v, t));
            }
        }
        best.map(|(tt, u, v, t)| {
            let (i0, i1, i2) = (self.idx[t], self.idx[t + 1], self.idx[t + 2]);
            let (n0, n1, n2) = (self.n(i0), self.n(i1), self.n(i2));
            let w = 1.0 - u - v;
            let n = normalize([
                n0[0] * w + n1[0] * u + n2[0] * v,
                n0[1] * w + n1[1] * u + n2[1] * v,
                n0[2] * w + n1[2] * u + n2[2] * v,
            ]);
            Hit { p: vadd(o, vmul(d, tt)), n }
        })
    }
    /// Highest surface point straight above/below (x, y) (the dash top).
    pub fn surface_at(&self, x: f32, y: f32) -> Option<Hit> {
        self.raycast([x, y, 5000.0], [0.0, 0.0, -1.0])
    }
}

// ------------------------------------------------------------------ vertex format and scene data
#[derive(Clone, Copy, Default)]
#[repr(C)]
pub struct Vtx {
    pub p: V3,
    pub n: V3,
    pub c: V3,
}

#[derive(Clone, Copy)]
pub struct MatU {
    pub col: [f32; 3],
    pub a: [f32; 4],
    pub b: [f32; 4],
}
impl Default for MatU {
    fn default() -> MatU {
        MatU { col: [1.0, 1.0, 1.0], a: [0.6, 0.0, 0.0, 0.0], b: [1.0, 0.0, 0.0, 0.0] }
    }
}

#[derive(Default)]
pub struct SceneData {
    pub skin_mat: MatU,
    pub other_mat: MatU,
    pub cover_mat: MatU,
    pub cover_alpha: f32,
    pub side_mat: MatU,
    pub side: Vec<Vtx>,
    pub side_idx: Vec<u32>,
    pub side_ver: u64,
    pub show_side: bool,
    pub span_mix: Vec<f32>,
    pub part_ranges: Vec<(usize, usize, MatU)>,
    pub skin: Vec<Vtx>,
    pub skin_idx: Vec<u32>,
    pub skin_ver: u64,
    pub span_tris: usize, // triangles per profile span
    pub span_colors: Vec<[f32; 3]>,
    pub parts: Vec<Vtx>,
    pub parts_ver: u64,
    pub preview: Vec<Vtx>,
    pub preview_ver: u64,
    pub other: Vec<Vtx>,
    pub other_idx: Vec<u32>,
    pub other_ver: u64,
    pub show_other: bool,
    pub cover: Vec<Vtx>,
    pub cover_idx: Vec<u32>,
    pub cover_ver: u64,
    pub show_cover: bool,
    pub show_skin: bool,
    /// Index where the inner wall surface starts in skin_idx (0 = skin only, no wall).
    pub wall_from: usize,
    pub clip_on: bool,
    pub clip_x: f32,
    pub clip_dir: f32,
    /// key, ambient, fill, reflections / exposure, texture, lights fixed to the scene / key light direction
    pub light_a: [f32; 4],
    pub light_b: [f32; 4],
    pub key_dir: [f32; 3],
    pub show_parts: bool,
    /// The piece with its cuts applied (a closed solid), drawn instead of the skin when set.
    pub result: Vec<Vtx>,
    pub result_ver: u64,
    pub show_result: bool,
    pub result_cap: bool,
    /// Inlay plugs.
    pub plug: Vec<Vtx>,
    pub plug_ver: u64,
    /// The 3D parts themselves, drawn when the boolean result is not on screen.
    pub objs: Vec<Vtx>,
    pub objs_ver: u64,
    pub show_objs: bool,
    pub view_proj: Mat4,
    pub view: Mat4,
}

// ------------------------------------------------------------------ GL resources
struct Buf {
    vao: glow::VertexArray,
    vbo: glow::Buffer,
    ebo: Option<glow::Buffer>,
    count: i32,
}

pub struct GlRes {
    prog: glow::Program,
    u_vp: Option<glow::UniformLocation>,
    u_v: Option<glow::UniformLocation>,
    u_col: Option<glow::UniformLocation>,
    u_tint: Option<glow::UniformLocation>,
    u_ma: Option<glow::UniformLocation>,
    u_mb: Option<glow::UniformLocation>,
    u_clip: Option<glow::UniformLocation>,
    u_la: Option<glow::UniformLocation>,
    u_lb: Option<glow::UniformLocation>,
    u_kd: Option<glow::UniformLocation>,
    skin: Option<Buf>,
    parts: Option<Buf>,
    preview: Option<Buf>,
    other: Option<Buf>,
    cover: Option<Buf>,
    side: Option<Buf>,
    result: Option<Buf>,
    plug: Option<Buf>,
    objs: Option<Buf>,
    seen: [u64; 9],
}

const VS: &str = r#"#version 330 core
layout(location=0) in vec3 aP; layout(location=1) in vec3 aN; layout(location=2) in vec3 aC;
uniform mat4 uVP; uniform mat4 uV; out vec3 vNw; out vec3 vC; out vec3 vP; out vec3 vVp;
void main(){ vNw = aN; vC = aC; vP = aP; vVp = (uV * vec4(aP, 1.0)).xyz; gl_Position = uVP * vec4(aP, 1.0); }"#;
const FS: &str = r#"#version 330 core
in vec3 vNw; in vec3 vC; in vec3 vP; in vec3 vVp;
uniform mat4 uV;
uniform vec4 uCol;   // rgb base color (sRGB), a = opacity
uniform vec4 uTint;  // rgb section color, w = mix amount
uniform vec4 uMatA;  // roughness, metal, coat, grain kind
uniform vec4 uMatB;  // grain cell (mm), grain depth
uniform vec4 uLA;    // key, ambient, fill, reflections
uniform vec4 uLB;    // exposure, texture strength, lights fixed to scene
uniform vec3 uKD;    // key light direction
uniform vec4 uClip;  // x on, y cut at world x, z cut faces drawn flat, w side (+1 drops x above)
out vec4 o;

vec3 hash3(vec3 p){
  p = vec3(dot(p,vec3(127.1,311.7,74.7)), dot(p,vec3(269.5,183.3,246.1)), dot(p,vec3(113.5,271.9,124.6)));
  return fract(sin(p)*43758.5453123);
}
// returns (F1, F2) distances of 3D voronoi
vec2 vor(vec3 x){
  vec3 ip = floor(x), fp = fract(x);
  float d1 = 8.0, d2 = 8.0;
  for(int k=-1;k<=1;k++) for(int j=-1;j<=1;j++) for(int i=-1;i<=1;i++){
    vec3 g = vec3(float(i),float(j),float(k));
    vec3 r = g + hash3(ip+g) - fp;
    float d = dot(r,r);
    if(d<d1){ d2=d1; d1=d; } else if(d<d2){ d2=d; }
  }
  return vec2(sqrt(d1), sqrt(d2));
}
float vnoise(vec3 x){
  vec3 i = floor(x), f = fract(x); f = f*f*(3.0-2.0*f);
  float a = hash3(i).x, b = hash3(i+vec3(1,0,0)).x, c = hash3(i+vec3(0,1,0)).x, d = hash3(i+vec3(1,1,0)).x;
  float e = hash3(i+vec3(0,0,1)).x, g = hash3(i+vec3(1,0,1)).x, h = hash3(i+vec3(0,1,1)).x, k = hash3(i+vec3(1,1,1)).x;
  return mix(mix(mix(a,b,f.x),mix(c,d,f.x),f.y), mix(mix(e,g,f.x),mix(h,k,f.x),f.y), f.z);
}
float grainH(int kind, vec3 p, float cell){
  if(kind==1){ vec2 v = vor(p/cell*1.0); return 0.5*v.x + 0.5*vnoise(p/(cell*3.6)); }
  if(kind==2){ vec2 v = vor(p/cell); float crease = smoothstep(0.0, 0.6, v.y - v.x); return crease*0.8 + 0.2*vnoise(p/(cell*0.35)); }
  if(kind==3){ return vnoise(p/cell) + 0.5*vnoise(p/(cell*0.4)); }
  if(kind==4){ return vnoise(vec3(p.x/(cell*40.0), p.y/cell, p.z/cell)) + 0.5*vnoise(vec3(p.x/(cell*14.0), p.y/(cell*0.5), p.z/(cell*0.5))); }
  if(kind==6){ vec3 q = p/cell; float a = sin((q.x+q.y)*3.14159), b = sin((q.x-q.y)*3.14159); return 0.5+0.25*(a*b) ; }
  return 0.0;
}
vec3 env(vec3 R, float rough){
  float t = clamp(R.z*0.5+0.5, 0.0, 1.0);
  vec3 sky = mix(vec3(0.015,0.016,0.02), vec3(0.34,0.36,0.42), smoothstep(0.42,1.0,t));
  sky += vec3(0.10,0.09,0.08) * smoothstep(0.55,0.46,t) * smoothstep(0.2,0.46,t);
  float sh = mix(70.0, 3.0, rough);
  float top = pow(max(dot(R, normalize(vec3(0.15,-0.35,0.92))),0.0), sh) * 3.2;
  float l1  = pow(max(dot(R, normalize(vec3(-0.85,0.15,0.5))),0.0), sh*0.5) * 1.5;
  float l2  = pow(max(dot(R, normalize(vec3(0.85,0.25,0.45))),0.0), sh*0.5) * 1.1;
  vec3 sharp = sky + vec3(top) + vec3(l1)*vec3(1.0,0.97,0.92) + vec3(l2)*vec3(0.9,0.95,1.0);
  vec3 avg = vec3(0.16,0.17,0.2);
  return mix(sharp, avg, rough*rough*0.85);
}
void main(){
  if(uClip.x > 0.5 && (vP.x - uClip.y) * uClip.w > 0.0) discard;
  if(uClip.x > 0.5 && uClip.z > 0.5 && !gl_FrontFacing){ o = vec4(0.93, 0.52, 0.14, 1.0); return; }
  vec3 Nw = normalize(vNw); if(!gl_FrontFacing) Nw = -Nw;
  float rough = uMatA.x, metal = uMatA.y, coat = uMatA.z; int kind = int(uMatA.w+0.5);
  float cell = max(uMatB.x, 0.05), depth = uMatB.y;
  vec3 col = uCol.rgb * vC;
  if(uTint.w > 0.0) col = mix(col, uTint.rgb, uTint.w);
  // grain / bump (derivative based, no tangents needed)
  depth = min(depth * uLB.y, 1.6);
  if(kind>0 && depth>0.0){
    vec3 dpx = dFdx(vP), dpy = dFdy(vP);
    float fp = max(length(dpx), length(dpy));
    float fade = 1.0 - smoothstep(0.12*cell, 0.5*cell, fp);
    if(fade > 0.01){
      float hh = grainH(kind, vP, cell);
      float h = hh * cell * 0.35 * depth;
      if(kind==2 || kind==1) col *= 1.0 - 0.5*clamp(depth,0.0,1.0)*fade*(1.0 - clamp(hh,0.0,1.0));
      float dhx = dFdx(h), dhy = dFdy(h);
      vec3 r1 = cross(dpy, Nw), r2 = cross(Nw, dpx);
      float det = dot(dpx, r1);
      vec3 grad = sign(det) * (dhx*r1 + dhy*r2);
      Nw = normalize(abs(det)*Nw - fade*grad);
    }
  }
  if(kind==5){ // oak: grain runs along x; arched growth rings, open pores as short dark dashes, long pale streaks
    vec3 q = vP;
    float ac = q.y + 0.55*q.z;                       // across the grain
    // growth rings. Slow noise bends and spaces them so the arches never repeat
    float sp = 0.75 + 0.5*vnoise(vec3(q.x/700.0, ac/160.0, 3.7));
    float r = ac/26.0*sp + 5.5*vnoise(vec3(q.x/420.0, ac/150.0, 1.3)) + 1.6*vnoise(vec3(q.x/150.0, ac/50.0, 8.1)) + 0.4*vnoise(vec3(q.x/50.0, ac/16.0, 5.2));
    float rg = sin(r*6.2832) + 0.45*sin(r*12.566 + 1.3);       // -1.45 .. 1.45
    float late = smoothstep(0.2, 1.2, rg);                      // dense latewood band
    float early = 1.0 - late;
    // long streaks of light and dark fibre
    float fib = vnoise(vec3(q.x/55.0, ac/0.9, q.z/0.9));
    float fib2 = vnoise(vec3(q.x/16.0, ac/0.32, q.z/0.32));
    // open pores: short dashes, mostly in the earlywood
    float pn = vnoise(vec3(q.x/16.0, ac/0.5, q.z/0.5)) * 0.7 + 0.3*vnoise(vec3(q.x/5.0, ac/0.35, q.z/0.35));
    float pore = smoothstep(0.56, 0.72, pn) * smoothstep(-0.2, 0.9, early*1.4 - 0.2);
    // broad color drift (heartwood / sapwood patches)
    float drift = vnoise(vec3(q.x/500.0, ac/110.0, 2.2));
    col *= 0.86 + 0.24*drift;
    col *= 1.0 + 0.16*(fib - 0.5) + 0.08*(fib2 - 0.5);
    col = mix(col, col*vec3(0.78,0.62,0.48), late*0.7);
    col *= 1.0 + 0.12*early;        // latewood is warmer and darker
    col = mix(col, col*vec3(0.62,0.42,0.30), pore*0.75);        // pores are dark orange-brown
    col = mix(col, min(col*1.22 + vec3(0.03,0.025,0.015), vec3(1.0)), early*smoothstep(0.62,0.9,fib)*0.45); // pale flecks
  }
  if(kind==6){ vec3 q = vP/cell; float w = step(0.0, sin(q.x*3.14159)*sin(q.y*3.14159)); col *= 0.7 + 0.5*w; }
  vec3 alb = pow(col, vec3(2.2));
  vec3 Nv = normalize(mat3(uV) * Nw);
  vec3 V = normalize(-vVp);
  vec3 kd = normalize(uKD); if(uLB.z > 0.5) kd = normalize(mat3(uV) * kd);
  vec3 fd = normalize(vec3(-0.6,0.1,0.5)); if(uLB.z > 0.5) fd = normalize(mat3(uV) * vec3(-0.6*0.8, -0.5, 0.4));
  float k1 = max(dot(Nv, kd), 0.0);
  float k2 = max(dot(Nv, fd), 0.0);
  float ndv = max(dot(Nv, V), 0.0);
  vec3 R = reflect(-V, Nv);
  vec3 Rw = R * mat3(uV);
  vec3 diff = alb * (1.0 - metal) * (0.20*uLA.y + 0.62*uLA.x*k1 + 0.28*uLA.z*k2);
  float fr = pow(1.0 - ndv, 5.0);
  vec3 F0 = mix(vec3(0.04), alb, metal);
  vec3 F = F0 + (vec3(1.0) - F0) * fr * (1.0 - 0.75*rough);
  vec3 spec = F * env(Rw, rough) * uLA.w;
  if(coat > 0.0){
    float Fc = 0.04 + 0.96*fr;
    spec += coat * Fc * env(Rw, 0.1) * 1.4 * uLA.w;
  }
  if(kind==3){ spec += alb*1.6 * pow(1.0 - ndv, 2.5) * 0.35 + vec3(0.05)*pow(1.0-ndv,2.0); }
  vec3 c = (diff + spec) * uLB.x;
  if(!gl_FrontFacing) c *= vec3(0.55,0.62,0.8);
  c = c / (1.0 + 0.18*c); // soft shoulder
  o = vec4(pow(clamp(c, 0.0, 1.0), vec3(1.0/2.2)), uCol.a);
}"#;

impl GlRes {
    pub fn new(gl: &glow::Context) -> Result<GlRes, String> {
        unsafe {
            let prog = gl.create_program()?;
            for (ty, src) in [(glow::VERTEX_SHADER, VS), (glow::FRAGMENT_SHADER, FS)] {
                let sh = gl.create_shader(ty)?;
                gl.shader_source(sh, src);
                gl.compile_shader(sh);
                if !gl.get_shader_compile_status(sh) {
                    return Err(gl.get_shader_info_log(sh));
                }
                gl.attach_shader(prog, sh);
            }
            gl.link_program(prog);
            if !gl.get_program_link_status(prog) {
                return Err(gl.get_program_info_log(prog));
            }
            Ok(GlRes {
                u_vp: gl.get_uniform_location(prog, "uVP"),
                u_v: gl.get_uniform_location(prog, "uV"),
                u_col: gl.get_uniform_location(prog, "uCol"),
                u_tint: gl.get_uniform_location(prog, "uTint"),
                u_ma: gl.get_uniform_location(prog, "uMatA"),
                u_mb: gl.get_uniform_location(prog, "uMatB"),
                u_clip: gl.get_uniform_location(prog, "uClip"),
                u_la: gl.get_uniform_location(prog, "uLA"),
                u_lb: gl.get_uniform_location(prog, "uLB"),
                u_kd: gl.get_uniform_location(prog, "uKD"),
                prog,
                skin: None,
                parts: None,
                preview: None,
                other: None,
                cover: None,
                side: None,
                result: None,
                plug: None,
                objs: None,
                seen: [u64::MAX; 9],
            })
        }
    }

    unsafe fn make_buf(gl: &glow::Context, v: &[Vtx], idx: Option<&[u32]>) -> Option<Buf> {
        if v.is_empty() {
            return None;
        }
        let vao = gl.create_vertex_array().ok()?;
        gl.bind_vertex_array(Some(vao));
        let vbo = gl.create_buffer().ok()?;
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
        let bytes = std::slice::from_raw_parts(v.as_ptr() as *const u8, std::mem::size_of_val(v));
        gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, bytes, glow::DYNAMIC_DRAW);
        let stride = std::mem::size_of::<Vtx>() as i32;
        for (loc, off) in [(0u32, 0i32), (1, 12), (2, 24)] {
            gl.enable_vertex_attrib_array(loc);
            gl.vertex_attrib_pointer_f32(loc, 3, glow::FLOAT, false, stride, off);
        }
        let mut ebo = None;
        let mut count = v.len() as i32;
        if let Some(ix) = idx {
            let b = gl.create_buffer().ok()?;
            gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(b));
            let ib = std::slice::from_raw_parts(ix.as_ptr() as *const u8, std::mem::size_of_val(ix));
            gl.buffer_data_u8_slice(glow::ELEMENT_ARRAY_BUFFER, ib, glow::DYNAMIC_DRAW);
            ebo = Some(b);
            count = ix.len() as i32;
        }
        gl.bind_vertex_array(None);
        Some(Buf { vao, vbo, ebo, count })
    }
    unsafe fn free(gl: &glow::Context, b: Option<Buf>) {
        if let Some(b) = b {
            gl.delete_vertex_array(b.vao);
            gl.delete_buffer(b.vbo);
            if let Some(e) = b.ebo {
                gl.delete_buffer(e);
            }
        }
    }

    unsafe fn set_mat(&self, gl: &glow::Context, m: &MatU, alpha: f32) {
        gl.uniform_4_f32(self.u_col.as_ref(), m.col[0], m.col[1], m.col[2], alpha);
        gl.uniform_4_f32(self.u_ma.as_ref(), m.a[0], m.a[1], m.a[2], m.a[3]);
        gl.uniform_4_f32(self.u_mb.as_ref(), m.b[0], m.b[1], m.b[2], m.b[3]);
    }

    pub fn draw(&mut self, gl: &glow::Context, sc: &SceneData) {
        unsafe {
            if self.seen[0] != sc.skin_ver {
                Self::free(gl, self.skin.take());
                self.skin = Self::make_buf(gl, &sc.skin, Some(&sc.skin_idx));
                self.seen[0] = sc.skin_ver;
            }
            if self.seen[1] != sc.parts_ver {
                Self::free(gl, self.parts.take());
                self.parts = Self::make_buf(gl, &sc.parts, None);
                self.seen[1] = sc.parts_ver;
            }
            if self.seen[2] != sc.preview_ver {
                Self::free(gl, self.preview.take());
                self.preview = Self::make_buf(gl, &sc.preview, None);
                self.seen[2] = sc.preview_ver;
            }
            if self.seen[3] != sc.other_ver {
                Self::free(gl, self.other.take());
                self.other = Self::make_buf(gl, &sc.other, Some(&sc.other_idx));
                self.seen[3] = sc.other_ver;
            }
            if self.seen[4] != sc.cover_ver {
                Self::free(gl, self.cover.take());
                self.cover = Self::make_buf(gl, &sc.cover, Some(&sc.cover_idx));
                self.seen[4] = sc.cover_ver;
            }
            if self.seen[8] != sc.side_ver {
                Self::free(gl, self.side.take());
                self.side = Self::make_buf(gl, &sc.side, Some(&sc.side_idx));
                self.seen[8] = sc.side_ver;
            }
            if self.seen[5] != sc.result_ver {
                Self::free(gl, self.result.take());
                self.result = Self::make_buf(gl, &sc.result, None);
                self.seen[5] = sc.result_ver;
            }
            if self.seen[6] != sc.plug_ver {
                Self::free(gl, self.plug.take());
                self.plug = Self::make_buf(gl, &sc.plug, None);
                self.seen[6] = sc.plug_ver;
            }
            if self.seen[7] != sc.objs_ver {
                Self::free(gl, self.objs.take());
                self.objs = Self::make_buf(gl, &sc.objs, None);
                self.seen[7] = sc.objs_ver;
            }
            gl.clear(glow::DEPTH_BUFFER_BIT);
            gl.enable(glow::DEPTH_TEST);
            gl.depth_func(glow::LEQUAL);
            gl.disable(glow::CULL_FACE);
            gl.use_program(Some(self.prog));
            gl.uniform_matrix_4_f32_slice(self.u_vp.as_ref(), false, &sc.view_proj);
            gl.uniform_matrix_4_f32_slice(self.u_v.as_ref(), false, &sc.view);
            let on = if sc.clip_on { 1.0 } else { 0.0 };
            gl.uniform_4_f32(self.u_la.as_ref(), sc.light_a[0], sc.light_a[1], sc.light_a[2], sc.light_a[3]);
            gl.uniform_4_f32(self.u_lb.as_ref(), sc.light_b[0], sc.light_b[1], sc.light_b[2], sc.light_b[3]);
            gl.uniform_3_f32(self.u_kd.as_ref(), sc.key_dir[0], sc.key_dir[1], sc.key_dir[2]);
            gl.uniform_4_f32(self.u_clip.as_ref(), on, sc.clip_x, 0.0, sc.clip_dir);
            let use_result = sc.show_result && self.result.is_some();
            if sc.show_skin && use_result {
                gl.enable(glow::POLYGON_OFFSET_FILL);
                gl.polygon_offset(1.0, 1.0);
                gl.uniform_4_f32(self.u_tint.as_ref(), 0.0, 0.0, 0.0, 0.0);
                gl.uniform_4_f32(self.u_clip.as_ref(), on, sc.clip_x, if sc.result_cap { 1.0 } else { 0.0 }, sc.clip_dir);
                self.set_mat(gl, &sc.skin_mat, 1.0);
                if let Some(b) = &self.result {
                    gl.bind_vertex_array(Some(b.vao));
                    gl.draw_arrays(glow::TRIANGLES, 0, b.count);
                }
                if let Some(b) = &self.plug {
                    gl.bind_vertex_array(Some(b.vao));
                    gl.draw_arrays(glow::TRIANGLES, 0, b.count);
                }
                gl.disable(glow::POLYGON_OFFSET_FILL);
            }
            if sc.show_skin && !use_result {
                if let Some(b) = &self.skin {
                    gl.bind_vertex_array(Some(b.vao));
                    gl.enable(glow::POLYGON_OFFSET_FILL);
                    gl.polygon_offset(1.0, 1.0);
                    let per = (sc.span_tris * 3) as i32;
                    self.set_mat(gl, &sc.skin_mat, 1.0);
                    let cap = if sc.wall_from > 0 { 1.0 } else { 0.0 };
                    gl.uniform_4_f32(self.u_clip.as_ref(), on, sc.clip_x, cap, sc.clip_dir);
                    for (s, c) in sc.span_colors.iter().enumerate() {
                        let k = sc.span_mix.get(s).copied().unwrap_or(0.0);
                        gl.uniform_4_f32(self.u_tint.as_ref(), c[0], c[1], c[2], k);
                        gl.draw_elements(glow::TRIANGLES, per, glow::UNSIGNED_INT, s as i32 * per * 4);
                    }
                    let spans_end = (sc.span_colors.len() as i32) * per;
                    if sc.wall_from == 0 && spans_end < b.count {
                        // side walls the editor view adds below the skin
                        gl.uniform_4_f32(self.u_tint.as_ref(), 0.0, 0.0, 0.0, 0.0);
                        gl.draw_elements(glow::TRIANGLES, b.count - spans_end, glow::UNSIGNED_INT, spans_end * 4);
                    }
                    if sc.wall_from > 0 && (sc.wall_from as i32) < b.count {
                        gl.uniform_4_f32(self.u_tint.as_ref(), 0.0, 0.0, 0.0, 0.0);
                        gl.draw_elements(glow::TRIANGLES, b.count - sc.wall_from as i32, glow::UNSIGNED_INT, sc.wall_from as i32 * 4);
                    }
                    gl.uniform_4_f32(self.u_tint.as_ref(), 0.0, 0.0, 0.0, 0.0);
                    gl.disable(glow::POLYGON_OFFSET_FILL);
                }
            }
            gl.uniform_4_f32(self.u_clip.as_ref(), on, sc.clip_x, 0.0, sc.clip_dir);
            gl.uniform_4_f32(self.u_tint.as_ref(), 0.0, 0.0, 0.0, 0.0);
            if sc.show_objs {
                if let Some(b) = &self.objs {
                    gl.bind_vertex_array(Some(b.vao));
                    gl.enable(glow::POLYGON_OFFSET_FILL);
                    gl.polygon_offset(-1.0, -1.0);
                    self.set_mat(gl, &MatU { col: [1.0, 1.0, 1.0], a: [0.45, 0.0, 0.1, 0.0], b: [1.0, 0.0, 0.0, 0.0] }, 1.0);
                    gl.draw_arrays(glow::TRIANGLES, 0, b.count);
                    gl.disable(glow::POLYGON_OFFSET_FILL);
                }
            }
            if sc.show_other {
                if let Some(b) = &self.other {
                    gl.bind_vertex_array(Some(b.vao));
                    gl.enable(glow::POLYGON_OFFSET_FILL);
                    gl.polygon_offset(1.0, 1.0);
                    self.set_mat(gl, &sc.other_mat, 1.0);
                    gl.draw_elements(glow::TRIANGLES, b.count, glow::UNSIGNED_INT, 0);
                    gl.disable(glow::POLYGON_OFFSET_FILL);
                }
            }
            if sc.show_parts {
                if let Some(b) = &self.parts {
                    gl.bind_vertex_array(Some(b.vao));
                    for (st, n, m) in &sc.part_ranges {
                        self.set_mat(gl, m, 1.0);
                        gl.draw_arrays(glow::TRIANGLES, *st as i32, *n as i32);
                    }
                }
            }
            self.set_mat(gl, &MatU::default(), 1.0);
            if let Some(b) = &self.preview {
                gl.bind_vertex_array(Some(b.vao));
                gl.draw_arrays(glow::TRIANGLES, 0, b.count);
            }
            if sc.show_cover {
                if let Some(b) = &self.cover {
                    gl.bind_vertex_array(Some(b.vao));
                    gl.enable(glow::POLYGON_OFFSET_FILL);
                    gl.polygon_offset(1.0, 1.0);
                    self.set_mat(gl, &sc.cover_mat, 1.0);
                    gl.draw_elements(glow::TRIANGLES, b.count, glow::UNSIGNED_INT, 0);
                    gl.disable(glow::POLYGON_OFFSET_FILL);
                }
            }
            if sc.show_side {
                if let Some(b) = &self.side {
                    gl.bind_vertex_array(Some(b.vao));
                    self.set_mat(gl, &sc.side_mat, 1.0);
                    gl.draw_elements(glow::TRIANGLES, b.count, glow::UNSIGNED_INT, 0);
                }
            }
            gl.bind_vertex_array(None);
            gl.use_program(None);
            gl.disable(glow::DEPTH_TEST);
        }
    }
}

pub type Shared = Arc<Mutex<SceneData>>;
pub type SharedGl = Arc<Mutex<Option<GlRes>>>;
