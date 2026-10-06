use serde::Serialize;
use std::io::Write;

pub const SCHEMA: &str = "sanctifier-ndjson-v1";

pub fn write_record<W: Write, T: Serialize>(
    writer: &mut W,
    record_type: &str,
    category: Option<&str>,
    data: &T,
) -> anyhow::Result<()> {
    let mut record = serde_json::json!({
        "schema": SCHEMA,
        "type": record_type,
        "data": data,
    });
    if let Some(category) = category {
        record["category"] = serde_json::Value::String(category.to_string());
    }
    serde_json::to_writer(&mut *writer, &record)?;
    writer.write_all(b"\n")?;
    writer.flush()?;
    Ok(())
}
