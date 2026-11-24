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
            interpret(&content, &mut pb);
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
        let dict_start = data[..pos + s].windows(2).rposition(|w| w == b"<<").unwrap_or(0);
        let dict = &data[dict_start..pos + s];
        let flate = find(dict, b"FlateDecode").is_some();
        let bytes = if flate {
            let mut d = Vec::new();
            if ZlibDecoder::new(raw).read_to_end(&mut d).is_err() && d.is_empty() {
                pos = start + e;
                continue;
            }
            d
        } else {
            raw.to_vec()
        };
        let text = String::from_utf8_lossy(&bytes).into_owned();
        if text.contains(" m") || text.contains(" l") || text.contains(" re") {
            out.push(text);
        }
        pos = start + e + 9;
    }
    out
}

fn interpret(text: &str, pb: &mut PathBuilder) {
    let mut stack: Vec<f64> = Vec::new();
    let mut start = (0.0, 0.0);
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
