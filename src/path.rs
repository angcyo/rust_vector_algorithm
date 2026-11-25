use std::fmt::Write;

/// SVG path 构建器. `flip_y` 为 true 时 Y 取反 (CAD/HPGL 的 Y 轴向上, SVG 向下).
#[derive(Debug, Clone)]
pub struct PathBuilder {
    d: String,
    flip_y: bool,
    pub cur: (f64, f64),
    start: (f64, f64),
    open: bool,
    m: [f64; 6],
    stack: Vec<[f64; 6]>,
    unit: f64,
}

const IDENT: [f64; 6] = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

fn num(v: f64) -> String {
    let s = format!("{:.4}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" || s.is_empty() { "0".into() } else { s.into() }
}

impl PathBuilder {
    pub fn new(flip_y: bool) -> Self {
        Self {
            d: String::new(),
            flip_y,
            cur: (0.0, 0.0),
            start: (0.0, 0.0),
            open: false,
            m: IDENT,
            stack: Vec::new(),
            unit: 1.0,
        }
    }

    /// 输出单位换算系数 (源单位 -> mm), 在最终输出时统一乘上.
    pub fn set_unit(&mut self, unit: f64) {
        self.unit = unit;
    }

    fn tx(&self, x: f64, y: f64) -> (f64, f64) {
        let m = &self.m;
        (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5])
    }

    fn p(&self, x: f64, y: f64) -> String {
        let (x, y) = self.tx(x, y);
        let (x, y) = (x * self.unit, y * self.unit);
        format!("{} {}", num(x), num(if self.flip_y { -y } else { y }))
    }

    /// 叠加变换: 新矩阵先作用于点, 再应用当前矩阵.
    pub fn concat(&mut self, n: [f64; 6]) {
        let o = self.m;
        self.m = [
            o[0] * n[0] + o[2] * n[1],
            o[1] * n[0] + o[3] * n[1],
            o[0] * n[2] + o[2] * n[3],
            o[1] * n[2] + o[3] * n[3],
            o[0] * n[4] + o[2] * n[5] + o[4],
            o[1] * n[4] + o[3] * n[5] + o[5],
        ];
    }

    pub fn push_matrix(&mut self) {
        self.stack.push(self.m);
    }

    pub fn pop_matrix(&mut self) {
        if let Some(m) = self.stack.pop() {
            self.m = m;
        }
    }

    fn is_identity(&self) -> bool {
        self.m == IDENT
    }

    /// 端点式圆弧转三次贝塞尔 (用于非单位变换).
    fn arc_as_cubics(&mut self, rx: f64, ry: f64, rot_deg: f64, large: bool, ccw: bool, x: f64, y: f64) {
        let (x0, y0) = self.cur;
        if rx.abs() < 1e-12 || ry.abs() < 1e-12 || ((x - x0).abs() < 1e-12 && (y - y0).abs() < 1e-12) {
            self.line_to(x, y);
            return;
        }
        let phi = rot_deg.to_radians();
        let (cp, sp) = (phi.cos(), phi.sin());
        let (dx, dy) = ((x0 - x) / 2.0, (y0 - y) / 2.0);
        let (x1, y1) = (cp * dx + sp * dy, -sp * dx + cp * dy);
        let (mut rx, mut ry) = (rx.abs(), ry.abs());
        let lam = x1 * x1 / (rx * rx) + y1 * y1 / (ry * ry);
        if lam > 1.0 {
            let s = lam.sqrt();
            rx *= s;
            ry *= s;
        }
        let num_ = rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1;
        let den = rx * rx * y1 * y1 + ry * ry * x1 * x1;
        let mut co = if den == 0.0 { 0.0 } else { (num_ / den).max(0.0).sqrt() };
        if large == ccw {
            co = -co;
        }
        let (cxp, cyp) = (co * rx * y1 / ry, -co * ry * x1 / rx);
        let (cx, cy) = (cp * cxp - sp * cyp + (x0 + x) / 2.0, sp * cxp + cp * cyp + (y0 + y) / 2.0);
        let ang = |ux: f64, uy: f64| uy.atan2(ux);
        let t0 = ang((x1 - cxp) / rx, (y1 - cyp) / ry);
        let t1 = ang((-x1 - cxp) / rx, (-y1 - cyp) / ry);
        let mut dt = t1 - t0;
        if ccw && dt < 0.0 {
            dt += std::f64::consts::TAU;
        } else if !ccw && dt > 0.0 {
            dt -= std::f64::consts::TAU;
        }
        let n = (dt.abs() / std::f64::consts::FRAC_PI_2).ceil().max(1.0) as usize;
        let step = dt / n as f64;
        let k = 4.0 / 3.0 * (step / 4.0).tan();
        let pt = |t: f64| (cx + rx * t.cos() * cp - ry * t.sin() * sp, cy + rx * t.cos() * sp + ry * t.sin() * cp);
        let dpt = |t: f64| (-rx * t.sin() * cp - ry * t.cos() * sp, -rx * t.sin() * sp + ry * t.cos() * cp);
        for i in 0..n {
            let (a, b) = (t0 + step * i as f64, t0 + step * (i + 1) as f64);
            let (p0, p3) = (pt(a), if i + 1 == n { (x, y) } else { pt(b) });
            let (d0, d3) = (dpt(a), dpt(b));
            self.cubic_to(p0.0 + k * d0.0, p0.1 + k * d0.1, p3.0 - k * d3.0, p3.1 - k * d3.1, p3.0, p3.1);
        }
    }

    pub fn move_to(&mut self, x: f64, y: f64) {
        if !self.d.is_empty() {
            self.d.push(' ');
        }
        let _ = write!(self.d, "M{}", self.p(x, y));
        self.cur = (x, y);
        self.start = (x, y);
        self.open = true;
    }

    pub fn line_to(&mut self, x: f64, y: f64) {
        if !self.open {
            self.move_to(x, y);
            return;
        }
        let _ = write!(self.d, " L{}", self.p(x, y));
        self.cur = (x, y);
    }

    pub fn cubic_to(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, x: f64, y: f64) {
        if !self.open {
            self.move_to(x1, y1);
        }
        let _ = write!(self.d, " C{} {} {}", self.p(x1, y1), self.p(x2, y2), self.p(x, y));
        self.cur = (x, y);
    }

    pub fn quad_to(&mut self, x1: f64, y1: f64, x: f64, y: f64) {
        if !self.open {
            self.move_to(x1, y1);
        }
        let _ = write!(self.d, " Q{} {}", self.p(x1, y1), self.p(x, y));
        self.cur = (x, y);
    }

    /// 椭圆弧(端点参数). `sweep_ccw`: 在未翻转坐标系下是否逆时针.
    pub fn arc_to(&mut self, rx: f64, ry: f64, rot_deg: f64, large: bool, sweep_ccw: bool, x: f64, y: f64) {
        if !self.open {
            self.move_to(x, y);
            return;
        }
        if !self.is_identity() {
            self.arc_as_cubics(rx, ry, rot_deg, large, sweep_ccw, x, y);
            return;
        }
        // 翻转 Y 后方向相反
        let sweep = if self.flip_y { !sweep_ccw } else { sweep_ccw };
        let rot = if self.flip_y { -rot_deg } else { rot_deg };
        let _ = write!(
            self.d,
            " A{} {} {} {} {} {}",
            num(rx * self.unit),
            num(ry * self.unit),
            num(rot),
            large as u8,
            sweep as u8,
            self.p(x, y)
        );
        self.cur = (x, y);
    }

    /// 圆心/半径/起止角(度)的圆弧, 逆时针从 start 到 end.
    pub fn arc_center(&mut self, cx: f64, cy: f64, r: f64, start_deg: f64, end_deg: f64, ccw: bool, connect: bool) {
        let (s, e) = (start_deg.to_radians(), end_deg.to_radians());
        let (sx, sy) = (cx + r * s.cos(), cy + r * s.sin());
        let (ex, ey) = (cx + r * e.cos(), cy + r * e.sin());
        if connect && self.open {
            self.line_to(sx, sy);
        } else {
            self.move_to(sx, sy);
        }
        let mut span = if ccw { end_deg - start_deg } else { start_deg - end_deg };
        span = span.rem_euclid(360.0);
        if span < 1e-9 && (end_deg - start_deg).abs() > 1e-9 {
            span = 360.0;
        }
        if span >= 360.0 - 1e-9 {
            // 整圆分两段
            let (mx, my) = (cx - (sx - cx), cy - (sy - cy));
            self.arc_to(r, r, 0.0, false, ccw, mx, my);
            self.arc_to(r, r, 0.0, false, ccw, sx, sy);
        } else {
            self.arc_to(r, r, 0.0, span > 180.0, ccw, ex, ey);
        }
    }

    pub fn close(&mut self) {
        if self.open {
            self.d.push_str(" Z");
            self.cur = self.start;
        }
    }

    pub fn is_empty(&self) -> bool {
        self.d.is_empty()
    }

    pub fn finish(self) -> String {
        self.d
    }
}
