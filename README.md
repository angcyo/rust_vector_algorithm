# rust_vector_algorithm

将 PLT (HPGL)、DXF、AI 文件解析为 SVG path 的 `d` 字符串。输出坐标 Y 轴已翻转（SVG 向下为正）。

```rust
rust_vector_algorithm::plt_to_svg_path(&text, 1.0 / 40.0); // 40 单位 = 1mm
rust_vector_algorithm::dxf_to_svg_path(&text);
rust_vector_algorithm::ai_to_svg_path(&bytes);
```

命令行：`cargo run -- file.plt|dxf|ai [out.svg]`

## 官方文档

| 格式 | 文档 |
|---|---|
| PLT / HPGL | HP-GL/2 and HP RTL Reference Guide: https://www.hp.com/ctg/Manual/bpl13210.pdf （概览：https://en.wikipedia.org/wiki/HP-GL） |
| DXF | Autodesk DXF Reference: https://help.autodesk.com/view/OARX/2024/ENU/?guid=GUID-235B22E0-A567-4CF6-92D3-38A2306D73F3 |
| AI | Adobe 未公开完整的现代 AI 规范。现代 AI 内含 PDF 数据，参见 PDF 规范 (ISO 32000-1): https://opensource.adobe.com/dc-acrobat-sdk-docs/pdfstandards/PDF32000_2008.pdf ；旧版 AI 基于 PostScript: https://www.adobe.com/jp/print/postscript/pdfs/PLRM.pdf |

## 支持内容

### PLT (HPGL)
- 指令：`IN` `DF` `PU` `PD` `PA` `PR`（绝对/相对坐标）、`AA` `AR`（圆弧）、`CI`（圆）、`PE`（压缩折线，base64/base32、小数位、绝对坐标 `=`、抬笔 `<`）
- 指令可用 `;`、换行或直接连写分隔
- `scale` 参数用于单位缩放

### DXF（仅 ASCII）
- 仅读取 `ENTITIES` 段
- 实体：`LINE`、`LWPOLYLINE`（含 bulge 圆弧、闭合）、`POLYLINE`/`VERTEX`（含 bulge、闭合）、`CIRCLE`、`ARC`、`ELLIPSE`（整椭圆与椭圆弧）、`SPLINE`（NURBS 求值后以折线近似；节点不合法时退化为拟合点/控制点连线）

### AI
- 旧版 PostScript 风格：`m` `l` `c` `v` `y` `h` 及 `moveto` `lineto` `curveto` `closepath`
- PDF 兼容 AI：提取内容流（未压缩或 FlateDecode），支持 `m` `l` `c` `v` `y` `h` `re`

## 不支持内容

### PLT (HPGL)
- HP-GL/2 的大部分指令：`SC` 缩放、`IP`/`IW`、`RO` 旋转、`AT`/`RT` 三点弧/贝塞尔、`ER`/`EA`/`RR`/`RA`/`EW` 矩形与扇形、多边形模式 `PM`/`FP`/`EP`、`LB` 文字、`SP`/`LT`/`PW` 笔号/线型/线宽（解析但忽略）
- 圆的弦角（`CI` 第二参数）被忽略
- 二进制 PCL/RTL 内容

### DXF
- 二进制 DXF
- `BLOCKS` 段与 `INSERT` 块引用、`HATCH`、`TEXT`/`MTEXT`、`DIMENSION`、`POINT`、`3DFACE`、`SOLID`、`LEADER`、`IMAGE` 等其他实体
- 图层、颜色、线型、线宽、单位（`$INSUNITS`）
- 非默认挤出方向（OCS 变换）、Z 坐标
- 有理 `SPLINE` 以外的精确曲线输出（样条为折线近似）

### AI
- 加密文件、PDF 对象流 (ObjStm) 以及 Flate 以外的滤镜（LZW、ASCII85 等）
- 变换矩阵 `cm`、图形状态栈 `q`/`Q`，因此嵌套变换与坐标系不会被应用
- 颜色、填充/描边样式、渐变、裁剪路径、透明度、图案
- 文字、位图、符号、图层、画板、多页
- Illustrator 私有数据（`%%BeginPrivate`、PGF 等）
