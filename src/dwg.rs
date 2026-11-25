use crate::dxf::{ellipse, poly, spline, Ent};
use crate::path::PathBuilder;

/// 解析 DWG (仅 AC1015 / R2000-2002) 的模型空间实体为 SVG path. 单位按图形单位 (视为 mm) 原样输出.
/// 支持 LINE, CIRCLE, ARC, LWPOLYLINE, POLYLINE_2D/VERTEX_2D, ELLIPSE, SPLINE, SOLID/TRACE.
pub fn dwg_to_svg_path(data: &[u8]) -> String {
    let mut pb = PathBuilder::new(true);
    if !data.starts_with(b"AC1015") {
        return String::new();
    }
    let objs = object_map(data);
    let mut i = 0;
    while i < objs.len() {
        let Some(o) = read_obj(data, objs[i].1) else {
            i += 1;
            continue;
        };
        let mut r = o.r;
        match o.ty {
            19 => line(&mut r, &mut pb, o.mode),
            18 => circle(&mut r, &mut pb, o.mode),
            17 => arc(&mut r, &mut pb, o.mode),
            77 => lwpoly(&mut r, &mut pb, o.mode),
            35 => ellipse_e(&mut r, &mut pb, o.mode),
            36 => spline_e(&mut r, &mut pb, o.mode),
            31 | 32 => solid(&mut r, &mut pb, o.mode),
            15 => {
                // 顶点是独立对象, 依赖其紧随多段线之后(句柄递增)到 SEQEND 为止
                let flags = r.bs();
                let mut pts = Vec::new();
                let mut j = i + 1;
                while j < objs.len() {
                    let Some(v) = read_obj(data, objs[j].1) else { break };
                    if v.ty != 10 {
                        break;
                    }
                    let mut vr = v.r;
                    if let Some((f, p)) = vertex(&mut vr) {
                        if f & 16 == 0 {
                            pts.push(p);
                        }
                    }
                    j += 1;
                }
                if o.mode != 1 {
                    poly(&mut pb, &pts, flags & 1 == 1);
                }
                i = j;
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    pb.finish()
}

/// 返回 (类型 -> 数量), 便于调试.
pub fn dwg_object_types(data: &[u8]) -> Vec<(i32, usize)> {
    let mut m = std::collections::BTreeMap::new();
    for (_, off) in object_map(data) {
        if let Some(o) = read_obj(data, off) {
            *m.entry(o.ty).or_insert(0usize) += 1;
        }
    }
    m.into_iter().collect()
}

fn object_map(d: &[u8]) -> Vec<(u64, usize)> {
    let rl = |p: usize| d.get(p..p + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap()) as usize);
    let n = rl(0x15).unwrap_or(0);
    let mut start = None;
    for k in 0..n.min(16) {
        let p = 0x19 + k * 9;
        if d.get(p) == Some(&2) {
            start = rl(p + 1);
        }
    }
    let Some(mut pos) = start else { return Vec::new() };
    let mut out = Vec::new();
    let (mut handle, mut off) = (0u64, 0i64);
    while pos + 2 <= d.len() {
        let size = ((d[pos] as usize) << 8) | d[pos + 1] as usize;
        if size <= 2 {
            break;
        }
        let end = (pos + size).min(d.len());
        let mut p = pos + 2;
        while p < end {
            let (mut h, mut sh) = (0u64, 0);
            while p < end {
                let b = d[p];
                p += 1;
                h |= ((b & 0x7f) as u64) << sh;
                sh += 7;
                if b & 0x80 == 0 {
                    break;
                }
            }
            let (mut v, mut sh, mut neg) = (0i64, 0, false);
            while p < end {
                let b = d[p];
                p += 1;
                if b & 0x80 == 0 {
                    v |= ((b & 0x3f) as i64) << sh;
                    neg = b & 0x40 != 0;
                    break;
                }
                v |= ((b & 0x7f) as i64) << sh;
                sh += 7;
            }
            handle += h;
            off += if neg { -v } else { v };
            if off >= 0 {
                out.push((handle, off as usize));
            }
        }
        pos += size + 2;
    }
    out.sort_by_key(|x| x.0);
    out
}

struct Obj<'a> {
    ty: i32,
    mode: u32,
    r: Bits<'a>,
}

fn read_obj(d: &[u8], off: usize) -> Option<Obj<'_>> {
    // MS: 对象数据字节数
    let (mut size, mut p, mut sh) = (0usize, off, 0);
    loop {
        let w = u16::from_le_bytes([*d.get(p)?, *d.get(p + 1)?]) as usize;
        p += 2;
        size |= (w & 0x7fff) << sh;
        sh += 15;
        if w & 0x8000 == 0 {
            break;
        }
    }
    let body = d.get(p..p.checked_add(size)?)?;
    let mut r = Bits { d: body, pos: 0 };
    let ty = r.bs();
    let _bitsize = r.rl();
    r.handle();
    loop {
        let n = r.bs();
        if n == 0 || r.eof() {
            break;
        }
        r.handle();
        r.pos += n as usize * 8;
    }
    if r.b() == 1 {
        let n = r.rl();
        r.pos += n as usize * 8;
    }
    let mode = r.bb();
    // 非实体对象 (层/块等) 无意义的字段会读出垃圾, 调用方只处理已知实体类型
    r.bl(); // numreactors
    r.b(); // nolinks
    r.bs(); // color
    r.bd(); // ltscale
    r.bb(); // ltype flags
    r.bb(); // plotstyle flags
    r.bs(); // invisibility
    r.rc(); // lineweight
    Some(Obj { ty, mode, r })
}

struct Bits<'a> {
    d: &'a [u8],
    pos: usize,
}

impl Bits<'_> {
    fn eof(&self) -> bool {
        self.pos >= self.d.len() * 8
    }
    fn b(&mut self) -> u32 {
        let byte = self.d.get(self.pos / 8).copied().unwrap_or(0);
        let v = (byte >> (7 - self.pos % 8)) & 1;
        self.pos += 1;
        v as u32
    }
    fn bits(&mut self, n: usize) -> u32 {
        (0..n).fold(0, |a, _| a << 1 | self.b())
    }
    fn bb(&mut self) -> u32 {
        self.bits(2)
    }
    fn rc(&mut self) -> u8 {
        self.bits(8) as u8
    }
    fn rl(&mut self) -> u32 {
        u32::from_le_bytes([self.rc(), self.rc(), self.rc(), self.rc()])
    }
    fn rd(&mut self) -> f64 {
        let mut b = [0u8; 8];
        for x in &mut b {
            *x = self.rc();
        }
        f64::from_le_bytes(b)
    }
    fn bs(&mut self) -> i32 {
        match self.bb() {
            0 => i16::from_le_bytes([self.rc(), self.rc()]) as i32,
            1 => self.rc() as i32,
            2 => 0,
            _ => 256,
        }
    }
    fn bl(&mut self) -> u32 {
        match self.bb() {
            0 => self.rl(),
            1 => self.rc() as u32,
            _ => 0,
        }
    }
    fn bd(&mut self) -> f64 {
        match self.bb() {
            0 => self.rd(),
            1 => 1.0,
            _ => 0.0,
        }
    }
    fn dd(&mut self, def: f64) -> f64 {
        let mut b = def.to_le_bytes();
        match self.bb() {
            0 => return def,
            1 => {
                for x in b.iter_mut().take(4) {
                    *x = self.rc();
                }
            }
            2 => {
                b[4] = self.rc();
                b[5] = self.rc();
                for x in b.iter_mut().take(4) {
                    *x = self.rc();
                }
            }
            _ => return self.rd(),
        }
        f64::from_le_bytes(b)
    }
    fn handle(&mut self) {
        self.bits(4);
        let n = self.bits(4);
        self.pos += n as usize * 8;
    }
    fn bt(&mut self) {
        if self.b() == 0 {
            self.bd();
        }
    }
    /// 挤出方向; 返回 Z 分量
    fn be(&mut self) -> f64 {
        if self.b() == 1 {
            1.0
        } else {
            self.bd();
            self.bd();
            self.bd()
        }
    }
}

fn mirrored(pb: &mut PathBuilder, nz: f64, mode: u32) -> bool {
    if mode == 1 {
        return false;
    }
    if nz < 0.0 {
        pb.push_matrix();
        pb.concat([-1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
    }
    true
}

fn done(pb: &mut PathBuilder, nz: f64) {
    if nz < 0.0 {
        pb.pop_matrix();
    }
}

fn line(r: &mut Bits, pb: &mut PathBuilder, mode: u32) {
    let zero = r.b() == 1;
    let sx = r.rd();
    let ex = r.dd(sx);
    let sy = r.rd();
    let ey = r.dd(sy);
    if !zero {
        r.rd();
        r.dd(0.0);
    }
    r.bt();
    let nz = r.be();
    if mirrored(pb, nz, mode) {
        pb.move_to(sx, sy);
        pb.line_to(ex, ey);
        done(pb, nz);
    }
}

fn circle(r: &mut Bits, pb: &mut PathBuilder, mode: u32) {
    let (cx, cy, _cz, rad) = (r.bd(), r.bd(), r.bd(), r.bd());
    r.bt();
    let nz = r.be();
    if mirrored(pb, nz, mode) {
        pb.arc_center(cx, cy, rad, 0.0, 360.0, true, false);
        done(pb, nz);
    }
}

fn arc(r: &mut Bits, pb: &mut PathBuilder, mode: u32) {
    let (cx, cy, _cz, rad) = (r.bd(), r.bd(), r.bd(), r.bd());
    r.bt();
    let nz = r.be();
    let (a0, a1) = (r.bd(), r.bd());
    if mirrored(pb, nz, mode) {
        pb.arc_center(cx, cy, rad, a0.to_degrees(), a1.to_degrees(), true, false);
        done(pb, nz);
    }
}

fn lwpoly(r: &mut Bits, pb: &mut PathBuilder, mode: u32) {
    let flag = r.bs();
    if flag & 4 != 0 {
        r.bd();
    }
    if flag & 8 != 0 {
        r.bd();
    }
    if flag & 2 != 0 {
        r.bd();
    }
    let mut nz = 1.0;
    if flag & 1 != 0 {
        r.bd();
        r.bd();
        nz = r.bd();
    }
    let np = r.bl() as usize;
    let nb = if flag & 16 != 0 { r.bl() as usize } else { 0 };
    let nw = if flag & 32 != 0 { r.bl() as usize } else { 0 };
    let _ = nw;
    if np == 0 || np > 1_000_000 {
        return;
    }
    let mut pts = Vec::with_capacity(np);
    let (mut px, mut py) = (r.rd(), r.rd());
    pts.push((px, py, 0.0));
    for _ in 1..np {
        px = r.dd(px);
        py = r.dd(py);
        pts.push((px, py, 0.0));
    }
    for k in 0..nb.min(np) {
        pts[k].2 = r.bd();
    }
    if mirrored(pb, nz, mode) {
        poly(pb, &pts, flag & 512 != 0);
        done(pb, nz);
    }
}

fn vertex(r: &mut Bits) -> Option<(i32, (f64, f64, f64))> {
    let flags = r.rc() as i32;
    let (x, y, _z) = (r.bd(), r.bd(), r.bd());
    let sw = r.bd();
    if sw >= 0.0 {
        r.bd();
    }
    let bulge = r.bd();
    Some((flags, (x, y, bulge)))
}

fn ellipse_e(r: &mut Bits, pb: &mut PathBuilder, mode: u32) {
    let c = [r.bd(), r.bd(), r.bd()];
    let m = [r.bd(), r.bd(), r.bd()];
    let n = [r.bd(), r.bd(), r.bd()];
    let (ratio, t0, t1) = (r.bd(), r.bd(), r.bd());
    let e = Ent {
        kind: "ELLIPSE".into(),
        p: vec![(10, c[0]), (20, c[1]), (11, m[0]), (21, m[1]), (40, ratio), (41, t0), (42, t1)],
        sval: vec![],
    };
    if mirrored(pb, n[2], mode) {
        ellipse(pb, &e);
        done(pb, n[2]);
    }
}

fn spline_e(r: &mut Bits, pb: &mut PathBuilder, mode: u32) {
    let scenario = r.bl();
    let degree = r.bl();
    let mut p = vec![(71, degree as f64)];
    if scenario == 2 {
        r.bd();
        for _ in 0..6 {
            r.bd();
        }
        let n = r.bl() as usize;
        if n > 1_000_000 {
            return;
        }
        for _ in 0..n {
            let (x, y, _) = (r.bd(), r.bd(), r.bd());
            p.push((11, x));
            p.push((21, y));
        }
    } else {
        let rational = r.b() == 1;
        r.b();
        r.b();
        r.bd();
        r.bd();
        let (nk, nc) = (r.bl() as usize, r.bl() as usize);
        let weighted = r.b() == 1;
        if nk > 1_000_000 || nc > 1_000_000 {
            return;
        }
        for _ in 0..nk {
            p.push((40, r.bd()));
        }
        for _ in 0..nc {
            let (x, y, _) = (r.bd(), r.bd(), r.bd());
            p.push((10, x));
            p.push((20, y));
            if weighted || rational && weighted {
                p.push((41, r.bd()));
            }
        }
    }
    if mode != 1 {
        spline(pb, &Ent { kind: "SPLINE".into(), p, sval: vec![] });
    }
}

fn solid(r: &mut Bits, pb: &mut PathBuilder, mode: u32) {
    r.bt();
    r.bd();
    let mut c = [(0.0, 0.0); 4];
    for v in &mut c {
        *v = (r.rd(), r.rd());
    }
    let nz = r.be();
    if mirrored(pb, nz, mode) {
        pb.move_to(c[0].0, c[0].1);
        for q in [c[1], c[3], c[2]] {
            pb.line_to(q.0, q.1);
        }
        pb.close();
        done(pb, nz);
    }
}
