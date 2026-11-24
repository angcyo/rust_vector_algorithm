pub mod ai;
pub mod dxf;
pub mod path;
pub mod plt;

pub use ai::ai_to_svg_path;
pub use dxf::dxf_to_svg_path;
pub use plt::plt_to_svg_path;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plt_basic() {
        let d = plt_to_svg_path("IN;SP1;PU0,0;PD40,0,40,40;PU;", 1.0);
        assert_eq!(d, "M0 0 L40 0 L40 -40");
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
        assert_eq!(d, "M0 0 L10 0 C10 -10 10 -20 0 -20 Z");
    }
}
