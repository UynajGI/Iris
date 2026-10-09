//! Repack only the pinned ONNX graph's external initializers. The caller checks
//! both input hashes and the exact final single-file hash before publication.
use anyhow::{bail, ensure, Context, Result};

struct Field {
    number: u64,
    wire: u64,
    payload: Vec<u8>,
}
fn varint(bytes: &[u8], cursor: &mut usize) -> Result<u64> {
    let mut value = 0u64;
    for shift in (0..70).step_by(7) {
        let byte = *bytes.get(*cursor).context("truncated protobuf varint")?;
        *cursor += 1;
        ensure!(shift < 63 || byte <= 1, "protobuf integer overflow");
        value |= u64::from(byte & 127) << shift;
        if byte < 128 {
            return Ok(value);
        }
    }
    bail!("invalid protobuf varint")
}
fn encode_varint(mut value: u64, output: &mut Vec<u8>) {
    while value >= 128 {
        output.push((value as u8 & 127) | 128);
        value >>= 7;
    }
    output.push(value as u8);
}
fn fields(bytes: &[u8]) -> Result<Vec<Field>> {
    let mut cursor = 0;
    let mut output = Vec::new();
    while cursor < bytes.len() {
        let tag = varint(bytes, &mut cursor)?;
        ensure!(tag >> 3 > 0, "invalid protobuf field");
        let wire = tag & 7;
        let start = cursor;
        let (start, end) = match wire {
            0 => {
                varint(bytes, &mut cursor)?;
                (start, cursor)
            }
            1 => (
                start,
                start.checked_add(8).context("protobuf size overflow")?,
            ),
            5 => (
                start,
                start.checked_add(4).context("protobuf size overflow")?,
            ),
            2 => {
                let length = usize::try_from(varint(bytes, &mut cursor)?)?;
                (
                    cursor,
                    cursor
                        .checked_add(length)
                        .context("protobuf size overflow")?,
                )
            }
            _ => bail!("unsupported protobuf wire type"),
        };
        let payload = bytes
            .get(start..end)
            .context("truncated protobuf field")?
            .to_vec();
        output.push(Field {
            number: tag >> 3,
            wire,
            payload,
        });
        cursor = end;
    }
    Ok(output)
}
fn serialize(mut fields: Vec<Field>) -> Vec<u8> {
    fields.sort_by_key(|field| field.number);
    let mut bytes = Vec::new();
    for field in fields {
        encode_varint(field.number << 3 | field.wire, &mut bytes);
        if field.wire == 2 {
            encode_varint(field.payload.len() as u64, &mut bytes);
        }
        bytes.extend(field.payload);
    }
    bytes
}
fn initializer(bytes: &[u8], external: &[u8]) -> Result<Vec<u8>> {
    let mut tensor = fields(bytes)?;
    let mut metadata = std::collections::HashMap::new();
    for entry in tensor.iter().filter(|field| field.number == 13) {
        let values = fields(&entry.payload)?;
        let value = |key| -> Result<String> {
            Ok(String::from_utf8(
                values
                    .iter()
                    .find(|field| field.number == key)
                    .context("missing external tensor metadata")?
                    .payload
                    .clone(),
            )?)
        };
        metadata.insert(value(1)?, value(2)?);
    }
    if metadata.is_empty() {
        return Ok(bytes.to_vec());
    }
    ensure!(
        metadata.get("location").map(String::as_str) == Some("model.onnx_data"),
        "unexpected external tensor location"
    );
    let offset = metadata
        .get("offset")
        .context("missing tensor offset")?
        .parse::<usize>()?;
    let length = metadata
        .get("length")
        .context("missing tensor length")?
        .parse::<usize>()?;
    let end = offset
        .checked_add(length)
        .context("tensor range overflow")?;
    let raw = external
        .get(offset..end)
        .context("external tensor outside pinned data")?;
    tensor.retain(|field| ![9, 13, 14].contains(&field.number));
    tensor.push(Field {
        number: 9,
        wire: 2,
        payload: raw.to_vec(),
    });
    tensor.push(Field {
        number: 14,
        wire: 0,
        payload: vec![0],
    });
    Ok(serialize(tensor))
}
pub(super) fn merge(model: &[u8], external: &[u8]) -> Result<Vec<u8>> {
    let mut model = fields(model)?;
    let graph = model
        .iter_mut()
        .find(|field| field.number == 7)
        .context("missing ONNX graph")?;
    let mut nodes = fields(&graph.payload)?;
    for node in nodes.iter_mut().filter(|field| field.number == 5) {
        node.payload = initializer(&node.payload, external)?;
    }
    graph.payload = serialize(nodes);
    Ok(serialize(model))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn malformed_lengths_and_overflow_are_rejected() {
        assert!(fields(&[10, 100, 0]).is_err());
        assert!(fields(&[0]).is_err());
        assert!(fields(&[8, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255]).is_err());
    }
}
