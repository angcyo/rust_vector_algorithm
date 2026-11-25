pub mod ai;
pub mod dxf;
pub mod path;
pub mod plt;

pub use ai::ai_to_svg_path;
pub use dxf::dxf_to_svg_path;
pub use dxf::dxf_bytes_to_svg_path;
pub use plt::plt_to_svg_path;

///
/// @author <a href="mailto:angcyo@126.com">angcyo</a>
/// @date 2026/11/25
///

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dxf_insert_scaled() {
        let s = "0\nSECTION\n2\nBLOCKS\n0\nBLOCK\n2\nB\n10\n0\n20\n0\n0\nLINE\n10\n0\n20\n0\n11\n1\n21\n0\n0\nENDBLK\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n0\nINSERT\n2\nB\n10\n5\n20\n5\n41\n2\n42\n2\n0\nENDSEC\n0\nEOF\n";
        assert_eq!(dxf_to_svg_path(s), "M5 -5 L7 -5");
    }

    #[test]
    fn ai_cm() {
        let d = ai_to_svg_path(b"q 2 0 0 2 10 0 cm 0 0 m 1 1 l Q");
        assert_eq!(d, "M3.5278 0 L4.2333 -0.7056");
    }

    #[test]
    fn plt_three_point_arc() {
        let d = plt_to_svg_path("PU0,0;PD;AT40,40,80,0;", 1.0 / 40.0);
        assert!(d.contains(" A1 1 0 "), "{d}");
    }

    #[test]
    fn plt_basic() {
        let d = plt_to_svg_path("IN;SP1;PU0,0;PD40,0,40,40;PU;", 1.0 / 40.0);
        assert_eq!(d, "M0 0 L1 0 L1 -1");
    }

    #[test]
    fn plt_sc_user_units() {
        // 10000 绘图仪单位(250mm) 对应 100 用户单位 => 1 用户单位 = 2.5mm
        let d = plt_to_svg_path("IN;IP0,0,10000,10000;SC0,100,0,100;PU0,0;PD10,0;", 1.0 / 100.0);
        assert_eq!(d, "M0 0 L25 0");
    }

    #[test]
    fn dxf_inches_to_mm() {
        let s = "0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n1\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n0\nLINE\n10\n0\n20\n0\n11\n1\n21\n0\n0\nENDSEC\n0\nEOF\n";
        assert_eq!(dxf_to_svg_path(s), "M0 0 L25.4 0");
    }

    #[test]
    fn dxf_line_and_circle() {
        let s = "0\nSECTION\n2\nENTITIES\n0\nLINE\n10\n0\n20\n0\n11\n10\n21\n5\n0\nCIRCLE\n10\n0\n20\n0\n40\n2\n0\nENDSEC\n0\nEOF\n";
        let d = dxf_to_svg_path(s);
        assert!(d.starts_with("M0 0 L10 -5 M2 0 A2 2 0 0 0 -2 0"), "{d}");
    }

    #[test]
    fn ai_ps() {
        let d = ai_to_svg_path(b"0 0 m\n10 0 l\n10 10 10 20 0 20 c\nh\n");
        assert_eq!(d, "M0 0 L3.5278 0 C3.5278 -3.5278 3.5278 -7.0556 0 -7.0556 Z");
    }
}

