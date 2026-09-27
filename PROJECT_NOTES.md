# 项目记录

最后更新：2026-09-28

## 项目目标

读取客服工单 JSON，每次通过配置指定两个参数，对原始工单进行一次遍历，输出两个参数
的二维计数矩阵。程序只呈现趋势和关联分布，不自动判断相关性。

## 当前实现

- 语言：Rust（edition 2024）。
- 程序入口：`src/main.rs`。
- 输入数据：`examples/tickets.json`，当前包含50条演示工单。
- 默认配置：`examples/params.json`，计算“最近30天 × category”。
- 核心输出：`result.json` 或 `web/result.json`。
- 展示页面：`web/index.html`。
- 3D图表：ECharts 5.6.0 + ECharts-GL 2.0.9，通过 CDN 加载。
- 自动部署：`.github/workflows/pages.yml`。

## 核心设计

`Analyzer` 接收两个实现 `Dimension` 接口的维度处理器。遍历每条工单时分别得到行、列
桶，并在稀疏 `HashMap` 中计数。遍历完成后再转换为固定二维矩阵，不重新遍历工单。

输出约定：

```text
matrix[左侧桶索引][右侧桶索引] = 组合出现次数
```

交换左右参数后，结果矩阵应互为转置，已有自动化测试覆盖该性质。

## 支持的维度

### time

- `hour_of_day`：00:00～23:00，共24桶。
- `last_30_days`：以数据中的最新日期为终点，向前30个自然日。
- `last_24_months`：以数据中的最新月份为终点，向前24个月。

时间格式固定为 `YYYY-MM-DD HH:MM`。缺失或格式错误会返回错误。

### category

当前50条示例数据中的全部类别均有独立桶：

- 退款退货
- 物流查询
- 商品咨询
- 账号问题
- 支付问题
- 投诉
- 其他

未知或缺失类别进入“其他”。`description` 不参与分类。

### priority

- 高
- 中
- 低
- 其他

未知或缺失优先级进入“其他”。

## 常用命令

运行测试：

```bash
cargo test --locked
```

生成默认矩阵：

```bash
cargo run --locked -- examples/tickets.json examples/params.json result.json
```

生成网页使用的数据：

```bash
cargo run --locked -- examples/tickets.json examples/params.json web/result.json
```

本地预览网页：

```bash
python3 -m http.server 8000 --directory web
```

然后访问 `http://localhost:8000`。不要直接使用 `file://` 打开页面，否则浏览器可能阻止
读取 `result.json`。

## 配置示例

每次固定两个维度：

```json
{
  "left": {
    "field": "created_at",
    "type": "time",
    "mode": "last_30_days"
  },
  "right": {
    "field": "category",
    "type": "category"
  }
}
```

其他可直接运行的配置位于 `examples/params_*.json`，包含参数镜像和三种时间模式。

## GitHub 与部署

- 源代码：https://github.com/caro07biu/cpp_practice
- 在线演示：https://caro07biu.github.io/cpp_practice/
- 默认分支：`main`
- Pages 来源：GitHub Actions

推送到 `main` 后，工作流会自动：

1. 执行 Rust 测试；
2. 生成 `web/result.json`；
3. 发布 `web` 目录到 GitHub Pages。

GitHub Pages 只托管静态页面。Rust 程序在 GitHub Actions 中执行，不会作为长期在线后端
运行。

## 后续修改注意事项

- 新增维度时，实现 `Dimension` 接口并在 `create_dimension` 中注册。
- 新增 category 值时，同步修改分类映射、桶顺序、测试和文档。
- 保持“原始工单只遍历一次”的设计约束。
- 二维矩阵是核心数据格式，3D柱状图只是展示层，可以替换为热力图或折线图。
- 不要向仓库提交 SSH 私钥、访问令牌、密码或真实客服隐私数据。
