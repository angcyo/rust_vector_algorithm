# rust_vector_algorithm

将 PLT (HPGL)、DXF、AI 文件解析为 SVG path 的 `d` 字符串。输出坐标 Y 轴已翻转（SVG 向下为正）。

```rust
rust_vector_algorithm::plt_to_svg_path(&text);
rust_vector_algorithm::dxf_to_svg_path(&text);
rust_vector_algorithm::ai_to_svg_path(&bytes);
```

**所有输出统一为 mm**（Y 轴翻转，不做平移，坐标可能为负）：
- PLT：默认 40 单位 = 1mm；支持 `IP` + `SC` 用户单位（仅按 X 方向等比换算，忽略偏移与各向异性；未给 `IP` 时假设 P1-P2 的 X 跨度为 10000 单位）
- DXF：按文件头 `$INSUNITS` 换算；无该变量时，`$MEASUREMENT=0` 按英寸，否则按 mm
- AI/PDF：pt × 25.4/72

命令行：`cargo run -- file.plt|dxf|ai [out.svg]`

## 官方文档

| 格式 | 文档 |
|---|---|
| PLT / HPGL | HP-GL/2 and HP RTL Reference Guide: https://www.hp.com/ctg/Manual/bpl13210.pdf （概览：https://en.wikipedia.org/wiki/HP-GL） |
| DXF | Autodesk DXF Reference: https://help.autodesk.com/view/OARX/2024/ENU/?guid=GUID-235B22E0-A567-4CF6-92D3-38A2306D73F3 |
| AI | Adobe 未公开完整的现代 AI 规范。现代 AI 内含 PDF 数据，参见 PDF 规范 (ISO 32000-1): https://opensource.adobe.com/dc-acrobat-sdk-docs/pdfstandards/PDF32000_2008.pdf ；旧版 AI 基于 PostScript: https://www.adobe.com/jp/print/postscript/pdfs/PLRM.pdf |
| DWG | Open Design Specification for .dwg files: https://www.opendesign.com/files/guestdownloads/OpenDesign_Specification_for_.dwg_files.pdf |

## 支持内容

### PLT (HPGL)
- 指令：`IN` `DF` `PU` `PD` `PA` `PR`（绝对/相对坐标）、`AA` `AR`（圆弧）、`AT` `RT`（三点弧）、`BZ` `BR`（贝塞尔）、`CI`（圆）、`ER` `RR` `EA` `RA`（矩形轮廓）、`PE`（压缩折线，base64/base32、小数位、绝对坐标 `=`、抬笔 `<`）
- 指令可用 `;`、换行或直接连写分隔
- 扩展名：`.plt` `.hpgl` `.hpg` `.hgl` `.prn`

### DXF（ASCII 与二进制）
- ASCII DXF 与二进制 DXF (R13+)
- 读取 `ENTITIES` 与 `BLOCKS` 段
- 实体：`LINE`、`LWPOLYLINE`（含 bulge、闭合）、`POLYLINE`/`VERTEX`（含 bulge、闭合）、`CIRCLE`、`ARC`、`ELLIPSE`、`SPLINE`（NURBS 求值后以折线近似；节点不合法时退化为拟合点/控制点连线）、`SOLID`/`TRACE`/`3DFACE`（轮廓）
- `INSERT` 块引用：位置、缩放、旋转、阵列（行/列）、嵌套（最多 16 层）、块基点
- 挤出方向 Z 为负时做 X 镜像

### DWG（仅 AC1015，即 AutoCAD R2000/2000i/2002）
- 实体：`LINE` `CIRCLE` `ARC` `LWPOLYLINE` `ELLIPSE` `SPLINE` `SOLID`/`TRACE` `POLYLINE`(2D)+`VERTEX`；跳过图纸空间实体
- 其他版本请先用 ODA File Converter 转为 DXF 或 R2000 DWG

### AI / EPS / PDF
- 旧版 PostScript 风格：`m` `l` `c` `v` `y` `h` 及 `moveto` `lineto` `curveto` `rmoveto` `rlineto` `rcurveto` `closepath`
- PDF 兼容 AI 与 `.pdf`：提取内容流（未压缩，或 FlateDecode / ASCII85Decode / ASCIIHexDecode 及其组合），支持 `m` `l` `c` `v` `y` `h` `re`
- 变换矩阵 `cm` 与 `q`/`Q` 图形状态栈
- 扩展名：`.ai` `.eps` `.pdf` `.ps`

## 不支持内容

### PLT (HPGL)
- `RO` 旋转、`IW`、`EW`/`WG` 扇形、多边形模式 `PM`/`FP`/`EP` 的填充语义、`LB` 文字、`SP`/`LT`/`PW` 笔号/线型/线宽（解析但忽略）
- `SC` 仅支持 X 方向等比换算，不支持偏移、Y 独立比例和等比类型 (type 1/2)
- 圆的弦角（`CI` 第二参数）被忽略
- 二进制 PCL/RTL 内容

### DXF
- R12 二进制 DXF
- `HATCH`、`TEXT`/`MTEXT`、`DIMENSION`、`POINT`、`LEADER`、`IMAGE`、`MLINE`、`ATTRIB` 等其他实体
- 图层、颜色、线型、线宽、单位（`$INSUNITS`）
- 任意方向的挤出方向（仅处理 Z 为负的镜像）、Z 坐标
- 样条为折线近似，不输出精确曲线

### DWG
- 非 AC1015 版本（R13/R14 结构不同；R2004+ 有压缩与加密）
- `INSERT`/块定义、`HATCH`、`TEXT`、`DIMENSION`、3D 实体、3D/网格 `POLYLINE`、图层/颜色、单位（按 mm 原样输出）
- 仅在少量样本上验证，未知变体可能解析错误或输出为空

### AI / EPS / PDF
- 加密文件、PDF 对象流 (ObjStm) 以及 LZW、RunLength 等其他滤镜
- 颜色、填充/描边样式、渐变、裁剪路径、透明度、图案
- 文字、位图、符号、图层、画板、多页区分（所有页路径合并输出）
- Illustrator 私有数据（`%%BeginPrivate`、PGF 等）
- PostScript 的变量、过程、`gsave`/`grestore`/`translate`/`scale`/`rotate` 等通用语言特性
