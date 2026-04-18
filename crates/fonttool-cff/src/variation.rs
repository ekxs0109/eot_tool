use crate::CffError;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VariationAxisValue {
    pub tag: [u8; 4],
    pub value: f32,
}

pub fn parse_variation_axes(input: &str) -> Result<Vec<VariationAxisValue>, CffError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    trimmed
        .split(',')
        .map(|segment| parse_axis_segment(segment.trim()))
        .collect()
}

fn parse_axis_segment(segment: &str) -> Result<VariationAxisValue, CffError> {
    let (tag, value) = segment.split_once('=').ok_or_else(|| {
        CffError::InvalidVariationAxis(format!(
            "invalid variation axis segment `{segment}`"
        ))
    })?;

    let tag = tag.trim();
    let value = value.trim();

    if tag.len() != 4 {
        return Err(CffError::InvalidVariationAxis(format!(
            "variation axis tag `{tag}` must be exactly 4 bytes"
        )));
    }

    let value = value.parse::<f32>().map_err(|_| {
        CffError::InvalidVariationAxis(format!(
            "variation axis value `{value}` is not a valid float"
        ))
    })?;

    Ok(VariationAxisValue {
        tag: tag.as_bytes().try_into().expect("validated tag length"),
        value,
    })
}
