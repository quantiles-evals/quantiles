use anyhow::{Context, Result};
use arrow::array::{ArrayRef, BooleanBuilder, Float64Builder, Int64Builder, StringBuilder};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use parquet::arrow::ArrowWriter;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeSet, HashMap};
use std::fs::File;
use std::path::Path;
use std::sync::Arc;

/// Arrow field metadata key marking values that were serialized as JSON strings.
const JSON_ENCODED_METADATA_KEY: &str = "quantiles.json_encoded";
/// Version included in cache keys so incompatible on-disk formats use separate entries.
const CACHE_FORMAT_VERSION: &str = "v2";

/// Manages the on-disk cache for dataset batches.
pub struct DatasetCache {
    root: std::path::PathBuf,
}

impl DatasetCache {
    /// Create a cache rooted at `root`.
    #[must_use]
    pub fn new(root: std::path::PathBuf) -> Self {
        Self { root }
    }

    /// Return the filesystem path for a specific batch.
    #[must_use]
    pub fn batch_path(&self, cache_key: &str, offset: usize, limit: usize) -> std::path::PathBuf {
        self.root
            .join(cache_key)
            .join(format!("{offset}_{limit}.parquet"))
    }

    /// Write a vector of JSON objects to a Parquet file.
    ///
    /// # Errors
    ///
    /// Returns an error on I/O failure or Arrow conversion failure.
    pub async fn write_batch(&self, path: &Path, rows: &[Value]) -> Result<()> {
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        if rows.is_empty() {
            tokio::fs::write(path, &[]).await?;
            return Ok(());
        }

        let batch = json_to_arrow(rows)?;

        let file = File::create(path).context("failed to create parquet file")?;
        let mut writer = ArrowWriter::try_new(file, batch.schema(), None)?;
        writer.write(&batch)?;
        writer.close()?;
        Ok(())
    }

    /// Read a Parquet file back into a vector of JSON objects.
    ///
    /// # Errors
    ///
    /// Returns an error on I/O failure, Parquet parsing failure, or
    /// unsupported Arrow type conversion.
    pub async fn read_batch(&self, path: &Path) -> Result<Vec<Value>> {
        let metadata = tokio::fs::metadata(path).await?;
        if metadata.len() == 0 {
            return Ok(Vec::new());
        }

        let file = File::open(path).context("failed to open parquet file")?;
        let builder = ParquetRecordBatchReaderBuilder::try_new(file)?;
        let reader = builder.build()?;

        let mut rows = Vec::new();
        for batch in reader {
            let batch = batch?;
            let schema = batch.schema();
            let num_rows = batch.num_rows();

            for row_idx in 0..num_rows {
                let mut obj = serde_json::Map::new();
                for (col_idx, field) in schema.fields().iter().enumerate() {
                    let col = batch.column(col_idx);
                    let val = arrow_value_to_json(col, field, row_idx)?;
                    obj.insert(field.name().clone(), val);
                }
                rows.push(Value::Object(obj));
            }
        }
        Ok(rows)
    }
}

/// Build a deterministic cache key from dataset coordinates.
#[must_use]
pub fn cache_key(dataset_id: &str, config: &str, split: &str, revision: Option<&str>) -> String {
    let mut hasher = Sha256::new();
    hasher.update(CACHE_FORMAT_VERSION.as_bytes());
    hasher.update(b":");
    hasher.update(dataset_id.as_bytes());
    hasher.update(b":");
    hasher.update(config.as_bytes());
    hasher.update(b":");
    hasher.update(split.as_bytes());
    if let Some(rev) = revision {
        hasher.update(b":");
        hasher.update(rev.as_bytes());
    }
    format!("{:x}", hasher.finalize())
}

fn json_to_arrow(rows: &[Value]) -> Result<RecordBatch> {
    let first = rows.first().context("empty batch")?;
    let _first_obj = first.as_object().context("expected JSON object row")?;

    // Union of all keys across rows (some may be missing in some rows).
    let mut keys: BTreeSet<String> = BTreeSet::new();
    for row in rows {
        if let Some(obj) = row.as_object() {
            keys.extend(obj.keys().cloned());
        }
    }

    let mut fields = Vec::new();
    let mut arrays: Vec<ArrayRef> = Vec::new();

    for key in keys {
        let values: Vec<&Value> = rows
            .iter()
            .map(|r| r.get(&key).unwrap_or(&Value::Null))
            .collect();
        let dtype = infer_type(&values);
        let json_encoded = values
            .iter()
            .any(|value| matches!(value, Value::Array(_) | Value::Object(_)));
        let mut field = Field::new(key.clone(), dtype.clone(), true);
        if json_encoded {
            field = field.with_metadata(HashMap::from([(
                JSON_ENCODED_METADATA_KEY.to_owned(),
                "true".to_owned(),
            )]));
        }
        fields.push(field);
        arrays.push(build_array(&values, &dtype, json_encoded)?);
    }

    let schema = Arc::new(Schema::new(fields));
    Ok(RecordBatch::try_new(schema, arrays)?)
}

fn infer_type(values: &[&Value]) -> DataType {
    let mut has_string = false;
    let mut has_bool = false;
    let mut has_int = false;
    let mut has_float = false;
    let mut has_complex = false;

    for v in values {
        match v {
            Value::Null => {}
            Value::Bool(_) => has_bool = true,
            Value::Number(n) => {
                if n.is_i64() {
                    has_int = true;
                } else {
                    has_float = true;
                }
            }
            Value::String(_) => has_string = true,
            Value::Array(_) | Value::Object(_) => has_complex = true,
        }
    }

    if has_complex || has_string {
        DataType::Utf8
    } else if has_bool && !has_int && !has_float {
        DataType::Boolean
    } else if has_int && !has_float && !has_bool {
        DataType::Int64
    } else if has_float || (has_int && has_bool) {
        DataType::Float64
    } else {
        DataType::Utf8
    }
}

fn build_array(values: &[&Value], dtype: &DataType, json_encoded: bool) -> Result<ArrayRef> {
    match dtype {
        DataType::Boolean => {
            let mut builder = BooleanBuilder::new();
            for v in values {
                match v {
                    Value::Bool(b) => builder.append_value(*b),
                    _ => builder.append_null(),
                }
            }
            Ok(Arc::new(builder.finish()))
        }
        DataType::Int64 => {
            let mut builder = Int64Builder::new();
            for v in values {
                match v {
                    Value::Number(n) => builder.append_value(n.as_i64().unwrap_or(0)),
                    _ => builder.append_null(),
                }
            }
            Ok(Arc::new(builder.finish()))
        }
        DataType::Float64 => {
            let mut builder = Float64Builder::new();
            for v in values {
                match v {
                    Value::Number(n) => builder.append_value(n.as_f64().unwrap_or(0.0)),
                    _ => builder.append_null(),
                }
            }
            Ok(Arc::new(builder.finish()))
        }
        DataType::Utf8 => {
            let mut builder = StringBuilder::new();
            for v in values {
                match v {
                    Value::Null => builder.append_null(),
                    value if json_encoded => builder.append_value(serde_json::to_string(value)?),
                    Value::String(s) => builder.append_value(s),
                    other => builder.append_value(other.to_string()),
                }
            }
            Ok(Arc::new(builder.finish()))
        }
        _ => anyhow::bail!("unsupported Arrow type: {dtype:?}"),
    }
}

fn arrow_value_to_json(col: &dyn arrow::array::Array, field: &Field, row: usize) -> Result<Value> {
    use arrow::array::{BooleanArray, Float64Array, Int64Array, StringArray};

    if col.is_null(row) {
        return Ok(Value::Null);
    }

    if let Some(arr) = col.as_any().downcast_ref::<StringArray>() {
        let value = arr.value(row);
        if field
            .metadata()
            .get(JSON_ENCODED_METADATA_KEY)
            .is_some_and(|encoded| encoded == "true")
        {
            serde_json::from_str(value).context("failed to decode cached JSON column")
        } else {
            Ok(Value::String(value.to_string()))
        }
    } else if let Some(arr) = col.as_any().downcast_ref::<Int64Array>() {
        let n: i64 = arr.value(row);
        Ok(Value::Number(n.into()))
    } else if let Some(arr) = col.as_any().downcast_ref::<Float64Array>() {
        let n = arr.value(row);
        serde_json::Number::from_f64(n).map_or_else(|| Ok(Value::Null), |n| Ok(Value::Number(n)))
    } else if let Some(arr) = col.as_any().downcast_ref::<BooleanArray>() {
        Ok(Value::Bool(arr.value(row)))
    } else {
        anyhow::bail!("unsupported Arrow array type for JSON conversion")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_cache_key_is_deterministic() {
        let k1 = cache_key("quantiles/PubMedQA", "pqa_labeled", "test", Some("main"));
        let k2 = cache_key("quantiles/PubMedQA", "pqa_labeled", "test", Some("main"));
        assert_eq!(k1, k2);
        assert_eq!(k1.len(), 64); // sha256 hex
    }

    #[test]
    fn test_cache_key_differs_by_revision() {
        let k1 = cache_key("ds", "cfg", "split", Some("v1"));
        let k2 = cache_key("ds", "cfg", "split", Some("v2"));
        assert_ne!(k1, k2);
    }

    #[test]
    fn test_cache_key_without_revision() {
        let k = cache_key("ds", "cfg", "split", None);
        assert_eq!(k.len(), 64);
    }

    #[tokio::test]
    async fn test_write_and_read_batch_roundtrip() {
        let tmpdir = tempfile::tempdir().unwrap();
        let cache = DatasetCache::new(tmpdir.path().to_path_buf());
        let path = cache.batch_path("abc123", 0, 100);

        let rows = vec![
            json!({"id": 1, "name": "alice", "score": 3.15, "active": true}),
            json!({"id": 2, "name": "bob", "score": 2.71, "active": false}),
            json!({"id": 3, "name": "charlie", "score": 1.41, "active": true}),
        ];

        cache.write_batch(&path, &rows).await.unwrap();
        let read = cache.read_batch(&path).await.unwrap();

        assert_eq!(read.len(), 3);
        assert_eq!(read[0]["id"], 1);
        assert_eq!(read[0]["name"], "alice");
        assert_eq!(read[0]["score"], 3.15);
        assert_eq!(read[0]["active"], true);
        assert_eq!(read[1]["name"], "bob");
        assert_eq!(read[2]["name"], "charlie");
    }

    #[tokio::test]
    async fn test_nested_json_roundtrip() {
        let tmpdir = tempfile::tempdir().unwrap();
        let cache = DatasetCache::new(tmpdir.path().to_path_buf());
        let path = cache.batch_path("nested", 0, 10);
        let rows = vec![
            json!({
                "options": {"A": "alpha", "B": "beta"},
                "list": ["one", "two"]
            }),
            json!({"options": "unstructured", "list": ["three"]}),
        ];

        cache.write_batch(&path, &rows).await.unwrap();
        let read = cache.read_batch(&path).await.unwrap();

        assert_eq!(read, rows);
    }

    #[tokio::test]
    async fn test_empty_batch_roundtrip() {
        let tmpdir = tempfile::tempdir().unwrap();
        let cache = DatasetCache::new(tmpdir.path().to_path_buf());
        let path = cache.batch_path("empty", 0, 10);

        cache.write_batch(&path, &[]).await.unwrap();
        let read = cache.read_batch(&path).await.unwrap();

        assert!(read.is_empty());
    }

    #[tokio::test]
    async fn test_missing_fields_survive_roundtrip() {
        let tmpdir = tempfile::tempdir().unwrap();
        let cache = DatasetCache::new(tmpdir.path().to_path_buf());
        let path = cache.batch_path("sparse", 0, 10);

        let rows = vec![
            json!({"id": 1, "name": "alice"}),
            json!({"id": 2, "age": 30}), // missing "name"
            json!({"id": 3, "name": "bob", "age": 25}),
        ];

        cache.write_batch(&path, &rows).await.unwrap();
        let read = cache.read_batch(&path).await.unwrap();

        assert_eq!(read.len(), 3);
        assert_eq!(read[0]["id"], 1);
        assert_eq!(read[1]["name"], Value::Null);
        assert_eq!(read[2]["age"], 25);
    }

    #[test]
    fn test_infer_type_bool_only() {
        let values = vec![&Value::Bool(true), &Value::Bool(false)];
        assert_eq!(infer_type(&values), DataType::Boolean);
    }

    #[test]
    fn test_infer_type_int_only() {
        let v1 = json!(42);
        let v2 = json!(7);
        let values = vec![&v1, &v2];
        assert_eq!(infer_type(&values), DataType::Int64);
    }

    #[test]
    fn test_infer_type_float_only() {
        let v1 = json!(3.15);
        let v2 = json!(2.71);
        let values = vec![&v1, &v2];
        assert_eq!(infer_type(&values), DataType::Float64);
    }

    #[test]
    fn test_infer_type_mixed_int_float() {
        let v1 = json!(42);
        let v2 = json!(3.15);
        let values = vec![&v1, &v2];
        assert_eq!(infer_type(&values), DataType::Float64);
    }

    #[test]
    fn test_infer_type_string() {
        let v1 = json!("hello");
        let v2 = json!(42);
        let values = vec![&v1, &v2];
        assert_eq!(infer_type(&values), DataType::Utf8);
    }

    #[test]
    fn test_infer_type_null_skipped() {
        let v = json!(42);
        let values = vec![&Value::Null, &v];
        assert_eq!(infer_type(&values), DataType::Int64);
    }
}
