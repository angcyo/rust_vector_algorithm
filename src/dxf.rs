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

/// 解析 ASCII DXF 的 ENTITIES 段为 SVG path.
/// 支持 LINE, LWPOLYLINE(含 bulge), POLYLINE/VERTEX, CIRCLE, ARC, ELLIPSE, SPLINE.
pub fn dxf_to_svg_path(text: &str) -> String {
    let pairs = read_pairs(text);
    let mut ents: Vec<Ent> = Vec::new();
    let mut in_entities = false;
    let mut cur: Option<Ent> = None;
    let mut i = 0;
    while i < pairs.len() {
        let (code, val) = &pairs[i];
        if *code == 0 {
            if let Some(e) = cur.take() {
                ents.push(e);
            }
            match val.as_str() {
                "SECTION" => {
                    if let Some((2, n)) = pairs.get(i + 1) {
                        in_entities = n == "ENTITIES";
                        i += 1;
                    }
                }
                "ENDSEC" => in_entities = false,
                "EOF" => break,
                k if in_entities => cur = Some(Ent { kind: k.to_string(), ..Default::default() }),
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
        ents.push(e);
    }

    let mut pb = PathBuilder::new(true);
    let mut k = 0;
    while k < ents.len() {
        let e = &ents[k];
        match e.kind.as_str() {
            "LINE" => {
                pb.move_to(e.getd(10, 0.0), e.getd(20, 0.0));
                pb.line_to(e.getd(11, 0.0), e.getd(21, 0.0));
            }
            "LWPOLYLINE" => {
                let xs = e.all(10);
                let ys = e.all(20);
                // bulge 与顶点按顺序对应: 重新扫描保持顺序
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
                let _ = (xs, ys);
                poly(&mut pb, &pts, e.flags() & 1 == 1);
            }
            "POLYLINE" => {
                let closed = e.flags() & 1 == 1;
                let mut pts = Vec::new();
                k += 1;
                while k < ents.len() && ents[k].kind == "VERTEX" {
                    let v = &ents[k];
                    pts.push((v.getd(10, 0.0), v.getd(20, 0.0), v.getd(42, 0.0)));
                    k += 1;
                }
                poly(&mut pb, &pts, closed);
                // k 指向 SEQEND 或其他; 交给外层 k+=1 跳过
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
            "ELLIPSE" => ellipse(&mut pb, e),
            "SPLINE" => spline(&mut pb, e),
            _ => {}
        }
        k += 1;
    }
    pb.finish()
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
