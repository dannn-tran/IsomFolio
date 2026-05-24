use std::fs::File;
use std::io::BufReader;
use crate::models::ExifTechMeta;

pub struct ExifData {
    pub capture_date: Option<i64>,
    pub gps_lat: Option<f64>,
    pub gps_lon: Option<f64>,
    pub tech: ExifTechMeta,
}

pub fn read_exif(path: &str) -> Option<ExifData> {
    let file = File::open(path).ok()?;
    let mut reader = BufReader::new(file);
    let exif = exif::Reader::new().read_from_container(&mut reader).ok()?;

    let capture_date = exif
        .get_field(exif::Tag::DateTimeOriginal, exif::In::PRIMARY)
        .and_then(|f| ascii_first(&f.value).and_then(exif_datetime_to_unix));

    let lat_ref_south = exif
        .get_field(exif::Tag::GPSLatitudeRef, exif::In::PRIMARY)
        .map(|f| f.display_value().to_string().contains('S'))
        .unwrap_or(false);

    let lon_ref_west = exif
        .get_field(exif::Tag::GPSLongitudeRef, exif::In::PRIMARY)
        .map(|f| f.display_value().to_string().contains('W'))
        .unwrap_or(false);

    let gps_lat = exif
        .get_field(exif::Tag::GPSLatitude, exif::In::PRIMARY)
        .and_then(|f| dms_to_decimal(&f.value))
        .map(|v| if lat_ref_south { -v } else { v });

    let gps_lon = exif
        .get_field(exif::Tag::GPSLongitude, exif::In::PRIMARY)
        .and_then(|f| dms_to_decimal(&f.value))
        .map(|v| if lon_ref_west { -v } else { v });

    let camera_make = ascii_field(&exif, exif::Tag::Make);
    let camera_model = ascii_field(&exif, exif::Tag::Model);
    let lens_model = ascii_field(&exif, exif::Tag::LensModel);

    let focal_length_mm = exif
        .get_field(exif::Tag::FocalLength, exif::In::PRIMARY)
        .and_then(|f| rational_to_f64(&f.value));

    let aperture = exif
        .get_field(exif::Tag::FNumber, exif::In::PRIMARY)
        .and_then(|f| rational_to_f64(&f.value));

    let shutter_speed = exif
        .get_field(exif::Tag::ExposureTime, exif::In::PRIMARY)
        .and_then(|f| match &f.value {
            exif::Value::Rational(v) => v.first().map(|r| match r.num {
                0 => "0".to_string(),
                1 => format!("1/{}", r.denom),
                n => format!("{n}/{}", r.denom),
            }),
            _ => None,
        });

    let iso = exif
        .get_field(exif::Tag::ISOSpeed, exif::In::PRIMARY)
        .and_then(|f| match &f.value {
            exif::Value::Short(v) => v.first().map(|&n| n as i32),
            _ => None,
        });

    let flash = exif
        .get_field(exif::Tag::Flash, exif::In::PRIMARY)
        .and_then(|f| match &f.value {
            exif::Value::Short(v) => v.first().map(|&n| n & 1 == 1),
            _ => None,
        });

    Some(ExifData {
        capture_date,
        gps_lat,
        gps_lon,
        tech: ExifTechMeta { camera_make, camera_model, lens_model, focal_length_mm, aperture, shutter_speed, iso, flash },
    })
}

fn ascii_first(value: &exif::Value) -> Option<&str> {
    let exif::Value::Ascii(v) = value else { return None; };
    std::str::from_utf8(v.first()?).ok()
}

fn ascii_field(exif: &exif::Exif, tag: exif::Tag) -> Option<String> {
    let field = exif.get_field(tag, exif::In::PRIMARY)?;
    let s = ascii_first(&field.value)?.trim_end_matches('\0').trim();
    if s.is_empty() { None } else { Some(s.to_string()) }
}

fn rational_to_f64(v: &exif::Value) -> Option<f64> {
    match v {
        exif::Value::Rational(v) => v.first().filter(|r| r.denom != 0).map(|r| r.num as f64 / r.denom as f64),
        _ => None,
    }
}

fn dms_to_decimal(v: &exif::Value) -> Option<f64> {
    match v {
        exif::Value::Rational(v) if v.len() >= 3 => {
            let to_f = |r: &exif::Rational| if r.denom == 0 { 0.0 } else { r.num as f64 / r.denom as f64 };
            Some(to_f(&v[0]) + to_f(&v[1]) / 60.0 + to_f(&v[2]) / 3600.0)
        }
        _ => None,
    }
}

fn exif_datetime_to_unix(s: &str) -> Option<i64> {
    if s.len() < 19 { return None; }
    let year: i32 = s[0..4].parse().ok()?;
    let month: u32 = s[5..7].parse().ok()?;
    let day: u32 = s[8..10].parse().ok()?;
    let hour: u32 = s[11..13].parse().ok()?;
    let min: u32 = s[14..16].parse().ok()?;
    let sec: u32 = s[17..19].parse().ok()?;

    if year < 1970 || month == 0 || month > 12 || day == 0 || day > 31 {
        return None;
    }

    let mut days: i64 = 0;
    for y in 1970..year {
        days += if is_leap(y) { 366 } else { 365 };
    }
    for m in 1..month {
        days += days_in_month(year, m) as i64;
    }
    days += (day - 1) as i64;
    Some(days * 86400 + hour as i64 * 3600 + min as i64 * 60 + sec as i64)
}

fn is_leap(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => if is_leap(year) { 29 } else { 28 },
        _ => 0,
    }
}
