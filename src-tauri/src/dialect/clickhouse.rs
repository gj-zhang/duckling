use std::str;
use std::sync::Arc;

use arrow::array::*;
use arrow::datatypes::*;
use chrono::naive::NaiveDate;
use chrono::prelude::*;
use chrono::DateTime;
use chrono_tz::Tz;
use clickhouse_rs::types::{Decimal, FromSql, SqlType};
use clickhouse_rs::{types::column::Column, Block, ClientHandle, Pool, Simple};
use duckdb::polars::export::chrono;
use futures_core::stream::Stream;
use futures_util::stream::StreamExt;
use serde::{Deserialize, Serialize};
use tauri::{Manager, Window};

use crate::api::{serialize_preview, ArrowData};
use crate::dialect::{Dialect, TreeNode};
use crate::utils::{build_tree, Table};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ClickhouseDialect {
  pub host: String,
  pub port: String,
  pub username: String,
  pub password: String,
}

impl ClickhouseDialect {
  pub(crate) fn get_url(&self) -> String {
    format!(
      "tcp://{}:{}@{}:{}/temp_database_lxn?compression=lz4&ping_timeout=42ms",
      self.username, self.password, self.host, self.port,
    )
  }
}

impl Dialect for ClickhouseDialect {
  async fn get_db(&self) -> Option<TreeNode> {
    let url = self.get_url();
    if let Ok(tables) = get_tables(url).await {
      Some(TreeNode {
        name: self.host.clone(),
        path: self.host.clone(),
        node_type: "root".to_string(),
        children: Some(build_tree(tables)),
      })
    } else {
      None
    }
  }
}

impl ClickhouseDialect {
  fn get_schema(&self) -> Vec<Table> {
    vec![]
  }

  // pub async fn query(&self, sql: &str) -> anyhow::Result<ArrowData> {
  //   let pool = Pool::new(self.get_url());
  //   let mut client = pool.get_handle().await?;
  //
  //   let block = client.query(sql).fetch_all().await?;
  //
  //   let batch = block_to_arrow(&block)?;
  //
  //   Ok(ArrowData {
  //     total_count: batch.num_rows(),
  //     preview: serialize_preview(&batch)?,
  //   })
  // }

  pub async fn query_stream(&self, window: Window, sql: &str) -> anyhow::Result<()> {
    let pool = Pool::new(self.get_url());
    let mut client = pool.get_handle().await?;
    let mut stream = client.query(sql).stream_blocks();

    while let Some(block) = stream.next().await {
      let block = block?;
      let batch = block_to_arrow(&block)?;
      window
        .emit(
          "query-stream",
          ArrowData {
            total_count: batch.num_rows(),
            preview: serialize_preview(&batch)?,
          },
        )
        .unwrap();
    }
    Ok(())
  }

  pub async fn query_block(
    &self,
    sql: &str,
    limit: usize,
    offset: usize,
  ) -> anyhow::Result<ArrowData> {
    let pool = Pool::new(self.get_url());
    let mut client = pool.get_handle().await?;
    let mut stream = client.query(sql).stream_blocks();

    let mut batchs = vec![];

    let total = 0;
    while let Some(block) = stream.next().await {
      let block = block?;
      let current_count = block.row_count();
      if total + current_count < limit * offset {
        continue;
      }
      let batch = block_to_arrow(&block)?;
      batchs.push(batch);
    }
    let b = batchs[0].clone();
    let schema = b.schema();
    let batch = arrow::compute::concat_batches(&schema, &batchs)?;
    Ok(ArrowData {
      total_count: batch.num_rows(),
      preview: serialize_preview(&batch)?,
    })
  }
  pub async fn query(&self, sql: &str) -> anyhow::Result<ArrowData> {
    let pool = Pool::new(self.get_url());
    let mut client = pool.get_handle().await?;
    let mut stream = client.query(sql).stream_blocks();

    let mut batchs = vec![];
    while let Some(block) = stream.next().await {
      let block = block?;
      let batch = block_to_arrow(&block)?;
      batchs.push(batch);
    }
    let b = batchs[0].clone();
    let schema = b.schema();
    let batch = arrow::compute::concat_batches(&schema, &batchs)?;
    Ok(ArrowData {
      total_count: batch.num_rows(),
      preview: serialize_preview(&batch)?,
    })
  }
}

fn convert_type(col_type: &SqlType) -> DataType {
  match col_type {
    SqlType::Bool => DataType::Boolean,
    SqlType::UInt8 => DataType::UInt8,
    SqlType::UInt16 => DataType::UInt16,
    SqlType::UInt32 => DataType::UInt32,
    SqlType::UInt64 => DataType::UInt64,
    SqlType::Int8 => DataType::Int8,
    SqlType::Int16 => DataType::Int16,
    SqlType::Int32 => DataType::Int32,
    SqlType::Int64 => DataType::Int64,
    SqlType::Float32 => DataType::Float32,
    SqlType::Float64 => DataType::Float64,
    SqlType::Date => DataType::Date32,
    SqlType::String => DataType::Utf8,
    SqlType::DateTime(_) => DataType::Date64,
    SqlType::Nullable(t) => convert_type(t.clone()),
    SqlType::Decimal(d1, d2) => DataType::Utf8,
    SqlType::Array(t) => DataType::List(Arc::new(Field::new("", convert_type(t), false))),
    _ => DataType::Utf8,
  }
}

macro_rules! create_array {
  ($col:expr, $ty:ty, $nav:ty) => {
    Arc::new(<$ty>::from(
      $col
        .iter::<$nav>()?
        .collect::<Vec<_>>()
        .into_iter()
        .copied()
        .collect::<Vec<_>>(),
    )) as ArrayRef
  };
}

macro_rules! generate_array {
  ($block:expr, $col:expr, $ty:ty, $nav:ty, $nullable:expr) => {
    if $nullable {
      Arc::new(<$ty>::from(collect_block::<Option<$nav>>(
        $block,
        $col.name(),
      ))) as ArrayRef
    } else {
      Arc::new(<$ty>::from(collect_block::<$nav>($block, $col.name()))) as ArrayRef
    }
  };
}

fn collect_block<'b, T: FromSql<'b>>(block: &'b Block, column: &str) -> Vec<T> {
  (0..block.row_count())
    .map(|i| block.get(i, column).unwrap())
    .collect()
}

fn date_to_days(t: &NaiveDate) -> i32 {
  t.signed_duration_since(NaiveDate::from_ymd_opt(1970, 1, 1).unwrap())
    .num_days() as i32
}

fn convert_col(
  block: &Block,
  col_type: &SqlType,
  col: &Column<Simple>,
) -> anyhow::Result<(Field, ArrayRef)> {
  let nullable = matches!(col_type, SqlType::Nullable(_));
  let typ = if let SqlType::Nullable(t) = col_type {
    t.clone()
  } else {
    col_type
  };
  let field = Field::new(col.name(), convert_type(typ), nullable);
  let arr: ArrayRef = match typ {
    SqlType::UInt8 => {
      if nullable {
        Arc::new(UInt8Array::from(collect_block::<Option<u8>>(
          block,
          col.name(),
        ))) as ArrayRef
      } else {
        Arc::new(UInt8Array::from(collect_block::<u8>(block, col.name()))) as ArrayRef
      }
    }
    SqlType::UInt16 => generate_array!(block, col, UInt16Array, u16, nullable),
    SqlType::UInt32 => generate_array!(block, col, UInt32Array, u32, nullable),
    SqlType::UInt64 => generate_array!(block, col, UInt64Array, u64, nullable),
    SqlType::Int8 => generate_array!(block, col, Int8Array, i8, nullable),
    SqlType::Int16 => generate_array!(block, col, Int16Array, i16, nullable),
    SqlType::Int32 => generate_array!(block, col, Int32Array, i32, nullable),
    SqlType::Int64 => generate_array!(block, col, Int64Array, i64, nullable),
    SqlType::Float32 => generate_array!(block, col, Float32Array, f32, nullable),
    SqlType::Float64 => generate_array!(block, col, Float64Array, f64, nullable),
    SqlType::Date => {
      if nullable {
        let res: Vec<_> = collect_block::<Option<NaiveDate>>(block, col.name())
          .iter()
          .map(|tt| tt.as_ref().map(date_to_days))
          .collect::<Vec<Option<i32>>>();
        Arc::new(Date32Array::from(res)) as ArrayRef
      } else {
        let res: Vec<_> = collect_block::<NaiveDate>(block, col.name())
          .iter()
          .map(date_to_days)
          .collect::<Vec<i32>>();
        Arc::new(Date32Array::from(res)) as ArrayRef
      }
    }
    SqlType::DateTime(_) => {
      if nullable {
        let res = collect_block::<Option<DateTime<Tz>>>(block, col.name());
        let res = res
          .iter()
          .map(|t| t.map(|i| i.timestamp() * 1000))
          .collect::<Vec<Option<i64>>>();
        Arc::new(Date64Array::from(res)) as ArrayRef
      } else {
        let res = collect_block::<DateTime<Tz>>(block, col.name());
        let res = res
          .iter()
          .map(|t| t.timestamp() * 1000)
          .collect::<Vec<i64>>();
        Arc::new(Date64Array::from(res)) as ArrayRef
      }
    }
    SqlType::Decimal(_d1, _d2) => {
      if nullable {
        Arc::new(StringArray::from(
          col
            .iter::<Option<Decimal>>()?
            .collect::<Vec<_>>()
            .into_iter()
            .map(|t| t.map(|i| format!("{i}")))
            .clone()
            .collect::<Vec<_>>(),
        )) as ArrayRef
      } else {
        Arc::new(StringArray::from(
          col
            .iter::<Decimal>()?
            .collect::<Vec<_>>()
            .into_iter()
            .map(|t| format!("{t}"))
            .clone()
            .collect::<Vec<_>>(),
        )) as ArrayRef
      }
    }
    _ => {
      let strings: Vec<_> = if nullable {
        col
          .iter::<Option<&[u8]>>()?
          .filter_map(|s| {
            if let Some(b) = s {
              str::from_utf8(b).ok()
            } else {
              None
            }
          })
          .collect()
      } else {
        col
          .iter::<&[u8]>()?
          .filter_map(|s| std::str::from_utf8(s).ok())
          .collect()
      };
      Arc::new(StringArray::from(strings)) as ArrayRef
    }
  };
  Ok((field, arr))
}
fn block_to_arrow(block: &Block) -> anyhow::Result<RecordBatch> {
  let mut fields = vec![];
  let mut data = vec![];
  for col in block.columns() {
    if let Ok((field, arr)) = convert_col(block, &col.sql_type(), col) {
      fields.push(field);
      data.push(arr);
    }
  }

  let schema = Schema::new(fields);
  let batch = RecordBatch::try_new(Arc::new(schema), data)?;
  Ok(batch)
}

async fn query_stream(url: &str, sql: &str) -> anyhow::Result<()> {
  let pool = Pool::new(url);
  let mut client = pool.get_handle().await?;

  let mut stream = client.query(sql).stream_blocks();
  while let Some(block) = stream.next().await {
    let block = block?;

    let columns = block.columns();
    for col in columns {
      println!("name: {:?}, sql_type: {:?}", col.name(), col.sql_type());
    }
  }
  Ok(())
}

async fn get_tables(url: String) -> anyhow::Result<Vec<Table>> {
  let sql = r#"
  select database as table_schema, name as table_name, engine as table_type
  from system.tables order by table_schema, table_type
  "#;
  let pool = Pool::new(url);
  let mut client = pool.get_handle().await?;

  let block = client.query(sql).fetch_all().await?;
  let mut tables = Vec::new();
  for row in block.rows() {
    let table_schema: String = row.get("table_schema")?;
    let table_name: String = row.get("table_name")?;
    let table_type: String = row.get("table_type")?;

    tables.push(Table {
      table_schema: table_schema.clone(),
      table_name,
      table_type: table_type.clone(),
      r#type: String::from(if table_type == "View" {
        "view"
      } else {
        "table"
      }),
    });
  }
  Ok(tables)
}
