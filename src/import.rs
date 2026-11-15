// Flat drawings (DXF, Illustrator AI, EPS) -> outline shapes. Closed paths become free-loop shapes in mm.
use crate::shapes::{Kind, Shape};

type P = [f64; 2];

pub fn import_drawing(path: &str) -> Result<Vec<Shape>, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let stem = std::path::Path::new(path).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or("drawing".into());
    let ext = std::path::Path::new(path).extension().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default();
    let loops = match ext.as_str() {
        "dxf" => dxf_loops(&String::from_utf8_lossy(&bytes))?,
        "ai" | "eps" | "pdf" => ps_loops(&bytes)?,
        _ => return Err("Use a .dxf, .ai or .eps file".into()),
    };
    let shapes = loops_to_shapes(&stem, loops);
    if shapes.is_empty() {
        Err("No closed outlines found. Closed paths (filled shapes, closed polylines, circles) are imported; open lines are skipped.".into())
    } else {
        Ok(shapes)
    }
}

fn area(p: &[P]) -> f64 {
    let mut a = 0.0;
    for i in 0..p.len() {
        let (u, v) = (p[i], p[(i + 1) % p.len()]);
        a += u[0] * v[1] - v[0] * u[1];
    }
    a / 2.0
}

fn bbox(p: &[P]) -> (P, P) {
    let (mut lo, mut hi) = ([f64::MAX; 2], [f64::MIN; 2]);
    for q in p {
        for k in 0..2 {
            lo[k] = lo[k].min(q[k]);
            hi[k] = hi[k].max(q[k]);
        }
    }
    (lo, hi)
}

fn rdp(p: &[P], tol: f64) -> Vec<P> {
    if p.len() < 3 {
        return p.to_vec();
    }
    let (a, b) = (p[0], p[p.len() - 1]);
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let l = dx.hypot(dy);
    let (mut best, mut bi) = (0.0, 0);
    for (i, q) in p.iter().enumerate().skip(1).take(p.len() - 2) {
        let d = if l < 1e-12 { (q[0] - a[0]).hypot(q[1] - a[1]) } else { ((q[0] - a[0]) * dy - (q[1] - a[1]) * dx).abs() / l };
        if d > best {
            best = d;
            bi = i;
        }
    }
    if best <= tol {
        return vec![a, b];
    }
    let mut left = rdp(&p[..=bi], tol);
    let right = rdp(&p[bi..], tol);
    left.pop();
    left.extend(right);
    left
}

/// Tidy loops (mm) and turn each into a shape, biggest first. Frame rectangles that surround everything are dropped.
fn loops_to_shapes(name: &str, mut loops: Vec<Vec<P>>) -> Vec<Shape> {
    let mut clean: Vec<Vec<P>> = Vec::new();
    for mut l in loops.drain(..) {
        l.dedup_by(|a, b| (a[0] - b[0]).hypot(a[1] - b[1]) < 1e-6);
        if l.len() > 1 && (l[0][0] - l[l.len() - 1][0]).hypot(l[0][1] - l[l.len() - 1][1]) < 1e-6 {
            l.pop();
        }
        if l.len() < 3 {
            continue;
        }
        let mut ring = l.clone();
        ring.push(l[0]);
        let mut r = rdp(&ring, 0.04);
        r.pop();
        if r.len() < 3 {
            continue;
        }
        let (lo, hi) = bbox(&r);
        if hi[0] - lo[0] < 0.5 || hi[1] - lo[1] < 0.5 || area(&r).abs() < 0.5 {
            continue;
        }
        if area(&r) < 0.0 {
            r.reverse();
        }
        clean.push(r);
    }
    if clean.len() > 1 {
        let all: Vec<P> = clean.iter().flatten().copied().collect();
        let (alo, ahi) = bbox(&all);
        clean.retain(|l| {
            let (lo, hi) = bbox(l);
            let cover = ((hi[0] - lo[0]) * (hi[1] - lo[1])) / ((ahi[0] - alo[0]) * (ahi[1] - alo[1])).max(1e-9);
            !(l.len() == 4 && cover > 0.98)
        });
    }
    clean.sort_by(|a, b| area(b).partial_cmp(&area(a)).unwrap());
    clean.truncate(24);
    let n = clean.len();
    clean
        .into_iter()
        .enumerate()
        .map(|(i, l)| {
            let (lo, hi) = bbox(&l);
            let (cx, cy) = ((lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0);
            let mut s = Shape::new(&if n > 1 { format!("{} {}", name, i + 1) } else { name.to_string() }, Kind::Free, hi[0] - lo[0], hi[1] - lo[1]);
            s.pts = l.iter().map(|p| [p[0] - cx, p[1] - cy]).collect();
            s.fix_rad();
            s
        })
        .collect()
}

// ---------------------------------------------------------------- PostScript / PDF paths
struct Ctm([f64; 6]);
impl Ctm {
    fn ap(&self, x: f64, y: f64) -> P {
        let m = &self.0;
        [m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5]]
    }
    fn mul(&self, n: &[f64]) -> Ctm {
        let m = &self.0;
        Ctm([n[0] * m[0] + n[1] * m[2], n[0] * m[1] + n[1] * m[3], n[2] * m[0] + n[3] * m[2], n[2] * m[1] + n[3] * m[3], n[4] * m[0] + n[5] * m[2] + m[4], n[4] * m[1] + n[5] * m[3] + m[5]])
    }
}

fn flatten(a: P, b: P, c: P, d: P, out: &mut Vec<P>) {
    for i in 1..=12 {
        let t = i as f64 / 12.0;
        let u = 1.0 - t;
        out.push([u * u * u * a[0] + 3.0 * u * u * t * b[0] + 3.0 * u * t * t * c[0] + t * t * t * d[0], u * u * u * a[1] + 3.0 * u * u * t * b[1] + 3.0 * u * t * t * c[1] + t * t * t * d[1]]);
    }
}

fn inflate_streams(b: &[u8]) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    let find = |h: &[u8], n: &[u8], from: usize| -> Option<usize> { h[from..].windows(n.len()).position(|w| w == n).map(|p| p + from) };
    let mut pos = 0;
    while let Some(s) = find(b, b"stream", pos) {
        if s >= 3 && &b[s - 3..s] == b"end" {
            pos = s + 6;
            continue;
        }
        let mut st = s + 6;
        if b.get(st) == Some(&b'\r') {
            st += 1;
        }
        if b.get(st) == Some(&b'\n') {
            st += 1;
        }
        let Some(e) = find(b, b"endstream", st) else { break };
        let dict = &b[s.saturating_sub(400)..s];
        let dict = if let Some(p) = dict.windows(2).rposition(|w| w == b"<<") { &dict[p..] } else { dict };
        let raw = &b[st..e];
        if dict.windows(11).any(|w| w == b"FlateDecode") {
            if let Ok(v) = miniz_oxide::inflate::decompress_to_vec_zlib(raw) {
                out.push(v);
            }
        } else {
            out.push(raw.to_vec());
        }
        pos = e + 9;
    }
    out
}

fn ps_loops(bytes: &[u8]) -> Result<Vec<Vec<P>>, String> {
    let mut b = bytes;
    // DOS EPS binary header
    if b.len() > 30 && b[..4] == [0xC5, 0xD0, 0xD3, 0xC6] {
        let off = u32::from_le_bytes([b[4], b[5], b[6], b[7]]) as usize;
        let len = u32::from_le_bytes([b[8], b[9], b[10], b[11]]) as usize;
        if off + len <= b.len() {
            b = &b[off..off + len];
        }
    }
    let mut texts: Vec<String> = Vec::new();
    if b.starts_with(b"%PDF") || bytes.windows(5).any(|w| w == b"%PDF-") && !b.starts_with(b"%!PS") {
        let streams = inflate_streams(b);
        let mut page: Vec<String> = Vec::new();
        for s in &streams {
            let t = String::from_utf8_lossy(s).to_string();
            if t.contains("%AI") || t.contains("%%") || t.contains("%!PS") {
                continue;
            }
            if t.contains(" m") || t.contains(" re") {
                page.push(t);
            }
        }
        if page.is_empty() {
            for s in &streams {
                texts.push(String::from_utf8_lossy(s).to_string());
            }
        } else {
            texts = page;
        }
    } else {
        texts.push(String::from_utf8_lossy(b).to_string());
        // some AI files carry the artwork as PostScript inside compressed streams
        if !texts[0].contains(" moveto") && !texts[0].contains(" m\n") && !texts[0].contains(" m ") {
            for s in inflate_streams(b) {
                texts.push(String::from_utf8_lossy(&s).to_string());
            }
        }
    }
    let mut loops: Vec<Vec<P>> = Vec::new();
    for t in &texts {
        parse_paths(t, &mut loops);
    }
    const MM: f64 = 25.4 / 72.0;
    for l in loops.iter_mut() {
        for p in l.iter_mut() {
            *p = [p[0] * MM, p[1] * MM];
        }
    }
    Ok(loops)
}

fn parse_paths(text: &str, loops: &mut Vec<Vec<P>>) {
    let mut ctm = Ctm([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
    let mut stack: Vec<Ctm> = Vec::new();
    let mut nums: Vec<f64> = Vec::new();
    // current path: (points, closed)
    let mut subs: Vec<(Vec<P>, bool)> = Vec::new();
    let mut cur: P = [0.0, 0.0];
    let mut chars = text.chars().peekable();
    let mut tok = String::new();
    let mut tokens: Vec<String> = Vec::new();
    // tokenise: whitespace separated, skip comments and (strings)
    while let Some(c) = chars.next() {
        match c {
            '%' => {
                while let Some(&n) = chars.peek() {
                    if n == '\n' || n == '\r' {
                        break;
                    }
                    chars.next();
                }
            }
            '(' => {
                let mut depth = 1;
                while let Some(n) = chars.next() {
                    if n == '\\' {
                        chars.next();
                    } else if n == '(' {
                        depth += 1;
                    } else if n == ')' {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                }
                tokens.push("()".into());
            }
            c if c.is_whitespace() || c == '[' || c == ']' || c == '{' || c == '}' => {
                if !tok.is_empty() {
                    tokens.push(std::mem::take(&mut tok));
                }
                if c == '[' || c == ']' {
                    tokens.push(c.to_string());
                }
            }
            c => tok.push(c),
        }
    }
    if !tok.is_empty() {
        tokens.push(tok);
    }
    for t in tokens {
        if let Ok(v) = t.parse::<f64>() {
            nums.push(v);
            continue;
        }
        let n = nums.len();
        let g = |i: usize| nums[n - i];
        match t.as_str() {
            "m" | "M" | "moveto" if n >= 2 => {
                cur = ctm.ap(g(2), g(1));
                subs.push((vec![cur], false));
            }
            "l" | "L" | "lineto" if n >= 2 => {
                cur = ctm.ap(g(2), g(1));
                if let Some(s) = subs.last_mut() {
                    s.0.push(cur);
                } else {
                    subs.push((vec![cur], false));
                }
            }
            "c" | "C" | "curveto" if n >= 6 => {
                let (p1, p2, p3) = (ctm.ap(g(6), g(5)), ctm.ap(g(4), g(3)), ctm.ap(g(2), g(1)));
                if let Some(s) = subs.last_mut() {
                    flatten(cur, p1, p2, p3, &mut s.0);
                }
                cur = p3;
            }
            "v" | "V" if n >= 4 => {
                let (p2, p3) = (ctm.ap(g(4), g(3)), ctm.ap(g(2), g(1)));
                if let Some(s) = subs.last_mut() {
                    flatten(cur, cur, p2, p3, &mut s.0);
                }
                cur = p3;
            }
            "y" | "Y" if n >= 4 => {
                let (p1, p3) = (ctm.ap(g(4), g(3)), ctm.ap(g(2), g(1)));
                if let Some(s) = subs.last_mut() {
                    flatten(cur, p1, p3, p3, &mut s.0);
                }
                cur = p3;
            }
            "re" if n >= 4 => {
                let (x, y, w, h) = (g(4), g(3), g(2), g(1));
                let pts = vec![ctm.ap(x, y), ctm.ap(x + w, y), ctm.ap(x + w, y + h), ctm.ap(x, y + h)];
                cur = pts[0];
                subs.push((pts, true));
            }
            "h" | "H" | "closepath" | "cp" => {
                if let Some(s) = subs.last_mut() {
                    s.1 = true;
                }
            }
            "q" | "gsave" => stack.push(Ctm(ctm.0)),
            "Q" | "grestore" => {
                if let Some(c) = stack.pop() {
                    ctm = c;
                }
            }
            "cm" if n >= 6 => ctm = ctm.mul(&nums[n - 6..]),
            "f" | "F" | "f*" | "fill" | "eofill" | "B" | "B*" | "b" | "b*" | "s" | "S" | "stroke" => {
                let fills = !matches!(t.as_str(), "s" | "S" | "stroke");
                let closes = matches!(t.as_str(), "b" | "b*" | "s");
                for (p, closed) in subs.drain(..) {
                    if p.len() >= 3 && (fills || closed || closes) {
                        loops.push(p);
                    }
                }
            }
            "n" | "N" | "newpath" | "W" | "W*" | "clip" => {
                if t != "newpath" || true {
                    subs.clear();
                }
            }
            _ => {}
        }
        nums.clear();
    }
}

// ---------------------------------------------------------------- DXF
fn arc_pts(c: P, r: f64, a0: f64, a1: f64) -> Vec<P> {
    let mut a1 = a1;
    while a1 <= a0 {
        a1 += 360.0;
    }
    let n = (((a1 - a0) / 6.0).ceil() as usize).max(2);
    (0..=n).map(|i| {
        let a = (a0 + (a1 - a0) * i as f64 / n as f64).to_radians();
        [c[0] + r * a.cos(), c[1] + r * a.sin()]
    }).collect()
}

fn bulge_pts(a: P, b: P, bulge: f64, out: &mut Vec<P>) {
    if bulge.abs() < 1e-9 {
        out.push(b);
        return;
    }
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let d = dx.hypot(dy);
    if d < 1e-12 {
        return;
    }
    let off = d * (1.0 - bulge * bulge) / (4.0 * bulge);
    let c = [(a[0] + b[0]) / 2.0 - dy / d * off, (a[1] + b[1]) / 2.0 + dx / d * off];
    let r = (a[0] - c[0]).hypot(a[1] - c[1]);
    let a0 = (a[1] - c[1]).atan2(a[0] - c[0]);
    let sweep = 4.0 * bulge.atan();
    let n = ((sweep.abs().to_degrees() / 6.0).ceil() as usize).max(2);
    for i in 1..=n {
        let t = a0 + sweep * i as f64 / n as f64;
        out.push([c[0] + r * t.cos(), c[1] + r * t.sin()]);
    }
}

fn spline_pts(deg: usize, knots: &[f64], ctrl: &[P]) -> Vec<P> {
    let n = ctrl.len();
    if n < 2 || knots.len() != n + deg + 1 {
        return ctrl.to_vec();
    }
    let (t0, t1) = (knots[deg], knots[n]);
    let samples = n * 10;
    let mut out = Vec::new();
    for s in 0..=samples {
        let t = (t0 + (t1 - t0) * s as f64 / samples as f64).min(t1 - 1e-9).max(t0);
        let mut k = deg;
        while k + 1 < n && knots[k + 1] <= t {
            k += 1;
        }
        let mut d: Vec<P> = (0..=deg).map(|j| ctrl[(j + k).saturating_sub(deg).min(n - 1)]).collect();
        for r in 1..=deg {
            for j in (r..=deg).rev() {
                let i = j + k - deg;
                let den = knots[i + deg + 1 - r] - knots[i];
                let a = if den.abs() < 1e-12 { 0.0 } else { (t - knots[i]) / den };
                d[j] = [(1.0 - a) * d[j - 1][0] + a * d[j][0], (1.0 - a) * d[j - 1][1] + a * d[j][1]];
            }
        }
        out.push(d[deg]);
    }
    out
}

fn dxf_loops(txt: &str) -> Result<Vec<Vec<P>>, String> {
    let lines: Vec<&str> = txt.lines().collect();
    let mut pairs: Vec<(i32, String)> = Vec::new();
    let mut i = 0;
    while i + 1 < lines.len() {
        if let Ok(c) = lines[i].trim().parse::<i32>() {
            pairs.push((c, lines[i + 1].trim().to_string()));
        }
        i += 2;
    }
    if pairs.is_empty() {
        return Err("Could not read this DXF (is it a binary DXF? save it as ASCII)".into());
    }
    // units
    let mut scale = 1.0;
    for (k, (c, v)) in pairs.iter().enumerate() {
        if *c == 9 && v == "$INSUNITS" {
            if let Some((_, u)) = pairs.get(k + 1) {
                scale = match u.as_str() {
                    "1" => 25.4,
                    "2" => 304.8,
                    "5" => 10.0,
                    "6" => 1000.0,
                    "3" => 1609344.0,
                    _ => 1.0,
                };
            }
        }
    }
    // entities
    let mut ents: Vec<(String, Vec<(i32, String)>)> = Vec::new();
    let mut in_ent = false;
    for (c, v) in &pairs {
        if *c == 0 && v == "SECTION" {
            in_ent = false;
        }
        if *c == 2 && v == "ENTITIES" {
            in_ent = true;
            continue;
        }
        if *c == 0 && v == "ENDSEC" {
            in_ent = false;
        }
        if !in_ent {
            continue;
        }
        if *c == 0 {
            ents.push((v.clone(), Vec::new()));
        } else if let Some(e) = ents.last_mut() {
            e.1.push((*c, v.clone()));
        }
    }
    let f = |e: &Vec<(i32, String)>, c: i32| e.iter().find(|(k, _)| *k == c).and_then(|(_, v)| v.parse::<f64>().ok());
    let all = |e: &Vec<(i32, String)>, c: i32| e.iter().filter(|(k, _)| *k == c).filter_map(|(_, v)| v.parse::<f64>().ok()).collect::<Vec<f64>>();
    let mut closed: Vec<Vec<P>> = Vec::new();
    let mut open: Vec<Vec<P>> = Vec::new();
    let mut k = 0;
    while k < ents.len() {
        let (name, e) = (&ents[k].0, &ents[k].1);
        match name.as_str() {
            "LINE" => {
                if let (Some(x0), Some(y0), Some(x1), Some(y1)) = (f(e, 10), f(e, 20), f(e, 11), f(e, 21)) {
                    open.push(vec![[x0, y0], [x1, y1]]);
                }
            }
            "ARC" => {
                if let (Some(x), Some(y), Some(r), Some(a0), Some(a1)) = (f(e, 10), f(e, 20), f(e, 40), f(e, 50), f(e, 51)) {
                    open.push(arc_pts([x, y], r, a0, a1));
                }
            }
            "CIRCLE" => {
                if let (Some(x), Some(y), Some(r)) = (f(e, 10), f(e, 20), f(e, 40)) {
                    let mut p = arc_pts([x, y], r, 0.0, 360.0);
                    p.pop();
                    closed.push(p);
                }
            }
            "ELLIPSE" => {
                if let (Some(cx), Some(cy), Some(mx), Some(my), Some(ratio)) = (f(e, 10), f(e, 20), f(e, 11), f(e, 21), f(e, 40)) {
                    let (t0, t1) = (f(e, 41).unwrap_or(0.0), f(e, 42).unwrap_or(std::f64::consts::TAU));
                    let mut t1 = t1;
                    while t1 <= t0 {
                        t1 += std::f64::consts::TAU;
                    }
                    let n = 72;
                    let pts: Vec<P> = (0..=n).map(|i| {
                        let t = t0 + (t1 - t0) * i as f64 / n as f64;
                        [cx + mx * t.cos() - my * ratio * t.sin(), cy + my * t.cos() + mx * ratio * t.sin()]
                    }).collect();
                    if (t1 - t0 - std::f64::consts::TAU).abs() < 1e-6 {
                        let mut p = pts;
                        p.pop();
                        closed.push(p);
                    } else {
                        open.push(pts);
                    }
                }
            }
            "LWPOLYLINE" => {
                let flag = f(e, 70).unwrap_or(0.0) as i32;
                // vertices in order, each with an optional bulge following
                let mut v: Vec<(P, f64)> = Vec::new();
                let mut pending_x: Option<f64> = None;
                for (c, val) in e {
                    let Ok(x) = val.parse::<f64>() else { continue };
                    match c {
                        10 => pending_x = Some(x),
                        20 => {
                            if let Some(px) = pending_x.take() {
                                v.push(([px, x], 0.0));
                            }
                        }
                        42 => {
                            if let Some(l) = v.last_mut() {
                                l.1 = x;
                            }
                        }
                        _ => {}
                    }
                }
                polyline(&v, flag & 1 == 1, &mut closed, &mut open);
            }
            "POLYLINE" => {
                let flag = f(e, 70).unwrap_or(0.0) as i32;
                let mut v: Vec<(P, f64)> = Vec::new();
                let mut j = k + 1;
                while j < ents.len() && ents[j].0 == "VERTEX" {
                    let ve = &ents[j].1;
                    if let (Some(x), Some(y)) = (f(ve, 10), f(ve, 20)) {
                        v.push(([x, y], f(ve, 42).unwrap_or(0.0)));
                    }
                    j += 1;
                }
                polyline(&v, flag & 1 == 1, &mut closed, &mut open);
                k = j - 1;
            }
            "SPLINE" => {
                let flag = f(e, 70).unwrap_or(0.0) as i32;
                let deg = f(e, 71).unwrap_or(3.0) as usize;
                let (xs, ys) = (all(e, 10), all(e, 20));
                let ctrl: Vec<P> = xs.iter().zip(ys.iter()).map(|(x, y)| [*x, *y]).collect();
                let (fx, fy) = (all(e, 11), all(e, 21));
                let pts = if !fx.is_empty() && ctrl.is_empty() {
                    fx.iter().zip(fy.iter()).map(|(x, y)| [*x, *y]).collect()
                } else {
                    spline_pts(deg, &all(e, 40), &ctrl)
                };
                if flag & 1 == 1 {
                    closed.push(pts);
                } else {
                    open.push(pts);
                }
            }
            _ => {}
        }
        k += 1;
    }
    // join open pieces end to end
    let all_pts: Vec<P> = open.iter().flatten().copied().collect();
    if !all_pts.is_empty() {
        let (lo, hi) = bbox(&all_pts);
        let tol = (hi[0] - lo[0]).hypot(hi[1] - lo[1]) * 1e-4 + 1e-6;
        let near = |a: P, b: P| (a[0] - b[0]).hypot(a[1] - b[1]) <= tol;
        while let Some(mut cur) = open.pop() {
            loop {
                if cur.len() > 2 && near(cur[0], cur[cur.len() - 1]) {
                    closed.push(cur.clone());
                    break;
                }
                let end = cur[cur.len() - 1];
                let hit = open.iter().position(|p| near(p[0], end) || near(p[p.len() - 1], end));
                match hit {
                    Some(h) => {
                        let mut p = open.remove(h);
                        if !near(p[0], end) {
                            p.reverse();
                        }
                        cur.extend(p.into_iter().skip(1));
                    }
                    None => break,
                }
            }
        }
    }
    for l in closed.iter_mut() {
        for p in l.iter_mut() {
            *p = [p[0] * scale, p[1] * scale];
        }
    }
    Ok(closed)
}

fn polyline(v: &[(P, f64)], is_closed: bool, closed: &mut Vec<Vec<P>>, open: &mut Vec<Vec<P>>) {
    if v.len() < 2 {
        return;
    }
    let mut pts = vec![v[0].0];
    for i in 0..v.len() - 1 {
        bulge_pts(v[i].0, v[i + 1].0, v[i].1, &mut pts);
    }
    if is_closed {
        bulge_pts(v[v.len() - 1].0, v[0].0, v[v.len() - 1].1, &mut pts);
        pts.pop();
        closed.push(pts);
    } else {
        open.push(pts);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dxf_circle_and_lines() {
        let dxf = "0\nSECTION\n2\nENTITIES\n0\nCIRCLE\n10\n0\n20\n0\n40\n10\n0\nLINE\n10\n20\n20\n0\n11\n40\n21\n0\n0\nLINE\n10\n40\n20\n0\n11\n40\n21\n20\n0\nLINE\n10\n40\n20\n20\n11\n20\n21\n20\n0\nLINE\n10\n20\n20\n20\n11\n20\n21\n0\n0\nENDSEC\n0\nEOF\n";
        let l = dxf_loops(dxf).unwrap();
        assert_eq!(l.len(), 2);
        let s = loops_to_shapes("t", l);
        assert_eq!(s.len(), 2);
    }

    #[test]
    fn eps_paths() {
        let eps = "%!PS-Adobe-3.0 EPSF-3.0\n0 0 m\n100 0 l\n100 50 l\n0 50 l\nh\nf\n200 200 m 250 200 250 250 200 250 c h f\n";
        let l = ps_loops(eps.as_bytes()).unwrap();
        assert_eq!(l.len(), 2);
        let s = loops_to_shapes("e", l);
        assert!(!s.is_empty());
        assert!((s[0].w - 100.0 * 25.4 / 72.0).abs() < 0.01);
    }

    #[test]
    fn ai_flate_stream() {
        let content = b"q 1 0 0 1 5 5 cm 10 10 m 90 10 l 90 60 l 10 60 l h f Q";
        let z = miniz_oxide::deflate::compress_to_vec_zlib(content, 6);
        let mut pdf = b"%PDF-1.5\n4 0 obj\n<< /Filter /FlateDecode /Length 10 >>\nstream\n".to_vec();
        pdf.extend(z);
        pdf.extend(b"\nendstream\nendobj\n");
        let l = ps_loops(&pdf).unwrap();
        assert_eq!(l.len(), 1);
        assert!((l[0][0][0] - 15.0 * 25.4 / 72.0).abs() < 1e-6);
    }

    #[test]
    fn pdf_content_stream() {
        let pdf = b"%PDF-1.4\n1 0 obj\n<< /Length 50 >>\nstream\n10 10 m 90 10 l 90 60 l 10 60 l h f\nendstream\nendobj\n";
        let l = ps_loops(pdf).unwrap();
        assert_eq!(l.len(), 1);
    }
}
