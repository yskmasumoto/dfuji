use crate::geo;
use crate::sun;
use chrono::{Datelike, FixedOffset, LocalResult, TimeZone, Timelike};
use tracing::{debug, info, instrument};

const DEST_LAT: f64 = 35.36063; // 富士山の緯度
const DEST_LON: f64 = 138.72737; // 富士山の経度
const DEST_ALT: f64 = 3776.0; // 富士山の標高

const MINUS_HOUR: i64 = 2; // 日没時刻からのマイナス時間（2時間前）
const INTERVAL_SECONDS: i64 = 30; // 30秒ごとの計算

// 許容誤差
const AZIMUTH_THRESHOLD: f64 = 0.2; // 方位角の許容範囲
const ELEVATION_THRESHOLD: f64 = 0.2; // 高度角の許容範囲

/// angular_diff_deg
/// 2つの角度の差の絶対値を0〜180度の範囲で返す
/// # Arguments
/// * `a` - 角度a（度）
/// * `b` - 角度b（度）
/// # Returns
/// * 2つの角度の差の絶対値（度）
fn angular_diff_deg(a: f64, b: f64) -> f64 {
    let diff = (a - b).rem_euclid(360.0);
    if diff > 180.0 { 360.0 - diff } else { diff }
}

/// point
/// ダイヤモンド富士の観測可能性を単一地点で評価する関数
/// # Arguments
/// * `orig_lat` - 観測者の緯度（度）
/// * `orig_lon` - 観測者の経度（度）
/// * `year` - 年
/// * `month` - 月
/// * `day` - 日
/// # Returns
/// * 条件を満たした場合はその観測時刻（UNIX 時間、秒単位）を `Some` で返し、見つからなければ `None`
#[instrument(
    level = "info",
    skip(orig_lon),
    fields(
        observer.lat = orig_lat,
        observer.lon = orig_lon,
        date = %format!("{:04}-{:02}-{:02}", year, month, day)
    )
)]
pub fn point(orig_lat: f64, orig_lon: f64, year: i16, month: u8, day: u8) -> Option<i64> {
    detect_alignment_for_location(orig_lat, orig_lon, year, month, day, true)
}

/// range
/// 緯度・経度の範囲をグリッドサンプリングし、ダイヤモンド富士が見える組み合わせを探索する
/// # Arguments
/// * `lat_min` / `lat_max` / `lat_step` - 走査する緯度の下限・上限・刻み幅（度）
/// * `lon_min` / `lon_max` / `lon_step` - 走査する経度の下限・上限・刻み幅（度）
/// * `year` / `month` / `day` - 評価対象の日付
/// # Returns
/// * 条件を満たした `(lat, lon, unix_time)` のベクタ（見つからない場合は空）
#[instrument(level = "info", skip(lat_step, lon_step))]
pub fn range(
    lat_min: f64,
    lat_max: f64,
    lat_step: f64,
    lon_min: f64,
    lon_max: f64,
    lon_step: f64,
    year: i16,
    month: u8,
    day: u8,
) -> Vec<(f64, f64, i64)> {
    let latitudes = build_range(lat_min, lat_max, lat_step);
    let longitudes = build_range(lon_min, lon_max, lon_step);
    if latitudes.is_empty() || longitudes.is_empty() {
        info!("Empty latitude or longitude range; skipping search");
        return Vec::new();
    }

    let mut results = Vec::new();
    for lat in &latitudes {
        for lon in &longitudes {
            if let Some(unix_time) =
                detect_alignment_for_location(*lat, *lon, year, month, day, false)
            {
                info!(
                    latitude = lat,
                    longitude = lon,
                    unix_time,
                    "Diamond Fuji alignment detected in range search"
                );
                results.push((*lat, *lon, unix_time));
            }
        }
    }

    if results.is_empty() {
        info!("Diamond Fuji alignment not detected in provided range");
    }
    results
}

/// build_range
/// 指定した範囲とステップ幅に基づいて値のベクタを生成するヘルパー関数
/// start から end まで step ごとに増減させながら値を追加していく
/// # Arguments
/// * `start` - 範囲の開始値
/// * `end` - 範囲の終了値
/// * `step` - ステップ幅
/// # Returns
/// * 指定範囲内の値のベクタ
fn build_range(start: f64, end: f64, step: f64) -> Vec<f64> {
    if step == 0.0 {
        return Vec::new();
    }

    let mut values = Vec::new();
    let mut current = start;
    if start <= end {
        while current <= end + f64::EPSILON {
            values.push(current);
            current += step.abs();
        }
    } else {
        while current >= end - f64::EPSILON {
            values.push(current);
            current -= step.abs();
        }
    }
    values
}

/// detect_alignment_for_location
/// 指定した観測地点と日付に対してダイヤモンド富士の観測可能性を評価する関数
/// # Arguments
/// * `obs_lat` - 観測者の緯度（度）
/// * `obs_lon` - 観測者の経度（度）
/// * `year` - 年
/// * `month` - 月
/// * `day` - 日
/// * `log_enabled` - ログ出力を有効にするかどうか
/// # Returns
/// * 条件を満たした場合はその観測時刻（UNIX 時間、秒単位）を `Some` で返し、見つからなければ `None`
fn detect_alignment_for_location(
    obs_lat: f64,
    obs_lon: f64,
    year: i16,
    month: u8,
    day: u8,
    log_enabled: bool,
) -> Option<i64> {
    let fuji_az_deg = geo::geo::calc_azimuth(obs_lat, obs_lon, DEST_LAT, DEST_LON);
    let fuji_alt_deg = geo::geo::calc_altitude(
        obs_lat, obs_lon, 0.0, // 標高を0mと仮定
        DEST_LAT, DEST_LON, DEST_ALT,
    );

    let sunset_time = sun::sun::calc_sunset_time(year, month, day, obs_lat, obs_lon);

    const SECONDS_PER_DAY: i64 = 24 * 60 * 60;
    let total_seconds = sunset_time.0 * 3600 + sunset_time.1 * 60 + sunset_time.2;
    let day_offset = total_seconds.div_euclid(SECONDS_PER_DAY);
    let mut seconds_in_day = total_seconds.rem_euclid(SECONDS_PER_DAY);
    if seconds_in_day < 0 {
        seconds_in_day += SECONDS_PER_DAY;
    }

    let hour = (seconds_in_day / 3600) as u32;
    let minute = ((seconds_in_day % 3600) / 60) as u32;
    let second = (seconds_in_day % 60) as u32;

    let Some(mut sunset_date) =
        chrono::NaiveDate::from_ymd_opt(year as i32, month as u32, day as u32)
    else {
        if log_enabled {
            eprintln!("Invalid input date: {:04}-{:02}-{:02}", year, month, day);
        }
        return None;
    };

    if day_offset != 0 {
        if let Some(adjusted) = sunset_date.checked_add_signed(chrono::Duration::days(day_offset)) {
            sunset_date = adjusted;
        } else {
            if log_enabled {
                eprintln!(
                    "Date overflow when adjusting sunset time (day offset: {})",
                    day_offset
                );
            }
            return None;
        }
    }

    let Some(sunset_time) = sunset_date.and_hms_opt(hour, minute, second) else {
        if log_enabled {
            eprintln!(
                "Sunset time out of range after normalization: {:02}:{:02}:{:02}",
                hour, minute, second
            );
        }
        return None;
    };

    if log_enabled {
        info!(
            sunset_time = %sunset_time.format("%Y-%m-%d %H:%M:%S"),
            latitude = obs_lat,
            longitude = obs_lon,
            "Computed sunset time"
        );
    }

    let sunset_time_minus_2h = sunset_time - chrono::Duration::hours(MINUS_HOUR);
    let loop_n = MINUS_HOUR * 60 * 60 / INTERVAL_SECONDS;
    let tz = FixedOffset::east_opt(9 * 3600).expect("valid JST offset");

    for i in 0..=loop_n {
        let current_time = sunset_time_minus_2h + chrono::Duration::seconds(i * INTERVAL_SECONDS);
        let current_time_str = current_time.format("%Y-%m-%d %H:%M:%S").to_string();
        if log_enabled {
            debug!(
                current_time = %current_time_str,
                latitude = obs_lat,
                longitude = obs_lon,
                hour = current_time.hour(),
                minute = current_time.minute(),
                second = current_time.second(),
                "Evaluating observer position"
            );
        }

        let (sun_az_deg, sun_alt_deg) = sun::sun::calc_sun_az_and_alt(
            current_time.year() as i16,
            current_time.month() as u8,
            current_time.day() as u8,
            current_time.hour() as u8,
            current_time.minute() as u8,
            current_time.second() as f64,
            9.0,
            obs_lat,
            obs_lon,
        );

        let az_diff = angular_diff_deg(sun_az_deg, fuji_az_deg);
        let alt_diff = (sun_alt_deg - fuji_alt_deg).abs();

        if log_enabled {
            debug!(
                current_time = %current_time_str,
                sun_azimuth_deg = %format!("{:.2}", sun_az_deg),
                sun_altitude_deg = %format!("{:.2}", sun_alt_deg),
                fuji_azimuth_deg = %format!("{:.2}", fuji_az_deg),
                fuji_altitude_deg = %format!("{:.2}", fuji_alt_deg),
                "Apparent positions"
            );
        }

        if az_diff < AZIMUTH_THRESHOLD && alt_diff < ELEVATION_THRESHOLD {
            let unix_time = match tz.from_local_datetime(&current_time) {
                LocalResult::Single(dt) => dt.timestamp(),
                LocalResult::Ambiguous(dt1, dt2) => {
                    if log_enabled {
                        debug!(
                            current_time = %current_time_str,
                            first_candidate = %dt1,
                            second_candidate = %dt2,
                            "Ambiguous local time; selecting earliest candidate"
                        );
                    }
                    dt1.timestamp()
                }
                LocalResult::None => {
                    if log_enabled {
                        debug!(
                            current_time = %current_time_str,
                            "Failed to convert candidate time to UNIX timestamp"
                        );
                    }
                    continue;
                }
            };

            if log_enabled {
                info!(
                    current_time = %current_time_str,
                    latitude = obs_lat,
                    longitude = obs_lon,
                    azimuth_diff = %format!("{:.3}", az_diff),
                    altitude_diff = %format!("{:.3}", alt_diff),
                    unix_time,
                    "Diamond Fuji alignment detected"
                );
            }
            return Some(unix_time);
        }
    }

    if log_enabled {
        info!(
            latitude = obs_lat,
            longitude = obs_lon,
            "Diamond Fuji alignment not detected"
        );
    }
    None
}
