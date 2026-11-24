use std::fmt::Write;

/// SVG path 构建器. `flip_y` 为 true 时 Y 取反 (CAD/HPGL 的 Y 轴向上, SVG 向下).
#[derive(Debug, Clone)]
pub struct PathBuilder {
    d: String,
    flip_y: bool,
    pub cur: (f64, f64),
    start: (f64, f64),
    open: bool,
}

fn num(v: f64) -> String {
    let s = format!("{:.4}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" || s.is_empty() { "0".into() } else { s.into() }
}

impl PathBuilder {
    pub fn new(flip_y: bool) -> Self {
        Self { d: String::new(), flip_y, cur: (0.0, 0.0), start: (0.0, 0.0), open: false }
    }

    fn p(&self, x: f64, y: f64) -> String {
        format!("{} {}", num(x), num(if self.flip_y { -y } else { y }))
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
        // 翻转 Y 后方向相反
        let sweep = if self.flip_y { !sweep_ccw } else { sweep_ccw };
        let rot = if self.flip_y { -rot_deg } else { rot_deg };
        let _ = write!(
            self.d,
            " A{} {} {} {} {} {}",
            num(rx),
            num(ry),
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
