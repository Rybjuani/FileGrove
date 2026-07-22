//! EXIF reader wrapping `kamadak-exif`.

#![allow(dead_code)]

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use anyhow::Result;

/// EXIF metadata container.
#[derive(Debug, Clone, Default)]
pub struct ExifMeta {
    pub capture_date: Option<chrono::DateTime<chrono::Utc>>,
    pub camera: Option<String>,
    pub lens: Option<String>,
    pub iso: Option<u32>,
    pub focal_length: Option<f32>,
    pub dimensions: Option<String>,
}

/// Read EXIF metadata from a file. Returns `Ok(Default)` if the file
/// has no EXIF or isn't a supported image.
pub async fn read_exif(path: &str) -> Result<ExifMeta> {
    let p = Path::new(path);
    if !p.exists() {
        return Ok(ExifMeta::default());
    }

    let file = match File::open(p) {
        Ok(f) => f,
        Err(_) => return Ok(ExifMeta::default()),
    };

    let mut buf = BufReader::new(file);
    let exif = match exif::Reader::new().read_from_container(&mut buf) {
        Ok(e) => e,
        Err(_) => return Ok(ExifMeta::default()),
    };

    let mut meta = ExifMeta::default();

    if let Some(field) = exif.get_field(exif::Tag::DateTimeOriginal, exif::In::PRIMARY) {
        let val_str = field.display_value().to_string();
        // EXIF date format: "YYYY:MM:DD HH:MM:SS"
        if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(&val_str, "%Y:%m:%d %H:%M:%S") {
            meta.capture_date = Some(chrono::DateTime::from_naive_utc_and_offset(
                naive,
                chrono::Utc,
            ));
        }
    }

    if let Some(field) = exif.get_field(exif::Tag::Model, exif::In::PRIMARY) {
        meta.camera = Some(
            field
                .display_value()
                .to_string()
                .trim_matches('"')
                .to_string(),
        );
    }

    if let Some(field) = exif.get_field(exif::Tag::LensModel, exif::In::PRIMARY) {
        meta.lens = Some(
            field
                .display_value()
                .to_string()
                .trim_matches('"')
                .to_string(),
        );
    }

    if let Some(field) = exif.get_field(exif::Tag::PhotographicSensitivity, exif::In::PRIMARY) {
        if let Some(val) = field.value.get_uint(0) {
            meta.iso = Some(val);
        }
    }

    if let Some(field) = exif.get_field(exif::Tag::FocalLength, exif::In::PRIMARY) {
        if let exif::Value::Rational(ref v) = field.value {
            if !v.is_empty() && v[0].denom != 0 {
                meta.focal_length = Some(v[0].num as f32 / v[0].denom as f32);
            }
        }
    }

    let width = exif
        .get_field(exif::Tag::PixelXDimension, exif::In::PRIMARY)
        .and_then(|f| f.value.get_uint(0));
    let height = exif
        .get_field(exif::Tag::PixelYDimension, exif::In::PRIMARY)
        .and_then(|f| f.value.get_uint(0));
    if let (Some(w), Some(h)) = (width, height) {
        meta.dimensions = Some(format!("{w}x{h}"));
    }

    Ok(meta)
}
