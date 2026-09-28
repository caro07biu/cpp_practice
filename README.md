# association-buckets

## 项目理解

无论是在观察“关联”还是“趋势”，本质上都是在理解两个变量之间的关系。变量可以是
时间，也可以是工单类别、优先级等业务类型；本项目将时间与类型统一抽象为可配置的
“参数”，因此可以任意选择两个参数进行组合、统计和可视化对比。

客服工作中存在大量难以直接写入规则的外部知识，例如某类问题通常在哪个时段集中出现、
某种优先级经常与哪些业务类别同时出现。这些隐性知识往往保存在客服人员的经验中。
三维柱状图把两个参数及其组合数量同时呈现出来，让客服能够直观地发现分布、趋势和可能的
联系；底层的结构化 JSON 矩阵则是同一信息的数学化表达，既方便客服查看，也方便 Agent
进一步读取、比较和提取信息。

本项目是一道“联系题”：它不替人判断两个变量是否存在因果关系，而是把可观察的联系整理成
稳定、可复用的数据结构，为人工分析和 Agent 推理提供共同基础。

一次遍历客服工单，统计两个固定参数的二维关联桶，并输出与展示框架无关的 JSON 矩阵。

项目当前状态、设计约定和部署信息见 [PROJECT_NOTES.md](PROJECT_NOTES.md)。

## 分析维度设计

分析器把每个变量实现为统一的 `Dimension`：时间维度负责解析时间并生成连续时间桶，
类别和优先级维度负责把业务值映射到固定桶，未知值统一进入“其他”。运行时通过 `left`
和 `right` 任意组合两个维度，先在一次工单遍历中生成稀疏计数，再转换为
`matrix[左侧桶][右侧桶]`。这种设计让统计逻辑与三维图表解耦，也便于继续增加新的维度。

## 关键发现

以下结论只针对仓库中的50条演示工单，用于展示如何从矩阵读取信息，不代表真实业务总体：

- “类别 × 优先级”中，“支付问题 × 高优先级”为14条，是数量最多的组合；其次是
  “退款退货 × 高优先级”7条。
- “最近30天 × 类别”中，2024-06-11的“支付问题”为3条，是单日单类别最高值；支付问题
  也在6月6日至11日多次出现，形成可继续调查的局部集中现象。
- 矩阵只能揭示样例数据中的分布和联系，不能单独证明因果关系；实际判断仍需结合客服掌握的
  业务背景和外部知识。

## AI 工具使用情况

开发过程中使用 Codex Agent 协助梳理需求、实现 Rust 统计逻辑与静态展示页面、补充测试和
文档，并通过终端执行 `cargo test`、生成演示 JSON 和检查 Git 差异。参数语义、展示方式及
最终结论由开发者确认；AI 生成内容均通过自动化测试或实际运行结果复核。

## 在线地址

- GitHub 仓库：https://github.com/caro07biu/cpp_practice
- 在线演示：https://caro07biu.github.io/cpp_practice/

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

`web/index.html` 提供两个预设演示入口，选择后由 `web/chart.html` 使用 ECharts 5 和
ECharts-GL 读取相应的预生成 JSON，不需要 npm、前端框架或打包步骤。图表支持拖动旋转、
滚轮缩放和悬停查看组合数量。

本地预览时先生成数据，再启动一个静态文件服务器：

```bash
cargo run -- examples/tickets.json examples/params.json web/data/time-category.json
cargo run -- examples/tickets.json examples/params_category_priority.json web/data/category-priority.json
python3 -m http.server 8000 --directory web
```

访问 `http://localhost:8000`。不要直接双击 HTML，因为浏览器通常不允许页面通过
`file://` 读取旁边的 JSON 文件。

仓库包含 `.github/workflows/pages.yml`。推送到 GitHub 的 `main` 分支后，在仓库
`Settings → Pages → Build and deployment` 中将来源设为 `GitHub Actions`。工作流会：

1. 运行全部 Rust 测试；
2. 根据两个示例配置生成对应的演示 JSON；
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
