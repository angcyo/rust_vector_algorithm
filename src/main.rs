use std::{env, fs, process};

///
/// @author <a href="mailto:angcyo@126.com">angcyo</a>
/// @date 2026/11/25
///

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("用法: {} <file.plt|dxf|ai> [out.svg]", args[0]);
        process::exit(1);
    }
    let path = &args[1];
    let ext = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    let data = fs::read(path).expect("读取文件失败");
    let d = match ext.as_str() {
        "plt" | "hpgl" | "hpg" | "hgl" | "prn" => rust_vector_algorithm::plt_to_svg_path(&String::from_utf8_lossy(&data), 1.0 / 40.0),
        "dwg" => rust_vector_algorithm::dwg_to_svg_path(&data),
        "dxf" => rust_vector_algorithm::dxf_bytes_to_svg_path(&data),
        "ai" | "eps" | "pdf" | "ps" => rust_vector_algorithm::ai_to_svg_path(&data),
        _ => {
            eprintln!("不支持的格式: {ext}");
            process::exit(1);
        }
    };
    match args.get(2) {
        Some(out) => {
            let svg = format!(
                "<svg xmlns=\"http://www.w3.org/2000/svg\"><path d=\"{d}\" fill=\"none\" stroke=\"black\"/></svg>"
            );
            fs::write(out, svg).expect("写入失败");
        }
        None => println!("{d}"),
    }
}

