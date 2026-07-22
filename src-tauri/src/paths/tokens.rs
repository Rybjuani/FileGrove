//! Token resolver — see SPEC §5.5.
//!
//! A destination string like `"/photos/{exifyear}/{month}/{camera}"`
//! becomes `/photos/2024/07/Canon EOS R6`. Tokens either resolve to a
//! string (kept in the path) or to `None` (dropped from the path).

use std::path::PathBuf;

use chrono::{Datelike, Timelike};

use crate::engine::matcher::FileItem;

/// Resolve `template` against `file`'s metadata, returning the absolute
/// destination directory.
pub fn resolve_destination(template: &str, file: &FileItem) -> Result<PathBuf, String> {
    let mut out = String::with_capacity(template.len() + 32);
    let bytes = template.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'{' {
            if let Some(end) = template[i + 1..].find('}') {
                let token = &template[i + 1..i + 1 + end];
                let parts: Vec<&str> = token.splitn(2, ':').collect();
                let name = parts[0];
                let arg = parts.get(1).copied();
                if let Some(value) = resolve_token(name, arg, file) {
                    out.push_str(&value);
                } else {
                    let next = i + end + 2;
                    if matches!(out.as_bytes().last(), Some(b'/' | b'\\'))
                        && matches!(bytes.get(next), Some(b'/' | b'\\'))
                    {
                        i += 1;
                    }
                }
                i = i + 1 + end + 1;
                continue;
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    Ok(PathBuf::from(out))
}

fn resolve_token(name: &str, arg: Option<&str>, file: &FileItem) -> Option<String> {
    let fmt = arg.unwrap_or("yyyy-mm-dd");
    match name {
        // Date — based on file mtime
        "date" => Some(format_date(file.modified_at, fmt)),
        "createddate" => Some(format_date(file.created_at, fmt)),
        "year" => Some(format!("{:04}", file.modified_at.year())),
        "month" => Some(format!("{:02}", file.modified_at.month())),
        "day" => Some(format!("{:02}", file.modified_at.day())),
        "time" => Some(format!(
            "{:02}-{:02}",
            file.modified_at.hour(),
            file.modified_at.minute()
        )),
        // Filename
        "name" => Some(stem(&file.name)),
        "ext" => Some(file.extension.clone()),
        // Path
        "folder" => parent_folder_name(&file.path),
        // EXIF tokens — postponed to v0.2.0; fall back to mtime / empty.
        "exifdate" => Some(format_date(file.modified_at, fmt)),
        "exifyear" => Some(format!("{:04}", file.modified_at.year())),
        "camera" | "lens" | "iso" | "focallength" | "dimensions" => None,
        _ => None,
    }
}

fn stem(filename: &str) -> String {
    PathBuf::from(filename)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string()
}

fn parent_folder_name(path: &str) -> Option<String> {
    PathBuf::from(path)
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
}

fn format_date(at: chrono::DateTime<chrono::Utc>, fmt: &str) -> String {
    // Mini-DSL: y=year, m=month, d=day, H=hour, M=minute
    // Accepts `yyyy`, `mm`, `dd`, `HH`, `MM` with any separators.
    let mut out = String::with_capacity(fmt.len());
    let chars: Vec<char> = fmt.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == 'y' {
            let take = count_repeats(&chars, i, 'y').min(4);
            out.push_str(&format!("{:04}", at.year())[..take]);
            i += take;
        } else if chars[i] == 'm' {
            let take = count_repeats(&chars, i, 'm').min(2);
            out.push_str(&format!("{:02}", at.month())[..take]);
            i += take;
        } else if chars[i] == 'd' {
            let take = count_repeats(&chars, i, 'd').min(2);
            out.push_str(&format!("{:02}", at.day())[..take]);
            i += take;
        } else if chars[i] == 'H' {
            let take = count_repeats(&chars, i, 'H').min(2);
            out.push_str(&format!("{:02}", at.hour())[..take]);
            i += take;
        } else if chars[i] == 'M' {
            let take = count_repeats(&chars, i, 'M').min(2);
            out.push_str(&format!("{:02}", at.minute())[..take]);
            i += take;
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

fn count_repeats(chars: &[char], start: usize, c: char) -> usize {
    let mut n = 0;
    for &ch in &chars[start..] {
        if ch == c {
            n += 1;
        } else {
            break;
        }
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn file_at(name: &str, ext: &str, y: i32, m: u32, d: u32) -> FileItem {
        FileItem {
            path: format!("/watched/{}{}", name, ext),
            name: format!("{}{}", name, ext),
            extension: ext.to_string(),
            size: 100,
            modified_at: chrono::Utc.with_ymd_and_hms(y, m, d, 9, 30, 0).unwrap(),
            created_at: chrono::Utc::now(),
        }
    }

    #[test]
    fn resolves_date_tokens() {
        let f = file_at("photo", ".jpg", 2024, 7, 22);
        let p = resolve_destination("/photos/{year}/{month}/{day}", &f).unwrap();
        assert_eq!(p.to_str().unwrap(), "/photos/2024/07/22");
    }

    #[test]
    fn resolves_filename_tokens() {
        let f = file_at("IMG_2024", ".jpg", 2024, 7, 22);
        let p = resolve_destination("/archive/{name}{ext}", &f).unwrap();
        assert_eq!(p.to_str().unwrap(), "/archive/IMG_2024.jpg");
    }

    #[test]
    fn unknown_token_is_dropped() {
        let f = file_at("photo", ".jpg", 2024, 7, 22);
        let p = resolve_destination("/photos/{camera}/{year}", &f).unwrap();
        assert_eq!(p.to_str().unwrap(), "/photos/2024");
    }
}
