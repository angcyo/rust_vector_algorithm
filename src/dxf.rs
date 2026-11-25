use crate::path::PathBuilder;

type Pairs = Vec<(i32, String)>;

fn read_pairs(text: &str) -> Pairs {
    let mut out = Vec::new();
    let mut lines = text.lines();
    while let (Some(c), Some(v)) = (lines.next(), lines.next()) {
        if let Ok(code) = c.trim().parse::<i32>() {
            out.push((code, v.trim().to_string()));
        }
    }
    out
}

fn f(v: &str) -> f64 {
    v.parse().unwrap_or(0.0)
}

#[derive(Default, Clone)]
struct Ent {
    kind: String,
    p: Vec<(i32, f64)>, // 数值组码
    sval: Vec<(i32, String)>,
}

impl Ent {
    fn get(&self, code: i32) -> Option<f64> {
        self.p.iter().find(|(c, _)| *c == code).map(|x| x.1)
    }
    fn getd(&self, code: i32, d: f64) -> f64 {
        self.get(code).unwrap_or(d)
    }
    fn all(&self, code: i32) -> Vec<f64> {
        self.p.iter().filter(|(c, _)| *c == code).map(|x| x.1).collect()
    }
    fn flags(&self) -> i32 {
        self.getd(70, 0.0) as i32
    }
}

/// 解析 DXF (ASCII 或二进制) 为 SVG path.
pub fn dxf_bytes_to_svg_path(data: &[u8]) -> String {
    if data.starts_with(b"AutoCAD Binary DXF") {
        build(read_pairs_binary(data))
    } else {
        build(read_pairs(&String::from_utf8_lossy(data)))
    }
}

/// 解析 ASCII DXF 为 SVG path.
/// 支持 ENTITIES 与 BLOCKS(INSERT 含缩放/旋转/阵列/嵌套), LINE, LWPOLYLINE, POLYLINE, CIRCLE, ARC,
/// ELLIPSE, SPLINE, SOLID/TRACE/3DFACE, 负挤出方向镜像.
pub fn dxf_to_svg_path(text: &str) -> String {
    build(read_pairs(text))
}

fn read_pairs_binary(d: &[u8]) -> Pairs {
    let mut out = Vec::new();
    let mut i = 22;
    let cstr = |i: &mut usize| {
        let s = *i;
        while *i < d.len() && d[*i] != 0 {
            *i += 1;
        }
        let r = String::from_utf8_lossy(&d[s..(*i).min(d.len())]).into_owned();
        *i += 1;
        r
    };
    while i + 2 <= d.len() {
        let code = u16::from_le_bytes([d[i], d[i + 1]]) as i32;
        i += 2;
        let need = |n: usize, i: usize| i + n <= d.len();
        let v = match code {
            0..=9 | 100..=102 | 105 | 300..=369 | 390..=399 | 410..=419 | 430..=439 | 470..=481 | 999 | 1000..=1009 => cstr(&mut i),
            10..=59 | 110..=149 | 210..=239 | 460..=469 | 1010..=1059 => {
                if !need(8, i) {
                    break;
                }
                let v = f64::from_le_bytes(d[i..i + 8].try_into().unwrap());
                i += 8;
                v.to_string()
            }
            90..=99 | 420..=429 | 440..=459 | 1071 => {
                if !need(4, i) {
                    break;
                }
                let v = i32::from_le_bytes(d[i..i + 4].try_into().unwrap());
                i += 4;
                v.to_string()
            }
            160..=169 => {
                if !need(8, i) {
                    break;
                }
                let v = i64::from_le_bytes(d[i..i + 8].try_into().unwrap());
                i += 8;
                v.to_string()
            }
            290..=299 => {
                if !need(1, i) {
                    break;
                }
                i += 1;
                d[i - 1].to_string()
            }
            _ => {
                if !need(2, i) {
                    break;
                }
                let v = i16::from_le_bytes([d[i], d[i + 1]]);
                i += 2;
                v.to_string()
            }
        };
        out.push((code, v));
    }
    out
}

struct Block {
    base: (f64, f64),
    ents: Vec<Ent>,
}

fn build(pairs: Pairs) -> String {
    let mut ents: Vec<Ent> = Vec::new();
    let mut blocks: std::collections::HashMap<String, Block> = Default::default();
    let mut sec = String::new();
    let mut cur_block: Option<String> = None;
    let mut cur: Option<Ent> = None;

    let mut commit = |e: Ent, sec: &str, cur_block: &mut Option<String>, ents: &mut Vec<Ent>| match sec {
        "ENTITIES" => ents.push(e),
        "BLOCKS" => match e.kind.as_str() {
            "BLOCK" => {
                let name = e.sval.iter().find(|(c, _)| *c == 2).map(|x| x.1.clone()).unwrap_or_default();
                blocks.insert(name.clone(), Block { base: (e.getd(10, 0.0), e.getd(20, 0.0)), ents: Vec::new() });
                *cur_block = Some(name);
            }
            "ENDBLK" => *cur_block = None,
            _ => {
                if let Some(b) = cur_block.as_ref().and_then(|n| blocks.get_mut(n)) {
                    b.ents.push(e);
                }
            }
        },
        _ => {}
    };

    let mut i = 0;
    while i < pairs.len() {
        let (code, val) = &pairs[i];
        if *code == 0 {
            if let Some(e) = cur.take() {
                commit(e, &sec, &mut cur_block, &mut ents);
            }
            match val.as_str() {
                "SECTION" => {
                    if let Some((2, n)) = pairs.get(i + 1) {
                        sec = n.clone();
                        i += 1;
                    }
                }
                "ENDSEC" => sec.clear(),
                "EOF" => break,
                k if sec == "ENTITIES" || sec == "BLOCKS" => {
                    cur = Some(Ent { kind: k.to_string(), ..Default::default() })
                }
                _ => {}
            }
        } else if let Some(e) = cur.as_mut() {
            if (1..=9).contains(code) || *code == 100 {
                e.sval.push((*code, val.clone()));
            } else {
                e.p.push((*code, f(val)));
            }
        }
        i += 1;
    }
    if let Some(e) = cur.take() {
        commit(e, &sec, &mut cur_block, &mut ents);
    }

    let mut pb = PathBuilder::new(true);
    pb.set_unit(insunits_to_mm(&pairs));
    draw(&mut pb, &ents, &blocks, 0);
    pb.finish()
}

/// 读取 $INSUNITS (回退 $MEASUREMENT) 得到 -> mm 的系数; 无单位按 mm.
fn insunits_to_mm(pairs: &Pairs) -> f64 {
    let hdr = |name: &str| {
        pairs.iter().position(|(c, v)| *c == 9 && v == name).and_then(|i| match pairs.get(i + 1) {
            Some((70, v)) => v.parse::<i32>().ok(),
            _ => None,
        })
    };
    match hdr("$INSUNITS") {
        Some(1) => 25.4,
        Some(2) => 304.8,
        Some(3) => 1_609_344.0,
        Some(4) | Some(0) | None => {
            if hdr("$INSUNITS").is_none() && hdr("$MEASUREMENT") == Some(0) {
                25.4
            } else {
                1.0
            }
        }
        Some(5) => 10.0,
        Some(6) => 1000.0,
        Some(7) => 1_000_000.0,
        Some(8) => 25.4e-6,
        Some(9) => 0.0254,
        Some(10) => 914.4,
        Some(11) => 1e-7,
        Some(12) => 1e-6,
        Some(13) => 0.001,
        Some(14) => 100.0,
        Some(15) => 10_000.0,
        Some(16) => 100_000.0,
        Some(17) => 1e12,
        Some(_) => 1.0,
    }
}

fn draw(pb: &mut PathBuilder, ents: &[Ent], blocks: &std::collections::HashMap<String, Block>, depth: u32) {
    let mut k = 0;
    while k < ents.len() {
        let e = &ents[k];
        let mirror = e.getd(230, 1.0) < 0.0 && e.kind != "INSERT";
        if mirror {
            pb.push_matrix();
            pb.concat([-1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
        }
        match e.kind.as_str() {
            "INSERT" if depth < 16 => {
                if let Some(b) = e.sval.iter().find(|(c, _)| *c == 2).and_then(|(_, n)| blocks.get(n)) {
                    let (sx, sy) = (e.getd(41, 1.0), e.getd(42, 1.0));
                    let rot = e.getd(50, 0.0).to_radians();
                    let (cols, rows) = (e.getd(70, 1.0).max(1.0) as usize, e.getd(71, 1.0).max(1.0) as usize);
                    let (cs, rs) = (e.getd(44, 0.0), e.getd(45, 0.0));
                    let (c, s) = (rot.cos(), rot.sin());
                    let (a, bb, cc, d) = (sx * c, sx * s, -sy * s, sy * c);
                    let mx = if e.getd(230, 1.0) < 0.0 { -1.0 } else { 1.0 };
                    for r in 0..rows {
                        for col in 0..cols {
                            let (ox, oy) = (col as f64 * cs, r as f64 * rs);
                            let px = e.getd(10, 0.0) + c * ox - s * oy;
                            let py = e.getd(20, 0.0) + s * ox + c * oy;
                            pb.push_matrix();
                            pb.concat([mx, 0.0, 0.0, 1.0, 0.0, 0.0]);
                            pb.concat([a, bb, cc, d, px - (a * b.base.0 + cc * b.base.1), py - (bb * b.base.0 + d * b.base.1)]);
                            draw(pb, &b.ents, blocks, depth + 1);
                            pb.pop_matrix();
                        }
                    }
                }
            }
            "SOLID" | "TRACE" | "3DFACE" => {
                let g = |c: i32| (e.getd(c, 0.0), e.getd(c + 10, 0.0));
                let pts = if e.kind == "3DFACE" { [g(10), g(11), g(12), g(13)] } else { [g(10), g(11), g(13), g(12)] };
                pb.move_to(pts[0].0, pts[0].1);
                for p in &pts[1..] {
                    pb.line_to(p.0, p.1);
                }
                pb.close();
            }
            "LINE" => {
                pb.move_to(e.getd(10, 0.0), e.getd(20, 0.0));
                pb.line_to(e.getd(11, 0.0), e.getd(21, 0.0));
            }
            "LWPOLYLINE" => {
                // bulge 与顶点按顺序对应
                let mut pts: Vec<(f64, f64, f64)> = Vec::new();
                for (c, v) in &e.p {
                    match c {
                        10 => pts.push((*v, 0.0, 0.0)),
                        20 => {
                            if let Some(l) = pts.last_mut() {
                                l.1 = *v
                            }
                        }
                        42 => {
                            if let Some(l) = pts.last_mut() {
                                l.2 = *v
                            }
                        }
                        _ => {}
                    }
                }
                poly(pb, &pts, e.flags() & 1 == 1);
            }
            "POLYLINE" => {
                let closed = e.flags() & 1 == 1;
                let mut pts = Vec::new();
                k += 1;
                while k < ents.len() && ents[k].kind == "VERTEX" {
                    let v = &ents[k];
                    // 跳过样条控制点/多面网格面记录以外的辅助顶点 (flag 16)
                    if (v.flags() & 16) == 0 {
                        pts.push((v.getd(10, 0.0), v.getd(20, 0.0), v.getd(42, 0.0)));
                    }
                    k += 1;
                }
                poly(pb, &pts, closed);
            }
            "CIRCLE" => {
                pb.arc_center(e.getd(10, 0.0), e.getd(20, 0.0), e.getd(40, 0.0), 0.0, 360.0, true, false);
            }
            "ARC" => {
                pb.arc_center(
                    e.getd(10, 0.0),
                    e.getd(20, 0.0),
                    e.getd(40, 0.0),
                    e.getd(50, 0.0),
                    e.getd(51, 360.0),
                    true,
                    false,
                );
            }
            "ELLIPSE" => ellipse(pb, e),
            "SPLINE" => spline(pb, e),
            _ => {}
        }
        if mirror {
            pb.pop_matrix();
        }
        k += 1;
    }
}


fn poly(pb: &mut PathBuilder, pts: &[(f64, f64, f64)], closed: bool) {
    if pts.is_empty() {
        return;
    }
    pb.move_to(pts[0].0, pts[0].1);
    let n = pts.len();
    let segs = if closed { n } else { n - 1 };
    for i in 0..segs {
        let (x0, y0, b) = pts[i];
        let (x1, y1, _) = pts[(i + 1) % n];
        if b.abs() < 1e-12 {
            pb.line_to(x1, y1);
        } else {
            let chord = ((x1 - x0).powi(2) + (y1 - y0).powi(2)).sqrt();
            let r = chord * (1.0 + b * b) / (4.0 * b.abs());
            pb.arc_to(r, r, 0.0, b.abs() > 1.0, b > 0.0, x1, y1);
        }
    }
    if closed {
        pb.close();
    }
}

fn ellipse(pb: &mut PathBuilder, e: &Ent) {
    let (cx, cy) = (e.getd(10, 0.0), e.getd(20, 0.0));
    let (mx, my) = (e.getd(11, 1.0), e.getd(21, 0.0));
    let ratio = e.getd(40, 1.0);
    let (t0, mut t1) = (e.getd(41, 0.0), e.getd(42, std::f64::consts::TAU));
    if t1 <= t0 {
        t1 += std::f64::consts::TAU;
    }
    let rx = (mx * mx + my * my).sqrt();
    let ry = rx * ratio;
    let rot = my.atan2(mx);
    let pt = |t: f64| {
        let (ex, ey) = (rx * t.cos(), ry * t.sin());
        (cx + ex * rot.cos() - ey * rot.sin(), cy + ex * rot.sin() + ey * rot.cos())
    };
    let (sx, sy) = pt(t0);
    pb.move_to(sx, sy);
    let span = t1 - t0;
    if span >= std::f64::consts::TAU - 1e-9 {
        let (mx2, my2) = pt(t0 + std::f64::consts::PI);
        pb.arc_to(rx, ry, rot.to_degrees(), false, true, mx2, my2);
        pb.arc_to(rx, ry, rot.to_degrees(), false, true, sx, sy);
    } else {
        let (ex, ey) = pt(t1);
        pb.arc_to(rx, ry, rot.to_degrees(), span > std::f64::consts::PI, true, ex, ey);
    }
}

fn spline(pb: &mut PathBuilder, e: &Ent) {
    let degree = e.getd(71, 3.0) as usize;
    let knots = e.all(40);
    let xs = e.all(10);
    let ys = e.all(20);
    let n = xs.len().min(ys.len());
    if n == 0 {
        return;
    }
    if knots.len() == n + degree + 1 && n > degree {
        let weights = e.all(41);
        let (t0, t1) = (knots[degree], knots[n]);
        let steps = (n * 16).max(32);
        for s in 0..=steps {
            let t = if s == steps { t1 } else { t0 + (t1 - t0) * s as f64 / steps as f64 };
            let (x, y) = de_boor(degree, &knots, &xs[..n], &ys[..n], &weights, t);
            if s == 0 {
                pb.move_to(x, y);
            } else {
                pb.line_to(x, y);
            }
        }
    } else {
        // 退化: 使用拟合点
        let fx = e.all(11);
        let fy = e.all(21);
        let m = fx.len().min(fy.len());
        for i in 0..m {
            if i == 0 {
                pb.move_to(fx[i], fy[i]);
            } else {
                pb.line_to(fx[i], fy[i]);
            }
        }
        if m == 0 {
            for i in 0..n {
                if i == 0 {
                    pb.move_to(xs[i], ys[i]);
                } else {
                    pb.line_to(xs[i], ys[i]);
                }
            }
        }
    }
}

// NURBS 求值 (齐次坐标 De Boor)
fn de_boor(p: usize, kn: &[f64], xs: &[f64], ys: &[f64], w: &[f64], t: f64) -> (f64, f64) {
    let n = xs.len();
    let mut k = p;
    while k + 1 < n && t >= kn[k + 1] {
        k += 1;
    }
    let mut d: Vec<(f64, f64, f64)> = (0..=p)
        .map(|j| {
            let i = j + k - p;
            let wi = w.get(i).copied().unwrap_or(1.0);
            (xs[i] * wi, ys[i] * wi, wi)
        })
        .collect();
    for r in 1..=p {
        for j in (r..=p).rev() {
            let i = j + k - p;
            let den = kn[i + p + 1 - r] - kn[i];
            let a = if den.abs() < 1e-14 { 0.0 } else { (t - kn[i]) / den };
            d[j] = (
                (1.0 - a) * d[j - 1].0 + a * d[j].0,
                (1.0 - a) * d[j - 1].1 + a * d[j].1,
                (1.0 - a) * d[j - 1].2 + a * d[j].2,
            );
        }
    }
    let r = d[p];
    (r.0 / r.2, r.1 / r.2)
}
