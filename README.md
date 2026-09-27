# association-buckets

一次遍历客服工单，统计两个固定参数的二维关联桶，并输出与展示框架无关的 JSON 矩阵。

项目当前状态、设计约定和部署信息见 [PROJECT_NOTES.md](PROJECT_NOTES.md)。

## 运行

```bash
cargo run -- examples/tickets.json examples/params.json result.json
```

程序同时将结果打印到终端。第三个参数省略时默认写入 `result.json`。

## 参数配置

每次必须配置 `left` 和 `right` 两个维度：

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

目前支持三种维度类型：

- `time`：必须指定 `mode`。
- `category`：固定输出示例数据中的6个业务类别以及“其他”。
- `priority`：固定输出“高、中、低、其他”。

时间模式：

- `hour_of_day`：一天中的 00:00～23:00，共24桶。
- `last_30_days`：以输入数据的最新日期为终点，向前30个自然日。
- `last_24_months`：以输入数据的最新月份为终点，向前24个月。

未识别或缺失的 category、priority 值进入“其他”桶。时间字段缺失或格式不是
`YYYY-MM-DD HH:MM` 时会返回错误。

## 输出

```json
{
  "left": {
    "field": "category",
    "buckets": ["退款退货", "物流查询", "商品咨询", "账号问题", "支付问题", "投诉", "其他"]
  },
  "right": {
    "field": "priority",
    "buckets": ["高", "中", "低", "其他"]
  },
  "input_records": 50,
  "included_records": 50,
  "matrix": [
    [2, 3, 0, 0],
    [4, 6, 1, 0],
    [3, 2, 0, 0],
    [12, 10, 7, 0]
  ]
}
```

`matrix[i][j]` 表示左侧第 `i` 个桶与右侧第 `j` 个桶共同出现的次数。
该格式可以直接转换为 HTML 表格、ECharts 热力图或其他前端图表数据。

`input_records` 是输入工单总数；`included_records` 是最终时间窗口内进入矩阵的数量。
二者不同表示有记录位于最近30天或24个月的窗口之外。

## 三维演示与 GitHub Pages

`web/index.html` 使用 ECharts 5 和 ECharts-GL 直接读取 `web/result.json`，不需要 npm、
前端框架或打包步骤。页面支持拖动旋转、滚轮缩放和悬停查看组合数量。

本地预览时先生成数据，再启动一个静态文件服务器：

```bash
cargo run -- examples/tickets.json examples/params.json web/result.json
python3 -m http.server 8000 --directory web
```

访问 `http://localhost:8000`。不要直接双击 HTML，因为浏览器通常不允许页面通过
`file://` 读取旁边的 JSON 文件。

仓库包含 `.github/workflows/pages.yml`。推送到 GitHub 的 `main` 分支后，在仓库
`Settings → Pages → Build and deployment` 中将来源设为 `GitHub Actions`。工作流会：

1. 运行全部 Rust 测试；
2. 根据示例配置生成 `web/result.json`；
3. 将 `web` 目录发布到 GitHub Pages。

演示依赖 jsDelivr CDN。如果需要完全离线，可将两个 JavaScript 文件下载到 `web/vendor`
并改为相对路径。

## 更多配置示例

- `params_category_priority.json`：category × priority。
- `params_priority_category.json`：priority × category，用于镜像验证。
- `params_hour_category.json`：一天24小时 × category。
- `params_category_hour.json`：category × 一天24小时，用于镜像验证。
- `params_month_priority.json`：最近24个月 × priority。

## 实现方式

每种参数由统一的 `Dimension` 接口处理。`Analyzer` 注入两个维度处理器，在一次工单
遍历中生成稀疏计数。遍历完成后，时间维度根据期间观察到的最新时间确定窗口，再将
稀疏计数转换为固定二维矩阵，不会重新遍历原始工单。
