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
    use chrono::{Datelike, Timelike};
    use serde_json::Value;

    /// has_point_in_geojson
    /// GeoJSON文字列に指定の緯度経度が含まれるかを誤差許容付きで判定する
    /// # Arguments
    /// * `geojson_str` - GeoJSON形式の文字列
    /// * `lat` - 緯度（度）
    /// * `lon` - 経度（度）
    /// # Returns
    /// * 含まれる場合はtrue、含まれない場合はfalse
    fn has_point_in_geojson(geojson_str: &str, lat: f64, lon: f64) -> bool {
        let value: Value = serde_json::from_str(geojson_str).expect("valid geojson");
        let features = value
            .get("features")
            .and_then(|v| v.as_array())
            .expect("features array in geojson");

        const TOLERANCE: f64 = 1e-6;

        features.iter().any(|feature| {
            let coords = feature
                .get("geometry")
                .and_then(|g| g.get("coordinates"))
                .and_then(|c| c.as_array());

            match coords {
                Some(coords) if coords.len() == 2 => {
                    let x = coords[0].as_f64();
                    let y = coords[1].as_f64();
                    match (x, y) {
                        (Some(x), Some(y)) => {
                            let lat_first =
                                (x - lat).abs() < TOLERANCE && (y - lon).abs() < TOLERANCE;
                            let lon_first =
                                (x - lon).abs() < TOLERANCE && (y - lat).abs() < TOLERANCE;
                            lat_first || lon_first
                        }
                        _ => false,
                    }
                }
                _ => false,
            }
        })
    }

    /// point_invalid_date_returns_none
    /// 単一地点で無効な日付を与えた場合にNoneが返ることを確認するテスト
    #[test]
    fn point_invalid_date_returns_none() {
        assert!(point(35.0, 138.0, 2025, 13, 1).is_none());
    }

    /// range_with_zero_steps_returns_empty
    /// 緯度または経度の刻み幅が0の場合に空のベクタが返ることを確認するテスト
    #[test]
    fn range_with_zero_steps_returns_empty() {
        let result = range(35.0, 36.0, 0.0, 138.0, 139.0, 0.0, 2025, 11, 18);
        assert!(result.is_empty());
    }

    /// polygon_returns_polygon_json
    /// ポリゴンがGeoJSON形式で返ることを確認するテスト
    #[test]
    fn polygon_returns_polygon_json() {
        let output = polygon(2025, 11, 18);
        assert!(output.contains("\"type\":\"FeatureCollection\""));
        assert!(!output.contains("\"features\":[]"));
    }

    /// debug_polygon_point_count
    /// ポリゴン生成のデバッグ用テスト（無視される）
    #[ignore]
    #[test]
    fn debug_polygon_point_count() {
        let (hour, minute, second) = dfuji_sun::calc_sunset_time(
            2025,
            11,
            20,
            dfuji_core::FUJI_LATITUDE,
            dfuji_core::FUJI_LONGITUDE,
        );
        let sunset = chrono::NaiveDate::from_ymd_opt(2025, 11, 20)
            .and_then(|d| d.and_hms_opt(hour as u32, minute as u32, second as u32))
            .expect("valid sunset time");

        for offset in -30..=30 {
            let current_time = sunset + chrono::Duration::minutes(offset.into());
            let (sun_az, sun_alt) = dfuji_sun::calc_sun_az_and_alt(
                current_time.year() as i16,
                current_time.month() as u8,
                current_time.day() as u8,
                current_time.hour() as u8,
                current_time.minute() as u8,
                current_time.second() as f64,
                9.0,
                dfuji_core::FUJI_LATITUDE,
                dfuji_core::FUJI_LONGITUDE,
            );
            let obs_az = (sun_az + 180.0).rem_euclid(360.0);
            let (low_lat, low_lon) = dfuji_geo::calc_destination_point(
                dfuji_core::FUJI_LATITUDE,
                dfuji_core::FUJI_LONGITUDE,
                obs_az,
                dfuji_core::BISECTION_LOW_DISTANCE,
            );
            let (high_lat, high_lon) = dfuji_geo::calc_destination_point(
                dfuji_core::FUJI_LATITUDE,
                dfuji_core::FUJI_LONGITUDE,
                obs_az,
                dfuji_core::BISECTION_HIGH_DISTANCE,
            );
            let alt_low = dfuji_geo::calc_altitude(
                low_lat,
                low_lon,
                0.0,
                dfuji_core::FUJI_LATITUDE,
                dfuji_core::FUJI_LONGITUDE,
                dfuji_core::FUJI_ALTITUDE,
            );
            let alt_high = dfuji_geo::calc_altitude(
                high_lat,
                high_lon,
                0.0,
                dfuji_core::FUJI_LATITUDE,
                dfuji_core::FUJI_LONGITUDE,
                dfuji_core::FUJI_ALTITUDE,
            );
            let dist = dfuji_geo::solver_distance_for_altitude(sun_alt, obs_az);
            println!(
                "offset {:>3} min -> sun alt {:>6.2} deg, sun az {:>6.2} deg, alt_low {:>6.2}, alt_high {:>6.2}, distance {:?}",
                offset, sun_alt, sun_az, alt_low, alt_high, dist
            );
        }
        let sanity_alt = dfuji_geo::calc_altitude(
            dfuji_core::FUJI_LATITUDE + 0.001,
            dfuji_core::FUJI_LONGITUDE,
            0.0,
            dfuji_core::FUJI_LATITUDE,
            dfuji_core::FUJI_LONGITUDE,
            dfuji_core::FUJI_ALTITUDE,
        );
        let (dest_lat, dest_lon) = dfuji_geo::calc_destination_point(0.0, 0.0, 90.0, 1000.0);
        println!(
            "altitude sanity check near Fuji: {:.2} deg, dest from (0,0) east 1km -> ({:.6}, {:.6})",
            sanity_alt, dest_lat, dest_lon
        );
        let candidates = crate::tools::create_latlon_vec(2025, 11, 20);
        println!(
            "total candidates from create_latlon_vec: {}",
            candidates.len()
        );
        assert!(!candidates.is_empty());
    }

    /// consistency_among_functions
    /// pointとrange, polygonの整合性を確認するテスト
    /// 観測可能な地点をpoint関数で抽出し、その地点がrange関数とpolygon関数の結果にも含まれることを確認する
    /// （テスト時間短縮のため、抽出地点は先頭3件に制限）
    /// has_point_in_geojson関数を使用してpolygon関数の結果を検証する
    #[test]
    fn consistency_among_functions() {
        let year = 2025;
        let month = 11;
        let day = 18;
        // polygonの生成元データから観測可能な地点を抽出し、テスト時間を抑えるため先頭3件に絞る
        let candidates = crate::tools::create_latlon_vec(year, month, day);
        assert!(!candidates.is_empty(), "Polygon candidate list is empty");

        let mut aligned_samples = Vec::new();
        for (lat, lon) in candidates {
            if let Some(point_time) = point(lat, lon, year, month, day) {
                aligned_samples.push((lat, lon, point_time));
                if aligned_samples.len() >= 3 {
                    break;
                }
            }
        }

        assert!(
            !aligned_samples.is_empty(),
            "No alignment found among polygon candidates",
        );

        // polygon出力は一度だけ生成し、各地点が含まれることを確認する
        let polygon_geojson = polygon(year, month, day);

        for (lat, lon, point_time) in aligned_samples {
            println!("[consistency] candidate lat/lon = ({:.6}, {:.6})", lat, lon);
            println!("[consistency] point result = {:?}", point_time);

            // range関数で同じ地点が含まれることを確認（刻み幅は最小範囲で固定）
            let results = range(lat, lat, 0.0001, lon, lon, 0.0001, year, month, day);
            println!(
                "[consistency] range search returned {} matches",
                results.len()
            );
            let found_in_range = results.iter().any(|(r_lat, r_lon, r_time)| {
                (*r_lat - lat).abs() < 1e-9 && (*r_lon - lon).abs() < 1e-9 && *r_time == point_time
            });
            assert!(
                found_in_range,
                "Point ({lat:.6},{lon:.6}) not found in range results",
            );

            // polygon関数で生成されたGeoJSONに地点が含まれることを確認
            assert!(
                has_point_in_geojson(&polygon_geojson, lat, lon),
                "Point ({lat:.6},{lon:.6}) not found in polygon GeoJSON",
            );
        }
    }
}
