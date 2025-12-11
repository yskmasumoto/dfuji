use crate::tools;

use tracing::{info, instrument};

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
    tools::detect_alignment_for_location(orig_lat, orig_lon, year, month, day, true)
}

/// range
/// 緯度・経度の範囲をグリッドサンプリングし、ダイヤモンド富士が見える組み合わせを探索する
/// # Arguments
/// * `lat_min` / `lat_max` / `lat_step` - 走査する緯度の下限・上限・刻み幅（度）
/// * `lon_min` / `lon_max` / `lon_step` - 走査する経度の下限・上限・刻み幅（度）
/// * `year` / `month` / `day` - 評価対象の日付
/// # Returns
/// * 条件を満たした `(lat, lon, unix_time)` のベクタ（見つからない場合は空）
#[allow(clippy::too_many_arguments)]
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
    let latitudes = tools::build_range(lat_min, lat_max, lat_step);
    let longitudes = tools::build_range(lon_min, lon_max, lon_step);
    if latitudes.is_empty() || longitudes.is_empty() {
        info!("Empty latitude or longitude range; skipping search");
        return Vec::new();
    }

    let mut results = Vec::new();
    for lat in &latitudes {
        for lon in &longitudes {
            if let Some(unix_time) =
                tools::detect_alignment_for_location(*lat, *lon, year, month, day, false)
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

/// polygon
/// ダイヤモンド富士の観測可能な範囲をポリゴンとしてGeoJSON形式で返す関数
/// # Arguments
/// * `year` - 年
/// * `month` - 月
/// * `day` - 日
/// # Returns
/// * ポリゴンを表すGeoJSON文字列
#[instrument(level = "info")]
pub fn polygon(year: i16, month: u8, day: u8) -> String {
    let latlon_vec = tools::create_latlon_vec(year, month, day);
    tools::geojson(&latlon_vec)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn point_invalid_date_returns_none() {
        assert!(point(35.0, 138.0, 2025, 13, 1).is_none());
    }

    #[test]
    fn range_with_zero_steps_returns_empty() {
        let result = range(35.0, 36.0, 0.0, 138.0, 139.0, 0.0, 2025, 11, 18);
        assert!(result.is_empty());
    }

    #[test]
    fn polygon_returns_polygon_json() {
        let output = polygon(2025, 11, 18);
        assert!(output.contains("\"type\":\"FeatureCollection\""));
        assert!(output.contains("\"features\":[]"));
    }
}
