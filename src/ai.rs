use crate::path::PathBuilder;
use flate2::read::ZlibDecoder;
use std::io::Read;

/// 解析 Adobe Illustrator (.ai) 为 SVG path.
/// 支持: 旧版 PostScript/EPS 风格 AI (m l c v y h 等) 以及 PDF 兼容 AI (内容流, 支持 FlateDecode, 操作符 m l c v y h re).
pub fn ai_to_svg_path(data: &[u8]) -> String {
    let mut pb = PathBuilder::new(true);
    let mut found = false;

    if data.starts_with(b"%PDF") || find(data, b"%PDF-").is_some() {
        for content in pdf_streams(data) {
            found = true;
            pb.push_matrix();
            interpret(&content, &mut pb);
            pb.pop_matrix();
        }
    }
    if !found || pb.is_empty() {
        let text = String::from_utf8_lossy(data);
        interpret(&text, &mut pb);
    }
    pb.finish()
}

fn find(h: &[u8], n: &[u8]) -> Option<usize> {
    h.windows(n.len()).position(|w| w == n)
}

fn pdf_streams(data: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    let mut pos = 0;
    while let Some(s) = find(&data[pos..], b"stream") {
        let mut start = pos + s + 6;
        // 排除 "endstream"
        if pos + s >= 3 && &data[pos + s - 3..pos + s] == b"end" {
            pos = start;
            continue;
        }
        if data.get(start) == Some(&b'\r') {
            start += 1;
        }
        if data.get(start) == Some(&b'\n') {
            start += 1;
        }
        let Some(e) = find(&data[start..], b"endstream") else { break };
        let raw = &data[start..start + e];
        // 取字典判断是否 Flate
        let obj_start = data[..pos + s].windows(3).rposition(|w| w == b"obj").unwrap_or(0);
        let dict = &data[obj_start..pos + s];
        let mut bytes = raw.to_vec();
        for name in filters(dict) {
            bytes = match name.as_str() {
                "FlateDecode" | "Fl" => {
                    let mut d = Vec::new();
                    let _ = ZlibDecoder::new(&bytes[..]).read_to_end(&mut d);
                    d
                }
                "ASCII85Decode" | "A85" => ascii85(&bytes),
                "ASCIIHexDecode" | "AHx" => asciihex(&bytes),
                _ => Vec::new(),
            };
        }
        let text = String::from_utf8_lossy(&bytes).into_owned();
        if text.contains(" m") || text.contains(" l") || text.contains(" re") {
            out.push(text);
        }
        pos = start + e + 9;
    }
    out
}

fn filters(dict: &[u8]) -> Vec<String> {
    let s = String::from_utf8_lossy(dict);
    let Some(i) = s.find("/Filter") else { return Vec::new() };
    let rest = &s[i + 7..];
    let end = if rest.trim_start().starts_with('[') {
        rest.find(']').unwrap_or(rest.len())
    } else {
        let t = rest.trim_start();
        let off = rest.len() - t.len();
        off + t[1.min(t.len())..].find('/').map(|p| p + 1).unwrap_or(t.len())
    };
    rest[..end].split('/').skip(1).map(|n| n.split(|c: char| !c.is_alphanumeric()).next().unwrap_or("").to_string()).collect()
}

fn ascii85(d: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let (mut grp, mut n) = ([0u32; 5], 0);
    let mut i = 0;
    while i < d.len() {
        let c = d[i];
        i += 1;
        match c {
            b'~' => break,
            b'z' if n == 0 => out.extend_from_slice(&[0; 4]),
            b'!'..=b'u' => {
                grp[n] = (c - b'!') as u32;
                n += 1;
                if n == 5 {
                    let v = grp.iter().fold(0u32, |a, &g| a.wrapping_mul(85).wrapping_add(g));
                    out.extend_from_slice(&v.to_be_bytes());
                    n = 0;
                }
            }
            _ => {}
        }
    }
    if n > 1 {
        for g in grp.iter_mut().skip(n) {
            *g = 84;
        }
        let v = grp.iter().fold(0u32, |a, &g| a.wrapping_mul(85).wrapping_add(g));
        out.extend_from_slice(&v.to_be_bytes()[..n - 1]);
    }
    out
}

fn asciihex(d: &[u8]) -> Vec<u8> {
    let h: Vec<u8> = d.iter().take_while(|&&c| c != b'>').filter_map(|&c| (c as char).to_digit(16).map(|v| v as u8)).collect();
    h.chunks(2).map(|p| p[0] << 4 | p.get(1).copied().unwrap_or(0)).collect()
}

fn interpret(text: &str, pb: &mut PathBuilder) {
    let mut stack: Vec<f64> = Vec::new();
    let mut start = (0.0, 0.0);
    let mut depth = 0;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('%') {
            continue;
        }
        for tok in line.split_whitespace() {
            if let Ok(v) = tok.parse::<f64>() {
                stack.push(v);
                continue;
            }
            let s = std::mem::take(&mut stack);
            let n = s.len();
            match tok {
                "cm" if n >= 6 => {
                    let a = &s[n - 6..];
                    pb.concat([a[0], a[1], a[2], a[3], a[4], a[5]]);
                }
                "q" => {
                    pb.push_matrix();
                    depth += 1;
                }
                "Q" if depth > 0 => {
                    pb.pop_matrix();
                    depth -= 1;
                }
                "rmoveto" if n >= 2 => {
                    let (x, y) = (pb.cur.0 + s[n - 2], pb.cur.1 + s[n - 1]);
                    pb.move_to(x, y);
                    start = (x, y);
                }
                "rlineto" if n >= 2 => {
                    let (x, y) = (pb.cur.0 + s[n - 2], pb.cur.1 + s[n - 1]);
                    pb.line_to(x, y);
                }
                "rcurveto" if n >= 6 => {
                    let (cx, cy) = pb.cur;
                    let a = &s[n - 6..];
                    pb.cubic_to(cx + a[0], cy + a[1], cx + a[2], cy + a[3], cx + a[4], cy + a[5]);
                }
                "m" | "M" if n >= 2 => {
                    pb.move_to(s[n - 2], s[n - 1]);
                    start = (s[n - 2], s[n - 1]);
                }
                "l" | "L" | "ln" | "lineto" if n >= 2 => pb.line_to(s[n - 2], s[n - 1]),
                "moveto" if n >= 2 => {
                    pb.move_to(s[n - 2], s[n - 1]);
                    start = (s[n - 2], s[n - 1]);
                }
                "c" | "C" | "curveto" if n >= 6 => {
                    let a = &s[n - 6..];
                    pb.cubic_to(a[0], a[1], a[2], a[3], a[4], a[5]);
                }
                // v: 第一控制点为当前点
                "v" | "V" if n >= 4 => {
                    let a = &s[n - 4..];
                    let (cx, cy) = pb.cur;
                    pb.cubic_to(cx, cy, a[0], a[1], a[2], a[3]);
                }
                // y: 第二控制点为终点
                "y" | "Y" if n >= 4 => {
                    let a = &s[n - 4..];
                    pb.cubic_to(a[0], a[1], a[2], a[3], a[2], a[3]);
                }
                "re" if n >= 4 => {
                    let a = &s[n - 4..];
                    pb.move_to(a[0], a[1]);
                    pb.line_to(a[0] + a[2], a[1]);
                    pb.line_to(a[0] + a[2], a[1] + a[3]);
                    pb.line_to(a[0], a[1] + a[3]);
                    pb.close();
                }
                "h" | "closepath" | "s" | "b" | "f" | "F" | "B" if matches!(tok, "h" | "closepath" | "s" | "b") => {
                    pb.close();
                    pb.cur = start;
                }
                _ => {}
            }
        }
    }
}
