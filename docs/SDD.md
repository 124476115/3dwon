# TDModeler — 跨平台 3D 建模软件 设计文档 (SDD)

> 目标：打造一个跨平台、高性能、开源的 3D 建模软件，覆盖中望 3DOne 的核心建模与 3D 打印输出能力，并能导出 STL/OBJ/3MF 等切片格式。
> 方法论：SDD（先设计）+ TDD（测试驱动开发）。所有纯逻辑（几何、IO、文档）先行以测试驱动实现；渲染/UI 在逻辑稳定后接入。

---

## 1. 产品定位与范围

### 1.1 定位
面向教育 / 创客 / 3D 打印用户的实体建模工具，强调：易用、参数化、可直接导出打印切片格式。

### 1.2 功能范围（对标 3DOne，分模块）
| 模块 | 功能 | 优先级 |
|------|------|--------|
| 基本实体 | 六面体、球体、圆柱、圆锥、椭球、圆环 | P0 |
| 草图 | 矩形、圆、椭圆、正多边形、直线、圆弧、多段线 | P0 |
| 草图编辑 | 圆角、倒角、修剪/延伸、偏移 | P1 |
| 特征造型 | 拉伸、旋转、扫掠、放样、拔模 | P0/P1 |
| 基础编辑 | 移动、旋转、缩放、阵列(矩形/圆形)、镜像 | P0 |
| 组合编辑 | 布尔并集/差集/交集 | P0 |
| 特殊功能 | 抽壳、实体分割、圆柱折弯、圆角/倒角(实体) | P2 |
| 显示/材质 | 线框/实体模式、颜色、材质、视图导航、网格捕捉 | P1 |
| 测量 | 距离、尺寸 | P1 |
| 导入 | STL、OBJ、(IGES/STP 远期) | P0/P2 |
| 导出 | STL(二进制/ASCII)、OBJ、3MF、(IGES/STP 远期) | P0 |
| STL 工程 | 自动检查(流形/法向)、分离分割、修复 | P2 |

### 1.3 非目标（本期）
装配动画、曲面 NURBS 精修、工程图、Python 编程、云社区。

---

## 2. 技术栈

| 关注点 | 选型 | 理由 |
|--------|------|------|
| 语言 | Rust (stable, MSRV 1.85+) | 高性能、内存安全、跨平台原生编译 |
| 几何内核 | `manifold-rust` (纯 Rust 版 Manifold) | 布尔/拉伸/旋转/凸包，纯 Rust 无需 C++ 工具链，686 测试保证稳健 |
| 数学 | `nalgebra` | 与 manifold 生态一致；矩阵/变换/向量 |
| 窗口/事件 | `winit` 0.30 | Linux/Win/macOS 统一 |
| 渲染 | `wgpu` 0.29 (Vulkan/Metal/DX12) | 跨平台 GPU 渲染，高性能 |
| UI | `egui` + `egui-wgpu` + `egui-winit` | 即时模式、跨平台、易嵌入 |
| 错误 | `thiserror` | 统一错误类型 |
| 异步初始化 | `pollster` | GPU 初始化 |

导出格式：STL（二进制+ASCII）、OBJ、3MF（基于 XML/zip）。

---

## 3. 架构（Cargo Workspace）

```
tdmodeler/
├── Cargo.toml                 (workspace)
├── crates/
│   ├── tdmodeler-core/        (纯逻辑，无 GPU) ★TDD 重点
│   │   ├── math.rs            向量/矩阵/变换
│   │   ├── geometry.rs        包裹 manifold：实体/布尔/特征
│   │   ├── sketch.rs          2D 草图 → 多边形（用于拉伸/旋转）
│   │   ├── features.rs        拉伸/旋转/扫掠/放样/抽壳/阵列
│   │   ├── document.rs        文档模型：Body / Sketch / 历史树
│   │   └── mesh.rs            网格数据结构与属性
│   ├── tdmodeler-io/          (导入/导出) ★TDD 重点
│   │   ├── stl.rs             二进制/ASCII 读写 + 往返测试
│   │   ├── obj.rs             OBJ 读写
│   │   └── amf_3mf.rs         3MF 导出
│   ├── tdmodeler-render/      wgpu 渲染管线
│   │   ├── renderer.rs        管线/相机/拾取
│   │   ├── camera.rs          透视轨道相机
│   │   └── shaders/*.wgsl
│   └── tdmodeler-app/         winit+egui 应用外壳
│       ├── main.rs            事件循环
│       ├── ui.rs              面板/工具栏/属性
│       └── state.rs           应用状态、撤销重做
└── tests/                     集成测试（端到端建模→导出）
```

### 3.1 分层依赖
`app → render + ui`, `app/core → io`, `core → manifold-rust + nalgebra`。
渲染层依赖 core 的网格输出（TriangleMesh），不直接持有几何内核状态。

### 3.2 数据模型（document.rs）
- `Document`：拥有 `bodies: Vec<Body>`、`sketches: Vec<Sketch>`、`history: HistoryTree`。
- `Body`：持有最终 `TriangleMesh`（由特征树求值得到），以及名称/材质/颜色。
- `Sketch`：2D 曲线集合 → 可转为 `Polygon2D`（带洞多边形）供拉伸/旋转。
- `HistoryTree`：特征节点（Primitive / Extrude / Boolean / Transform），支持重算与撤销。
- 撤销/重做：命令模式 + 快照（文档级轻量克隆）。

### 3.3 几何内核策略
- 实体以 manifold `Manifold` 在 core 内建图；最终转 `TriangleMesh { positions: Vec<[f32;3]>, indices: Vec<u32>, normals }` 供渲染与导出。
- 布尔：`union/difference/intersection`。
- 拉伸：`CrossSection`（2D 多边形）→ `extrude(height, twist, scale)`。
- 旋转：`CrossSection` → `revolve(angle)`。
- 这些操作全部在 core 内以纯函数暴露，便于 TDD（无需 GPU）。

---

## 4. 测试策略（TDD）

1. **core 单元测试**（无 GPU，可 `cargo test` 全跑）：
   - math：变换矩阵、法向、包围盒。
   - geometry：基本实体体积/表面积正确性（如单位立方体体积=1）。
   - features：拉伸高度=输入高度；旋转 360° 实体体积≈解析值。
   - boolean：并集体积 ≤ 各体积和；差集结果非空且自交为 0（用 manifold 检查）。
   - sketch：正多边形顶点数、圆离散、带洞多边形面积。
   - document：历史重算、撤销/重做一致性。
2. **io 单元测试**：
   - STL 二进制：写→读往返，顶点/法向一致；体积保留。
   - STL ASCII：同上。
   - OBJ：往返；与 STL 互转顶点数一致。
   - 3MF：生成合法 zip+xml，可被解析。
3. **集成测试**：建一个带孔方块（拉伸矩形+圆孔相减）→ 导出 STL → 重新读回 → 断言三角面数>0 且为流形。

---

## 5. 导出格式要点
- **STL 二进制**：小端 f32，80 字节头 + u32 面数 + 每面 12 字节法向 + 9 字节顶点 + u16 属性。TDD 往返校验。
- **STL ASCII**：`solid`/`facet normal`/`outer loop`/`vertex`/`endloop`/`endfacet`/`endsolid`。
- **OBJ**：`v`/`f`，可选 `vn`。
- **3MF**：zip 内含 `3D/3dmodel.model`（XML），描述 `vertices` 与 `triangles`，兼容 PrusaSlicer/Cura。

---

## 6. 跨平台与构建
- `cargo build --release` 在 Linux/Windows/macOS 产出原生可执行文件。
- CI：GitHub Actions 三平台矩阵；`cargo test` 全量；`cargo clippy` 静态检查。
- 发布：按平台打包（Linux AppImage / Windows installer / macOS dmg）。

---

## 7. 开发里程碑（分步）

| 阶段 | 内容 | 产出 |
|------|------|------|
| M0 | 环境/Schema/SDD | 本文档、workspace 骨架 |
| M1 | core: math + geometry + features + primitives（TDD） | 可单元测试的建模内核 |
| M2 | io: STL/OBJ/3MF（TDD 往返） | 导出能力 |
| M3 | document + 撤销重做 + 集成测试 | 端到端建模→导出 |
| M4 | render: wgpu 相机/网格渲染 | 可视化 |
| M5 | app: egui UI（工具栏/属性/视图） | 可用原型 |
| M6 | 进阶：阵列/镜像/抽壳/草图编辑/测量 | 逼近 3DOne |

---

## 8. 风险
- manifold-rust 编译耗时/体积：纯 Rust，可接受；必要时关 `parallel`。
- wgpu 在无显示环境：用 Vulkan(lavapipe) 软件渲染验证编译，真实 GPU 运行。
- 曲面/工程图：本期不做，路线图中标记远期。
