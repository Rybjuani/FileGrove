//! Destination path-template resolver — see SPEC §5.5.
//!
//! Token values originating in filenames or EXIF fields are sanitized as a
//! single filesystem component. They can therefore never inject `..`, a path
//! separator or a Windows device name into the destination template.

use std::path::PathBuf;

use chrono::{Datelike, Timelike, Weekday};

use crate::engine::matcher::FileItem;
use crate::exif::{read_exif, ExifMeta};

const DEFAULT_DATE_FORMAT: &str = "yyyy-mm-dd";
const MAX_DERIVED_COMPONENT_BYTES: usize = 200;

/// Resolve a template without reading EXIF metadata.
///
/// This compatibility entry point applies the documented mtime/empty EXIF
/// fallbacks. Runtime organization should normally call
/// [`resolve_destination_from_file`] instead.
#[allow(dead_code)]
pub fn resolve_destination(template: &str, file: &FileItem) -> Result<PathBuf, String> {
    resolve_destination_with_metadata(template, file, &ExifMeta::default())
}

/// Read the file's EXIF block on Tokio's blocking pool, then resolve every
/// path token. EXIF read failures are non-fatal and use documented fallbacks.
pub async fn resolve_destination_from_file(
    template: &str,
    file: &FileItem,
) -> Result<PathBuf, String> {
    let metadata = read_exif(&file.path).await.unwrap_or_default();
    resolve_destination_with_metadata(template, file, &metadata)
}

/// Resolve a template with already-loaded EXIF metadata.
///
/// This is useful for callers that cache metadata and for deterministic tests.
pub fn resolve_destination_with_metadata(
    template: &str,
    file: &FileItem,
    metadata: &ExifMeta,
) -> Result<PathBuf, String> {
    if template.trim().is_empty() {
        return Err("INVALID_DESTINATION: destination is empty".to_string());
    }
    if template.contains('\0') {
        return Err("INVALID_DESTINATION_TOKEN_FORMAT: destination contains NUL".to_string());
    }

    let mut output = String::with_capacity(template.len() + 32);
    let mut cursor = 0;

    while let Some(relative_start) = template[cursor..].find('{') {
        let start = cursor + relative_start;
        let literal = &template[cursor..start];
        if literal.contains('}') {
            return Err(malformed_token("unmatched closing brace"));
        }
        output.push_str(literal);

        let token_start = start + 1;
        let Some(relative_end) = template[token_start..].find('}') else {
            return Err(malformed_token("unclosed token"));
        };
        let end = token_start + relative_end;
        let raw_token = &template[token_start..end];
        if raw_token.is_empty() || raw_token.contains('{') {
            return Err(malformed_token("empty or nested token"));
        }

        let (name, argument) = raw_token
            .split_once(':')
            .map_or((raw_token, None), |(name, argument)| (name, Some(argument)));
        if name.is_empty() || name.chars().any(char::is_whitespace) {
            return Err(malformed_token("invalid token name"));
        }

        match resolve_token(name, argument, file, metadata)? {
            Some(value) => output.push_str(&value),
            None => {
                // If an optional token occupied a whole path segment, avoid
                // leaving a doubled separator when that token is dropped.
                let after_token = end + 1;
                if output.ends_with(['/', '\\']) {
                    if let Some(next) = template[after_token..].chars().next() {
                        if matches!(next, '/' | '\\') {
                            cursor = after_token + next.len_utf8();
                            continue;
                        }
                    }
                }
            }
        }

        cursor = end + 1;
    }

    let remainder = &template[cursor..];
    if remainder.contains('}') {
        return Err(malformed_token("unmatched closing brace"));
    }
    output.push_str(remainder);

    if output.trim().is_empty() {
        return Err("INVALID_DESTINATION: destination resolved to empty".to_string());
    }
    Ok(PathBuf::from(output))
}

fn resolve_token(
    name: &str,
    argument: Option<&str>,
    file: &FileItem,
    metadata: &ExifMeta,
) -> Result<Option<String>, String> {
    // Interpret filesystem instants as local wall-clock values for path
    // components. Otherwise a UTC date can place files near midnight in the
    // wrong day/month for the user.
    let modified = file.modified_at.with_timezone(&chrono::Local);
    let created = file.created_at.with_timezone(&chrono::Local);
    let exif_date = metadata.capture_date.as_ref();

    let value = match name {
        "date" => Some(format_date(&modified, date_format(argument, name)?)),
        "createddate" => Some(format_date(&created, date_format(argument, name)?)),
        "year" => {
            no_argument(argument, name)?;
            Some(format!("{:04}", modified.year()))
        }
        "month" => {
            no_argument(argument, name)?;
            Some(format!("{:02}", modified.month()))
        }
        "day" => {
            no_argument(argument, name)?;
            Some(format!("{:02}", modified.day()))
        }
        "time" => {
            no_argument(argument, name)?;
            Some(format!("{:02}-{:02}", modified.hour(), modified.minute()))
        }
        "monthname" => {
            no_argument(argument, name)?;
            Some(month_name(modified.month()).to_string())
        }
        "monthshort" => {
            no_argument(argument, name)?;
            Some(month_short(modified.month()).to_string())
        }
        "weekday" => {
            no_argument(argument, name)?;
            Some(weekday_name(modified.weekday()).to_string())
        }
        "weekdayshort" => {
            no_argument(argument, name)?;
            Some(weekday_short(modified.weekday()).to_string())
        }
        "name" => resolve_name_token(file, argument)?,
        "namepart" => resolve_namepart_token(file, argument)?,
        "ext" => resolve_extension_token(file, argument)?,
        "folder" => {
            no_argument(argument, name)?;
            parent_folder_name(&file.path).and_then(|value| sanitize_component(&value))
        }
        "exifdate" => Some(match exif_date {
            Some(date) => format_date(date, date_format(argument, name)?),
            None => format_date(&modified, date_format(argument, name)?),
        }),
        "exifyear" => {
            no_argument(argument, name)?;
            Some(format!(
                "{:04}",
                exif_date.map_or(modified.year(), Datelike::year)
            ))
        }
        "exifweekday" => {
            no_argument(argument, name)?;
            let weekday = exif_date.map_or(modified.weekday(), Datelike::weekday);
            Some(weekday_name(weekday).to_string())
        }
        "exifweekdayshort" => {
            no_argument(argument, name)?;
            let weekday = exif_date.map_or(modified.weekday(), Datelike::weekday);
            Some(weekday_short(weekday).to_string())
        }
        "camera" => {
            no_argument(argument, name)?;
            metadata.camera.as_deref().and_then(sanitize_component)
        }
        "lens" => {
            no_argument(argument, name)?;
            metadata.lens.as_deref().and_then(sanitize_component)
        }
        "iso" => {
            no_argument(argument, name)?;
            metadata.iso.map(|iso| iso.to_string())
        }
        "focallength" => {
            no_argument(argument, name)?;
            metadata.focal_length.map(format_focal_length)
        }
        "dimensions" => {
            no_argument(argument, name)?;
            metadata.dimensions.as_deref().and_then(sanitize_component)
        }
        _ => {
            return Err(format!(
                "INVALID_DESTINATION_TOKEN: unknown token {{{name}}}"
            ));
        }
    };

    Ok(value)
}

fn resolve_name_token(file: &FileItem, argument: Option<&str>) -> Result<Option<String>, String> {
    let name = filename_stem(&file.name);
    let selected = match argument {
        None => Some(name.as_str()),
        Some(argument) => {
            let index = argument.parse::<usize>().map_err(|_| {
                "INVALID_DESTINATION_TOKEN_FORMAT: name index must be a non-negative integer"
                    .to_string()
            })?;
            default_name_parts(&name).nth(index)
        }
    };
    Ok(selected.and_then(sanitize_component))
}

fn resolve_namepart_token(
    file: &FileItem,
    argument: Option<&str>,
) -> Result<Option<String>, String> {
    let name = filename_stem(&file.name);
    let part = match argument {
        None => default_name_parts(&name).next(),
        Some("") => {
            return Err(
                "INVALID_DESTINATION_TOKEN_FORMAT: namepart delimiter is empty".to_string(),
            );
        }
        Some(delimiter) => name.split(delimiter).find(|part| !part.is_empty()),
    };
    Ok(part.and_then(sanitize_component))
}

fn resolve_extension_token(
    file: &FileItem,
    argument: Option<&str>,
) -> Result<Option<String>, String> {
    let extension = file.extension.trim_start_matches('.');
    let extension = match argument {
        None | Some("lower") => extension.to_lowercase(),
        Some("upper") => extension.to_uppercase(),
        Some(_) => {
            return Err(
                "INVALID_DESTINATION_TOKEN_FORMAT: ext accepts only upper or lower".to_string(),
            );
        }
    };
    Ok(sanitize_component(&extension))
}

fn filename_stem(filename: &str) -> String {
    PathBuf::from(filename)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_default()
        .to_string()
}

fn default_name_parts(name: &str) -> impl Iterator<Item = &str> {
    name.split(|ch: char| ch == '_' || ch == '-' || ch == '.' || ch.is_whitespace())
        .filter(|part| !part.is_empty())
}

fn parent_folder_name(path: &str) -> Option<String> {
    PathBuf::from(path)
        .parent()
        .and_then(|parent| parent.file_name())
        .and_then(|name| name.to_str())
        .map(str::to_string)
}

fn date_format<'a>(argument: Option<&'a str>, token: &str) -> Result<&'a str, String> {
    match argument {
        None => Ok(DEFAULT_DATE_FORMAT),
        Some("") => Err(format!(
            "INVALID_DESTINATION_TOKEN_FORMAT: {token} format is empty"
        )),
        Some(format) => Ok(format),
    }
}

fn no_argument(argument: Option<&str>, token: &str) -> Result<(), String> {
    if argument.is_some() {
        return Err(format!(
            "INVALID_DESTINATION_TOKEN_FORMAT: {token} does not accept an argument"
        ));
    }
    Ok(())
}

fn format_date<T>(date: &T, format: &str) -> String
where
    T: Datelike + Timelike,
{
    let chars: Vec<char> = format.chars().collect();
    let mut output = String::with_capacity(format.len() + 8);
    let mut index = 0;

    while index < chars.len() {
        let character = chars[index];
        let run = count_repeats(&chars, index, character);
        match character {
            'y' => {
                if run == 2 {
                    output.push_str(&format!("{:02}", date.year().rem_euclid(100)));
                } else if run == 1 {
                    output.push_str(&date.year().to_string());
                } else {
                    output.push_str(&format!("{:04}", date.year()));
                }
            }
            'm' => push_number(&mut output, date.month(), run),
            'd' => push_number(&mut output, date.day(), run),
            'H' => push_number(&mut output, date.hour(), run),
            'M' => push_number(&mut output, date.minute(), run),
            's' => push_number(&mut output, date.second(), run),
            _ => output.extend(std::iter::repeat(character).take(run)),
        }
        index += run;
    }

    output
}

fn push_number(output: &mut String, value: u32, run: usize) {
    if run >= 2 {
        output.push_str(&format!("{value:02}"));
    } else {
        output.push_str(&value.to_string());
    }
}

fn count_repeats(chars: &[char], start: usize, character: char) -> usize {
    chars[start..]
        .iter()
        .take_while(|candidate| **candidate == character)
        .count()
}

fn month_name(month: u32) -> &'static str {
    const MONTHS: [&str; 12] = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    MONTHS.get(month.saturating_sub(1) as usize).unwrap_or(&"")
}

fn month_short(month: u32) -> &'static str {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    MONTHS.get(month.saturating_sub(1) as usize).unwrap_or(&"")
}

fn weekday_name(weekday: Weekday) -> &'static str {
    match weekday {
        Weekday::Mon => "Monday",
        Weekday::Tue => "Tuesday",
        Weekday::Wed => "Wednesday",
        Weekday::Thu => "Thursday",
        Weekday::Fri => "Friday",
        Weekday::Sat => "Saturday",
        Weekday::Sun => "Sunday",
    }
}

fn weekday_short(weekday: Weekday) -> &'static str {
    match weekday {
        Weekday::Mon => "Mon",
        Weekday::Tue => "Tue",
        Weekday::Wed => "Wed",
        Weekday::Thu => "Thu",
        Weekday::Fri => "Fri",
        Weekday::Sat => "Sat",
        Weekday::Sun => "Sun",
    }
}

fn format_focal_length(value: f64) -> String {
    let mut formatted = format!("{value:.2}");
    while formatted.ends_with('0') {
        formatted.pop();
    }
    if formatted.ends_with('.') {
        formatted.pop();
    }
    format!("{formatted}mm")
}

/// Sanitize untrusted metadata as exactly one safe path component.
fn sanitize_component(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }

    let mut sanitized = String::with_capacity(value.len().min(MAX_DERIVED_COMPONENT_BYTES));
    let mut source_bytes = 0;
    for character in value.chars() {
        let character_bytes = character.len_utf8();
        if source_bytes + character_bytes > MAX_DERIVED_COMPONENT_BYTES {
            break;
        }
        source_bytes += character_bytes;
        if character.is_control()
            || matches!(
                character,
                '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'
            )
        {
            sanitized.push('_');
        } else {
            sanitized.push(character);
        }
    }

    while sanitized.ends_with([' ', '.']) {
        sanitized.pop();
    }
    let sanitized = sanitized.trim();
    if sanitized.is_empty() || matches!(sanitized, "." | "..") {
        return None;
    }

    if is_windows_device_name(sanitized) {
        Some(format!("_{sanitized}"))
    } else {
        Some(sanitized.to_string())
    }
}

fn is_windows_device_name(value: &str) -> bool {
    let base = value
        .split('.')
        .next()
        .unwrap_or(value)
        .to_ascii_uppercase();
    matches!(base.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || base
            .strip_prefix("COM")
            .or_else(|| base.strip_prefix("LPT"))
            .is_some_and(|suffix| suffix.len() == 1 && matches!(suffix.as_bytes()[0], b'1'..=b'9'))
}

fn malformed_token(reason: &str) -> String {
    format!("INVALID_DESTINATION_TOKEN_FORMAT: {reason}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Local, NaiveDate, TimeZone, Utc};

    fn file_at(name: &str, extension: &str) -> FileItem {
        FileItem {
            path: format!("/watched/Inbox/{name}"),
            name: name.to_string(),
            extension: extension.to_string(),
            size: 100,
            modified_at: Local
                .with_ymd_and_hms(2024, 7, 22, 9, 30, 45)
                .single()
                .unwrap()
                .with_timezone(&Utc),
            created_at: Local
                .with_ymd_and_hms(2023, 1, 2, 12, 0, 0)
                .single()
                .unwrap()
                .with_timezone(&Utc),
            identity: None,
        }
    }

    fn complete_metadata() -> ExifMeta {
        ExifMeta {
            capture_date: Some(
                NaiveDate::from_ymd_opt(2022, 12, 31)
                    .unwrap()
                    .and_hms_opt(23, 59, 58)
                    .unwrap(),
            ),
            capture_offset: None,
            camera: Some("Canon EOS R6".to_string()),
            lens: Some("RF24-70 F2.8 L IS USM".to_string()),
            iso: Some(800),
            focal_length: Some(50.5),
            dimensions: Some("6000x4000".to_string()),
        }
    }

    fn resolved(template: &str, file: &FileItem, metadata: &ExifMeta) -> String {
        resolve_destination_with_metadata(template, file, metadata)
            .unwrap()
            .to_string_lossy()
            .into_owned()
    }

    #[test]
    fn resolves_all_file_date_tokens() {
        let file = file_at("photo.jpg", ".jpg");
        let template = "/{date}/{date:dd.mm.yy_HH-MM-ss}/{createddate}/{year}/{month}/{day}/{time}/{monthname}/{monthshort}/{weekday}/{weekdayshort}";
        assert_eq!(
            resolved(template, &file, &ExifMeta::default()),
            "/2024-07-22/22.07.24_09-30-45/2023-01-02/2024/07/22/09-30/July/Jul/Monday/Mon"
        );
    }

    #[test]
    fn resolves_filename_extension_and_folder_tokens() {
        let file = file_at("Trip_to-Buenos Aires.2024.JPG", ".JPG");
        let template =
            "/{name}/{name:0}/{name:1}/{name:3}/{namepart}/{namepart:-}/{ext}/{ext:upper}/{folder}";
        assert_eq!(
            resolved(template, &file, &ExifMeta::default()),
            "/Trip_to-Buenos Aires.2024/Trip/to/Aires/Trip/Trip_to/jpg/JPG/Inbox"
        );
    }

    #[test]
    fn resolves_every_exif_token() {
        let file = file_at("photo.jpg", ".jpg");
        let template = "/{exifdate}/{exifdate:dd-mm-yyyy}/{exifyear}/{exifweekday}/{exifweekdayshort}/{camera}/{lens}/{iso}/{focallength}/{dimensions}";
        assert_eq!(
            resolved(template, &file, &complete_metadata()),
            "/2022-12-31/31-12-2022/2022/Saturday/Sat/Canon EOS R6/RF24-70 F2.8 L IS USM/800/50.5mm/6000x4000"
        );
    }

    #[test]
    fn exif_date_tokens_fall_back_to_mtime_and_optional_values_drop() {
        let file = file_at("photo.jpg", ".jpg");
        let template = "/photos/{camera}/{exifdate}/{exifyear}/{exifweekday}/{exifweekdayshort}";
        assert_eq!(
            resolved(template, &file, &ExifMeta::default()),
            "/photos/2024-07-22/2024/Monday/Mon"
        );
    }

    #[test]
    fn metadata_cannot_inject_traversal_separators_or_devices() {
        let file = file_at("photo.jpg", ".jpg");
        let metadata = ExifMeta {
            camera: Some("../Canon\\EOS:R6*?".to_string()),
            lens: Some("CON".to_string()),
            dimensions: Some("6000/../../4000".to_string()),
            ..ExifMeta::default()
        };
        let output = resolved("/safe/{camera}/{lens}/{dimensions}", &file, &metadata);
        assert_eq!(output, "/safe/.._Canon_EOS_R6__/_CON/6000_.._.._4000");
        assert!(!output.contains("/../"));
        assert!(!output.contains('\\'));
    }

    #[test]
    fn long_metadata_is_bounded() {
        let file = file_at("photo.jpg", ".jpg");
        let metadata = ExifMeta {
            camera: Some("x".repeat(1_000)),
            ..ExifMeta::default()
        };
        let output = resolved("{camera}", &file, &metadata);
        assert_eq!(output.len(), MAX_DERIVED_COMPONENT_BYTES);

        let unicode_metadata = ExifMeta {
            camera: Some("😀".repeat(1_000)),
            ..ExifMeta::default()
        };
        let unicode_output = resolved("{camera}", &file, &unicode_metadata);
        assert!(unicode_output.len() <= MAX_DERIVED_COMPONENT_BYTES);
        assert!(unicode_output.is_char_boundary(unicode_output.len()));
    }

    #[test]
    fn preserves_unicode_in_templates_and_metadata() {
        let file = file_at("foto.jpg", ".jpg");
        let metadata = ExifMeta {
            camera: Some("Cámara Ñandú".to_string()),
            ..ExifMeta::default()
        };
        assert_eq!(
            resolved("/Fotografías/{camera}/{year}", &file, &metadata),
            "/Fotografías/Cámara Ñandú/2024"
        );
    }

    #[test]
    fn malformed_and_unknown_tokens_are_rejected() {
        let file = file_at("photo.jpg", ".jpg");
        for template in ["/{year", "/year}", "/{}", "/{{year}}", "/{unknown}"] {
            assert!(resolve_destination(template, &file).is_err(), "{template}");
        }
        assert!(resolve_destination("/{name:not-a-number}", &file).is_err());
        assert!(resolve_destination("/{ext:title}", &file).is_err());
        assert!(resolve_destination("/{year:yyyy}", &file).is_err());
    }

    #[test]
    fn empty_destination_and_nul_are_rejected() {
        let file = file_at("photo.jpg", ".jpg");
        assert!(resolve_destination("", &file).is_err());
        assert!(resolve_destination("/bad\0path", &file).is_err());
    }
}
