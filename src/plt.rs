use crate::path::PathBuilder;

/// 解析 HPGL (.plt) 为 SVG path. `scale` 为单位缩放 (HPGL 默认 40 单位 = 1mm, 传 1.0/40.0 得 mm).
/// 支持: IN PU PD PA PR AA AR CI SP 以及 PE 压缩折线.
pub fn plt_to_svg_path(text: &str, scale: f64) -> String {
    let mut pb = PathBuilder::new(true);
    let (mut x, mut y) = (0.0f64, 0.0f64);
    let mut pen_down = false;
    let mut relative = false;

    let bytes = text.as_bytes();
    let mut i = 0;
    // 切分指令: 以 ';' 或换行结束; 同时容忍无分隔符连写 (如 PU0,0PD1,1)
    while i < bytes.len() {
        while i < bytes.len() && !(bytes[i].is_ascii_alphabetic()) {
            i += 1;
        }
        if i + 1 >= bytes.len() {
            break;
        }
        let cmd = text[i..i + 2].to_ascii_uppercase();
        i += 2;
        if cmd == "PE" {
            let start = i;
            let mut end = i;
            while end < bytes.len() && bytes[end] != b';' {
                end += 1;
            }
            decode_pe(&text[start..end], scale, &mut pb, &mut x, &mut y, &mut pen_down);
            i = end;
            continue;
        }
        let start = i;
        while i < bytes.len() && !bytes[i].is_ascii_alphabetic() && bytes[i] != b';' {
            i += 1;
        }
        let args: Vec<f64> = text[start..i]
            .split(|c: char| c == ',' || c.is_whitespace())
            .filter_map(|s| s.trim().parse().ok())
            .collect();
        match cmd.as_str() {
            "IN" | "DF" => {
                pen_down = false;
                relative = false;
            }
            "PU" | "PD" => {
                pen_down = cmd == "PD";
                if cmd == "PU" {
                    // 抬笔: 后续 PD 重新 M
                }
                if !pen_down && args.is_empty() {
                    continue;
                }
                if pen_down && args.is_empty() {
                    pb.move_to(x * scale, y * scale);
                }
                plot(&args, relative, scale, &mut pb, &mut x, &mut y, pen_down);
            }
            "PA" | "PR" => {
                relative = cmd == "PR";
                plot(&args, relative, scale, &mut pb, &mut x, &mut y, pen_down);
            }
            "CI" => {
                if let Some(&r) = args.first() {
                    pb.arc_center(x * scale, y * scale, r * scale, 0.0, 360.0, true, false);
                }
            }
            "AA" | "AR" if args.len() >= 3 => {
                let (cx, cy) = if cmd == "AR" { (x + args[0], y + args[1]) } else { (args[0], args[1]) };
                let ang = args[2];
                let r = ((x - cx).powi(2) + (y - cy).powi(2)).sqrt();
                let a0 = (y - cy).atan2(x - cx).to_degrees();
                let a1 = a0 + ang;
                if pen_down {
                    if pb.is_empty() || (pb.cur.0 - x * scale).abs() > 1e-9 || (pb.cur.1 - y * scale).abs() > 1e-9 {
                        pb.move_to(x * scale, y * scale);
                    }
                    pb.arc_center(cx * scale, cy * scale, r * scale, a0, a1, ang >= 0.0, true);
                }
                x = cx + r * a1.to_radians().cos();
                y = cy + r * a1.to_radians().sin();
            }
            _ => {}
        }
    }
    pb.finish()
}

fn plot(args: &[f64], rel: bool, scale: f64, pb: &mut PathBuilder, x: &mut f64, y: &mut f64, down: bool) {
    for p in args.chunks_exact(2) {
        if rel {
            *x += p[0];
            *y += p[1];
        } else {
            *x = p[0];
            *y = p[1];
        }
        if down {
            // 起点未建立(例如先 PD 后 PA)
            pb.line_to(*x * scale, *y * scale);
        } else {
            pb.move_to(*x * scale, *y * scale);
        }
    }
}

// PE: 以 base-64 (默认, 小数位 fractional_bits=0) 编码的相对坐标.
fn decode_pe(s: &str, scale: f64, pb: &mut PathBuilder, x: &mut f64, y: &mut f64, down: &mut bool) {
    let b = s.as_bytes();
    let (mut i, mut base32, mut frac) = (0usize, false, 0i32);
    let mut relative = true;
    let read = |i: &mut usize, base32: bool| -> Option<f64> {
        let (flag, bits) = if base32 { (32u8, 5) } else { (64u8, 6) };
        let mut v: i64 = 0;
        let mut shift = 0;
        loop {
            let c = *b.get(*i)?;
            *i += 1;
            let c = c.wrapping_sub(63);
            let (c, last) = if c & flag != 0 { (c - flag, false) } else { (c, true) };
            v |= (c as i64) << shift;
            shift += bits;
            if last {
                break;
            }
        }
        Some(if v & 1 == 1 { -((v >> 1) as f64) } else { (v >> 1) as f64 })
    };
    while i < b.len() {
        match b[i] {
            b':' => {
                i += 1;
                // 选笔号, 忽略
                let _ = read(&mut i, base32);
            }
            b'<' => {
                i += 1;
                *down = false;
            }
            b'>' => {
                i += 1;
                if let Some(f) = read(&mut i, base32) {
                    frac = f as i32;
                }
            }
            b'=' => {
                i += 1;
                relative = false;
            }
            b'7' => {
                i += 1;
                base32 = true;
            }
            _ => {
                let (Some(dx), Some(dy)) = (read(&mut i, base32), read(&mut i, base32)) else { return };
                let k = 1.0 / 2f64.powi(frac);
                let (dx, dy) = (dx * k, dy * k);
                if relative {
                    *x += dx;
                    *y += dy;
                } else {
                    *x = dx;
                    *y = dy;
                    relative = true;
                }
                if *down {
                    pb.line_to(*x * scale, *y * scale);
                } else {
                    pb.move_to(*x * scale, *y * scale);
                    *down = true;
                }
            }
        }
    }
}
