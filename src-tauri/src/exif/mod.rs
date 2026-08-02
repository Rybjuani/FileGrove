//! EXIF reader wrapping `kamadak-exif`.
//!
//! Reading image containers is blocking work. Callers on an async runtime
//! should use [`read_exif`], which moves that work to Tokio's blocking pool.

use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{FixedOffset, NaiveDateTime, Timelike};

/// EXIF metadata used by path-template tokens.
///
/// EXIF capture dates normally describe the camera's wall-clock time and may
/// not include a timezone. Keeping a `NaiveDateTime` prevents a capture close
/// to midnight from moving to another date merely because it was interpreted
/// as UTC. When the image provides an offset, it is preserved separately.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ExifMeta {
    pub capture_date: Option<NaiveDateTime>,
    pub capture_offset: Option<FixedOffset>,
    pub camera: Option<String>,
    pub lens: Option<String>,
    pub iso: Option<u32>,
    pub focal_length: Option<f64>,
    pub dimensions: Option<String>,
}

/// Read EXIF metadata without blocking an async runtime thread.
///
/// Missing EXIF, unsupported image containers and malformed metadata are
/// intentionally treated as empty metadata so destination tokens can use
/// their documented fallbacks.
pub async fn read_exif(path: &str) -> Result<ExifMeta> {
    let path = PathBuf::from(path);
    tokio::task::spawn_blocking(move || read_exif_sync(path))
        .await
        .context("EXIF reader task failed")?
}

/// Synchronous EXIF reader for non-async callers and tests.
pub fn read_exif_sync(path: impl AsRef<Path>) -> Result<ExifMeta> {
    let path = path.as_ref();
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(ExifMeta::default());
        }
        Err(error) => {
            return Err(error).with_context(|| format!("could not open image {}", path.display()));
        }
    };

    let mut reader = BufReader::new(file);
    let exif = match exif::Reader::new().read_from_container(&mut reader) {
        Ok(exif) => exif,
        // EXIF is optional. A corrupt or unsupported metadata block must not
        // prevent the file from being organized with mtime fallbacks.
        Err(_) => return Ok(ExifMeta::default()),
    };

    Ok(extract_metadata(&exif))
}

fn extract_metadata(exif: &exif::Exif) -> ExifMeta {
    let (capture_date, capture_offset) = capture_datetime(exif)
        .map(|(date, offset)| (Some(date), offset))
        .unwrap_or((None, None));

    let camera = ascii_field(exif, exif::Tag::Model);
    let lens = ascii_field(exif, exif::Tag::LensModel);
    let iso = first_uint(
        exif,
        &[
            exif::Tag::PhotographicSensitivity,
            exif::Tag::StandardOutputSensitivity,
            exif::Tag::RecommendedExposureIndex,
            exif::Tag::ISOSpeed,
        ],
    )
    .filter(|value| *value > 0);

    let focal_length = field_for_tag(exif, exif::Tag::FocalLength)
        .and_then(|field| first_number(&field.value))
        .filter(|value| value.is_finite() && *value > 0.0);

    let width = first_uint(exif, &[exif::Tag::PixelXDimension, exif::Tag::ImageWidth])
        .filter(|value| *value > 0);
    let height = first_uint(exif, &[exif::Tag::PixelYDimension, exif::Tag::ImageLength])
        .filter(|value| *value > 0);
    let dimensions = width
        .zip(height)
        .map(|(width, height)| format!("{width}x{height}"));

    ExifMeta {
        capture_date,
        capture_offset,
        camera,
        lens,
        iso,
        focal_length,
        dimensions,
    }
}

fn capture_datetime(exif: &exif::Exif) -> Option<(NaiveDateTime, Option<FixedOffset>)> {
    let candidates = [
        (
            exif::Tag::DateTimeOriginal,
            exif::Tag::SubSecTimeOriginal,
            exif::Tag::OffsetTimeOriginal,
        ),
        (
            exif::Tag::DateTimeDigitized,
            exif::Tag::SubSecTimeDigitized,
            exif::Tag::OffsetTimeDigitized,
        ),
        (
            exif::Tag::DateTime,
            exif::Tag::SubSecTime,
            exif::Tag::OffsetTime,
        ),
    ];

    candidates
        .into_iter()
        .find_map(|(date_tag, subsec_tag, offset_tag)| {
            let raw_date = ascii_field(exif, date_tag)?;
            let subsecond = ascii_field(exif, subsec_tag);
            let date = parse_exif_datetime(&raw_date, subsecond.as_deref())?;
            let offset = ascii_field(exif, offset_tag).and_then(|raw| parse_exif_offset(&raw));
            Some((date, offset))
        })
}

fn parse_exif_datetime(raw: &str, separate_subsecond: Option<&str>) -> Option<NaiveDateTime> {
    let raw = raw.trim().trim_matches('\0');
    let formats = [
        "%Y:%m:%d %H:%M:%S%.f",
        "%Y-%m-%d %H:%M:%S%.f",
        "%Y:%m:%d %H:%M:%S",
        "%Y-%m-%d %H:%M:%S",
    ];
    let mut date = formats
        .into_iter()
        .find_map(|format| NaiveDateTime::parse_from_str(raw, format).ok())?;

    if date.nanosecond() == 0 {
        if let Some(nanoseconds) = parse_subseconds(separate_subsecond.unwrap_or_default()) {
            date = date.with_nanosecond(nanoseconds)?;
        }
    }
    Some(date)
}

fn parse_subseconds(raw: &str) -> Option<u32> {
    let digits: String = raw
        .trim()
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .take(9)
        .collect();
    if digits.is_empty() {
        return None;
    }

    let value = digits.parse::<u32>().ok()?;
    value.checked_mul(10_u32.pow(9 - digits.len() as u32))
}

fn parse_exif_offset(raw: &str) -> Option<FixedOffset> {
    let raw = raw.trim().trim_matches('\0');
    let sign = match raw.as_bytes().first().copied()? {
        b'+' => 1,
        b'-' => -1,
        _ => return None,
    };
    let digits: String = raw[1..].chars().filter(|ch| ch.is_ascii_digit()).collect();
    if digits.len() != 4 {
        return None;
    }
    let hours = digits[..2].parse::<i32>().ok()?;
    let minutes = digits[2..].parse::<i32>().ok()?;
    if hours > 23 || minutes > 59 {
        return None;
    }
    FixedOffset::east_opt(sign * (hours * 3600 + minutes * 60))
}

fn ascii_field(exif: &exif::Exif, tag: exif::Tag) -> Option<String> {
    field_for_tag(exif, tag).and_then(|field| ascii_value(&field.value))
}

fn field_for_tag(exif: &exif::Exif, tag: exif::Tag) -> Option<&exif::Field> {
    exif.get_field(tag, exif::In::PRIMARY)
        .or_else(|| exif.fields().find(|field| field.tag == tag))
}

fn ascii_value(value: &exif::Value) -> Option<String> {
    let exif::Value::Ascii(values) = value else {
        return None;
    };

    values.iter().find_map(|bytes| {
        let value = String::from_utf8_lossy(bytes)
            .trim_matches('\0')
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        (!value.is_empty()).then_some(value)
    })
}

fn first_uint(exif: &exif::Exif, tags: &[exif::Tag]) -> Option<u32> {
    tags.iter()
        .find_map(|tag| field_for_tag(exif, *tag).and_then(|field| field.value.get_uint(0)))
}

fn first_number(value: &exif::Value) -> Option<f64> {
    match value {
        exif::Value::Rational(values) => values
            .first()
            .and_then(|value| (value.denom != 0).then_some(value.num as f64 / value.denom as f64)),
        exif::Value::SRational(values) => values
            .first()
            .and_then(|value| (value.denom != 0).then_some(value.num as f64 / value.denom as f64)),
        exif::Value::Float(values) => values.first().map(|value| f64::from(*value)),
        exif::Value::Double(values) => values.first().copied(),
        _ => value.get_uint(0).map(f64::from),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Datelike, Timelike};

    #[test]
    fn parses_capture_date_subseconds_and_offset() {
        let date = parse_exif_datetime("2024:07:22 09:30:15", Some("12500")).unwrap();
        assert_eq!((date.year(), date.month(), date.day()), (2024, 7, 22));
        assert_eq!((date.hour(), date.minute(), date.second()), (9, 30, 15));
        assert_eq!(date.nanosecond(), 125_000_000);

        let offset = parse_exif_offset("-03:00").unwrap();
        assert_eq!(offset.local_minus_utc(), -10_800);
        assert!(parse_exif_offset("+25:00").is_none());
    }

    #[test]
    fn rejects_blank_and_invalid_dates() {
        assert!(parse_exif_datetime("0000:00:00 00:00:00", None).is_none());
        assert!(parse_exif_datetime("not a date", None).is_none());
        assert!(parse_subseconds("").is_none());
    }

    #[test]
    fn extracts_complete_metadata_from_tiff() {
        let bytes = complete_test_tiff();
        let exif = exif::Reader::new().read_raw(bytes).unwrap();
        let metadata = extract_metadata(&exif);

        assert_eq!(metadata.camera.as_deref(), Some("Canon EOS R6"));
        assert_eq!(metadata.lens.as_deref(), Some("RF24-70 F2.8 L IS USM"));
        assert_eq!(metadata.iso, Some(200));
        assert_eq!(metadata.focal_length, Some(50.0));
        assert_eq!(metadata.dimensions.as_deref(), Some("6000x4000"));
        assert_eq!(
            metadata.capture_date.unwrap().date().to_string(),
            "2024-07-22"
        );
        assert_eq!(metadata.capture_offset.unwrap().local_minus_utc(), -10_800);
    }

    #[tokio::test]
    async fn unsupported_files_and_missing_paths_return_empty_metadata() {
        let directory = tempfile::tempdir().unwrap();
        let text_path = directory.path().join("not-an-image.txt");
        std::fs::write(&text_path, b"plain text").unwrap();

        let metadata = read_exif(text_path.to_str().unwrap()).await.unwrap();
        assert_eq!(metadata, ExifMeta::default());

        let missing = read_exif(directory.path().join("missing.jpg").to_str().unwrap())
            .await
            .unwrap();
        assert_eq!(missing, ExifMeta::default());
    }

    fn complete_test_tiff() -> Vec<u8> {
        // Minimal little-endian TIFF with one primary IFD and one EXIF IFD.
        const EXIF_IFD_OFFSET: u32 = 38;
        const DATA_OFFSET: u32 = 128;
        let camera = b"Canon EOS R6\0";
        let date = b"2024:07:22 09:30:15\0";
        let offset = b"-03:00\0";
        let lens = b"RF24-70 F2.8 L IS USM\0";

        let camera_offset = DATA_OFFSET;
        let date_offset = camera_offset + camera.len() as u32;
        let offset_offset = date_offset + date.len() as u32;
        let focal_offset = offset_offset + offset.len() as u32;
        let lens_offset = focal_offset + 8;

        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"II");
        push_u16(&mut bytes, 42);
        push_u32(&mut bytes, 8);

        // IFD0: Model and ExifIFDPointer.
        push_u16(&mut bytes, 2);
        push_entry(&mut bytes, 0x0110, 2, camera.len() as u32, camera_offset);
        push_entry(&mut bytes, 0x8769, 4, 1, EXIF_IFD_OFFSET);
        push_u32(&mut bytes, 0);

        // EXIF IFD.
        assert_eq!(bytes.len(), EXIF_IFD_OFFSET as usize);
        push_u16(&mut bytes, 7);
        push_entry(&mut bytes, 0x8827, 3, 1, 200);
        push_entry(&mut bytes, 0x9003, 2, date.len() as u32, date_offset);
        push_entry(&mut bytes, 0x9011, 2, offset.len() as u32, offset_offset);
        push_entry(&mut bytes, 0x920a, 5, 1, focal_offset);
        push_entry(&mut bytes, 0xa002, 4, 1, 6000);
        push_entry(&mut bytes, 0xa003, 4, 1, 4000);
        push_entry(&mut bytes, 0xa434, 2, lens.len() as u32, lens_offset);
        push_u32(&mut bytes, 0);
        assert_eq!(bytes.len(), DATA_OFFSET as usize);

        bytes.extend_from_slice(camera);
        bytes.extend_from_slice(date);
        bytes.extend_from_slice(offset);
        push_u32(&mut bytes, 50);
        push_u32(&mut bytes, 1);
        bytes.extend_from_slice(lens);
        bytes
    }

    fn push_entry(bytes: &mut Vec<u8>, tag: u16, field_type: u16, count: u32, value: u32) {
        push_u16(bytes, tag);
        push_u16(bytes, field_type);
        push_u32(bytes, count);
        push_u32(bytes, value);
    }

    fn push_u16(bytes: &mut Vec<u8>, value: u16) {
        bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn push_u32(bytes: &mut Vec<u8>, value: u32) {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
}
