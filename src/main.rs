use std::{collections::HashMap, env, fs, hash::Hash};

use anyhow::{Context, Result, bail};
use chrono::{Datelike, NaiveDateTime, Timelike};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Deserialize)]
struct Config {
    left: DimensionConfig,
    right: DimensionConfig,
}

#[derive(Debug, Deserialize)]
struct DimensionConfig {
    field: String,
    #[serde(rename = "type")]
    kind: DimensionType,
    mode: Option<TimeMode>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum DimensionType {
    Time,
    Category,
    Priority,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum TimeMode {
    HourOfDay,
    #[serde(rename = "last_30_days")]
    Last30Days,
    #[serde(rename = "last_24_months")]
    Last24Months,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum BucketKey {
    Number(i64),
    Name(String),
}

#[derive(Clone, Debug)]
struct Bucket {
    key: BucketKey,
    label: String,
}

trait Dimension {
    fn field(&self) -> &str;
    fn observe(&mut self, ticket: &Value) -> Result<BucketKey>;
    fn buckets(&self) -> Result<Vec<Bucket>>;
}

struct NamedDimension {
    field: String,
    kind: NamedKind,
}

enum NamedKind {
    Category,
    Priority,
}

impl Dimension for NamedDimension {
    fn field(&self) -> &str {
        &self.field
    }

    fn observe(&mut self, ticket: &Value) -> Result<BucketKey> {
        let value = ticket.get(&self.field).and_then(Value::as_str);
        let name = match self.kind {
            NamedKind::Category => match value {
                Some("退款退货") => "退款退货",
                Some("物流查询") => "物流查询",
                Some("商品咨询") => "商品咨询",
                Some("账号问题") => "账号问题",
                Some("支付问题") => "支付问题",
                Some("投诉") => "投诉",
                _ => "其他",
            },
            NamedKind::Priority => match value {
                Some("高") => "高",
                Some("中") => "中",
                Some("低") => "低",
                _ => "其他",
            },
        };
        Ok(BucketKey::Name(name.to_owned()))
    }

    fn buckets(&self) -> Result<Vec<Bucket>> {
        let names: &[&str] = match self.kind {
            NamedKind::Category => &[
                "退款退货",
                "物流查询",
                "商品咨询",
                "账号问题",
                "支付问题",
                "投诉",
                "其他",
            ],
            NamedKind::Priority => &["高", "中", "低", "其他"],
        };
        Ok(names
            .iter()
            .map(|name| Bucket {
                key: BucketKey::Name((*name).to_owned()),
                label: (*name).to_owned(),
            })
            .collect())
    }
}

struct TimeDimension {
    field: String,
    mode: TimeMode,
    latest: Option<i64>,
}

impl TimeDimension {
    fn key_and_label(&self, time: NaiveDateTime) -> (i64, String) {
        match self.mode {
            TimeMode::HourOfDay => (i64::from(time.hour()), format!("{:02}:00", time.hour())),
            TimeMode::Last30Days => {
                let key = i64::from(time.date().num_days_from_ce());
                (key, time.date().format("%Y-%m-%d").to_string())
            }
            TimeMode::Last24Months => {
                let key = i64::from(time.year()) * 12 + i64::from(time.month0());
                (key, time.format("%Y-%m").to_string())
            }
        }
    }

    fn label_for_key(&self, key: i64) -> Result<String> {
        match self.mode {
            TimeMode::HourOfDay => Ok(format!("{key:02}:00")),
            TimeMode::Last30Days => {
                let days = i32::try_from(key).context("日期超出支持范围")?;
                let date = chrono::NaiveDate::from_num_days_from_ce_opt(days)
                    .context("日期超出支持范围")?;
                Ok(date.format("%Y-%m-%d").to_string())
            }
            TimeMode::Last24Months => {
                let year = i32::try_from(key.div_euclid(12)).context("月份超出支持范围")?;
                let month = key.rem_euclid(12) + 1;
                Ok(format!("{year:04}-{month:02}"))
            }
        }
    }
}

impl Dimension for TimeDimension {
    fn field(&self) -> &str {
        &self.field
    }

    fn observe(&mut self, ticket: &Value) -> Result<BucketKey> {
        let raw = ticket
            .get(&self.field)
            .and_then(Value::as_str)
            .with_context(|| format!("字段 {} 缺失或不是字符串", self.field))?;
        let time = NaiveDateTime::parse_from_str(raw, "%Y-%m-%d %H:%M")
            .with_context(|| format!("字段 {} 的时间格式错误: {raw}", self.field))?;
        let (key, _) = self.key_and_label(time);
        self.latest = Some(self.latest.map_or(key, |latest| latest.max(key)));
        Ok(BucketKey::Number(key))
    }

    fn buckets(&self) -> Result<Vec<Bucket>> {
        let keys: Vec<i64> = match self.mode {
            TimeMode::HourOfDay => (0..24).collect(),
            TimeMode::Last30Days => {
                let Some(latest) = self.latest else {
                    return Ok(Vec::new());
                };
                ((latest - 29)..=latest).collect()
            }
            TimeMode::Last24Months => {
                let Some(latest) = self.latest else {
                    return Ok(Vec::new());
                };
                ((latest - 23)..=latest).collect()
            }
        };
        keys.into_iter()
            .map(|key| {
                Ok(Bucket {
                    key: BucketKey::Number(key),
                    label: self.label_for_key(key)?,
                })
            })
            .collect()
    }
}

fn create_dimension(config: DimensionConfig) -> Result<Box<dyn Dimension>> {
    if config.field.trim().is_empty() {
        bail!("维度 field 不能为空");
    }
    match config.kind {
        DimensionType::Time => Ok(Box::new(TimeDimension {
            field: config.field,
            mode: config.mode.context("time 类型必须提供 mode")?,
            latest: None,
        })),
        DimensionType::Category => {
            if config.mode.is_some() {
                bail!("category 类型不能提供 mode");
            }
            Ok(Box::new(NamedDimension {
                field: config.field,
                kind: NamedKind::Category,
            }))
        }
        DimensionType::Priority => {
            if config.mode.is_some() {
                bail!("priority 类型不能提供 mode");
            }
            Ok(Box::new(NamedDimension {
                field: config.field,
                kind: NamedKind::Priority,
            }))
        }
    }
}

#[derive(Debug, Serialize, PartialEq)]
struct DimensionOutput {
    field: String,
    buckets: Vec<String>,
}

#[derive(Debug, Serialize, PartialEq)]
struct AnalysisOutput {
    left: DimensionOutput,
    right: DimensionOutput,
    input_records: u64,
    included_records: u64,
    matrix: Vec<Vec<u64>>,
}

struct Analyzer {
    left: Box<dyn Dimension>,
    right: Box<dyn Dimension>,
}

impl Analyzer {
    fn new(config: Config) -> Result<Self> {
        Ok(Self {
            left: create_dimension(config.left)?,
            right: create_dimension(config.right)?,
        })
    }

    fn analyze(&mut self, tickets: &[Value]) -> Result<AnalysisOutput> {
        let mut sparse_counts: HashMap<(BucketKey, BucketKey), u64> = HashMap::new();

        // 原始工单只在这里遍历一次。时间窗口在遍历后根据 latest 裁剪稀疏计数。
        for ticket in tickets {
            let left = self.left.observe(ticket)?;
            let right = self.right.observe(ticket)?;
            *sparse_counts.entry((left, right)).or_default() += 1;
        }

        let left_buckets = self.left.buckets()?;
        let right_buckets = self.right.buckets()?;
        let mut matrix = vec![vec![0; right_buckets.len()]; left_buckets.len()];
        let mut included_records = 0;

        for (left_index, left) in left_buckets.iter().enumerate() {
            for (right_index, right) in right_buckets.iter().enumerate() {
                let count = sparse_counts
                    .get(&(left.key.clone(), right.key.clone()))
                    .copied()
                    .unwrap_or(0);
                matrix[left_index][right_index] = count;
                included_records += count;
            }
        }

        Ok(AnalysisOutput {
            left: DimensionOutput {
                field: self.left.field().to_owned(),
                buckets: left_buckets
                    .into_iter()
                    .map(|bucket| bucket.label)
                    .collect(),
            },
            right: DimensionOutput {
                field: self.right.field().to_owned(),
                buckets: right_buckets
                    .into_iter()
                    .map(|bucket| bucket.label)
                    .collect(),
            },
            input_records: tickets.len() as u64,
            included_records,
            matrix,
        })
    }
}

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    if !(3..=4).contains(&args.len()) {
        bail!(
            "用法: {} <tickets.json> <params.json> [result.json]",
            args[0]
        );
    }

    let tickets_text =
        fs::read_to_string(&args[1]).with_context(|| format!("无法读取数据文件 {}", args[1]))?;
    let params_text =
        fs::read_to_string(&args[2]).with_context(|| format!("无法读取参数文件 {}", args[2]))?;
    let tickets: Vec<Value> =
        serde_json::from_str(&tickets_text).context("数据文件不是 JSON 数组")?;
    let config: Config = serde_json::from_str(&params_text).context("参数文件格式错误")?;

    let mut analyzer = Analyzer::new(config)?;
    let output = analyzer.analyze(&tickets)?;
    let json = serde_json::to_string_pretty(&output)?;
    let output_path = args.get(3).map(String::as_str).unwrap_or("result.json");
    fs::write(output_path, format!("{json}\n"))
        .with_context(|| format!("无法写入结果文件 {output_path}"))?;

    println!("{json}");
    eprintln!("已生成关联矩阵: {output_path}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn analyze(tickets: &str, config: &str) -> AnalysisOutput {
        let tickets: Vec<Value> = serde_json::from_str(tickets).unwrap();
        let config: Config = serde_json::from_str(config).unwrap();
        Analyzer::new(config).unwrap().analyze(&tickets).unwrap()
    }

    #[test]
    fn counts_category_by_priority_and_keeps_other() {
        let output = analyze(
            r#"[
              {"category":"退款退货","priority":"高"},
              {"category":"物流查询","priority":"中"},
              {"category":"支付问题","priority":"紧急"},
              {"category":"账号问题"}
            ]"#,
            r#"{
              "left":{"field":"category","type":"category"},
              "right":{"field":"priority","type":"priority"}
            }"#,
        );

        assert_eq!(
            output.left.buckets,
            [
                "退款退货",
                "物流查询",
                "商品咨询",
                "账号问题",
                "支付问题",
                "投诉",
                "其他"
            ]
        );
        assert_eq!(output.right.buckets, ["高", "中", "低", "其他"]);
        assert_eq!(output.matrix[0], [1, 0, 0, 0]);
        assert_eq!(output.matrix[1], [0, 1, 0, 0]);
        assert_eq!(output.matrix[3], [0, 0, 0, 1]);
        assert_eq!(output.matrix[4], [0, 0, 0, 1]);
        assert_eq!(output.included_records, 4);
    }

    #[test]
    fn uses_latest_date_for_a_30_day_window() {
        let output = analyze(
            r#"[
              {"created_at":"2024-01-01 09:00","category":"退款退货"},
              {"created_at":"2024-02-01 10:00","category":"物流查询"},
              {"created_at":"2024-03-01 11:00","category":"账号问题"}
            ]"#,
            r#"{
              "left":{"field":"created_at","type":"time","mode":"last_30_days"},
              "right":{"field":"category","type":"category"}
            }"#,
        );

        assert_eq!(output.left.buckets.len(), 30);
        assert_eq!(output.left.buckets.last().unwrap(), "2024-03-01");
        assert_eq!(output.input_records, 3);
        assert_eq!(output.included_records, 2);
        assert_eq!(output.matrix[0][1], 1);
        assert_eq!(output.matrix[29][3], 1);
    }

    #[test]
    fn creates_24_hour_buckets() {
        let output = analyze(
            r#"[
              {"created_at":"2024-06-01 09:15","priority":"高"},
              {"created_at":"2024-06-02 09:45","priority":"低"}
            ]"#,
            r#"{
              "left":{"field":"created_at","type":"time","mode":"hour_of_day"},
              "right":{"field":"priority","type":"priority"}
            }"#,
        );

        assert_eq!(output.left.buckets.len(), 24);
        assert_eq!(output.matrix[9], [1, 0, 1, 0]);
    }

    #[test]
    fn creates_last_24_month_buckets() {
        let output = analyze(
            r#"[
              {"created_at":"2022-02-01 09:00","category":"退款退货"},
              {"created_at":"2024-01-01 09:00","category":"物流查询"}
            ]"#,
            r#"{
              "left":{"field":"created_at","type":"time","mode":"last_24_months"},
              "right":{"field":"category","type":"category"}
            }"#,
        );

        assert_eq!(output.left.buckets.first().unwrap(), "2022-02");
        assert_eq!(output.left.buckets.last().unwrap(), "2024-01");
        assert_eq!(output.included_records, 2);
    }

    #[test]
    fn swapping_dimensions_transposes_the_matrix() {
        let tickets = r#"[
          {"category":"退款退货","priority":"高"},
          {"category":"退款退货","priority":"中"},
          {"category":"物流查询","priority":"中"},
          {"category":"支付问题","priority":"低"}
        ]"#;
        let category_priority = analyze(
            tickets,
            r#"{
              "left":{"field":"category","type":"category"},
              "right":{"field":"priority","type":"priority"}
            }"#,
        );
        let priority_category = analyze(
            tickets,
            r#"{
              "left":{"field":"priority","type":"priority"},
              "right":{"field":"category","type":"category"}
            }"#,
        );

        for row in 0..category_priority.matrix.len() {
            for column in 0..category_priority.matrix[row].len() {
                assert_eq!(
                    category_priority.matrix[row][column],
                    priority_category.matrix[column][row]
                );
            }
        }
    }
}
