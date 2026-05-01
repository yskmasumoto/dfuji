use crate::tools;
use dfuji_geo::vec2geojson;

use tracing::{info, instrument};

/// Alignment
/// 単一観測地点におけるダイヤモンド富士アライメントの検出結果
/// # Fields
/// * `unix_time` - 観測時刻（UNIX 時間、秒単位）
/// * `az_diff` - 方位角の差（度）
/// * `alt_diff` - 高度角の差（度）
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Alignment {
    pub unix_time: i64,
    pub az_diff: f64,
    pub alt_diff: f64,
}

/// RangeMatch
/// `range` 探索における単一の候補
/// # Fields
/// * `lat` - 観測者の緯度（度）
/// * `lon` - 観測者の経度（度）
/// * `alignment` - その地点での検出結果
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RangeMatch {
    pub lat: f64,
    pub lon: f64,
    pub alignment: Alignment,
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
/// * 条件を満たした場合は `Alignment` を `Some` で返し、見つからなければ `None`
#[instrument(
    level = "info",
    skip(orig_lon),
    fields(
        observer.lat = orig_lat,
        observer.lon = orig_lon,
        date = %format!("{:04}-{:02}-{:02}", year, month, day)
    )
)]
pub fn point(orig_lat: f64, orig_lon: f64, year: i16, month: u8, day: u8) -> Option<Alignment> {
    tools::detect_alignment_for_location(orig_lat, orig_lon, year, month, day, true)
}

/// range
/// 緯度・経度の範囲をグリッドサンプリングし、ダイヤモンド富士が見える組み合わせを探索する
/// # Arguments
/// * `lat_min` / `lat_max` / `lat_step` - 走査する緯度の下限・上限・刻み幅（度）
/// * `lon_min` / `lon_max` / `lon_step` - 走査する経度の下限・上限・刻み幅（度）
/// * `year` / `month` / `day` - 評価対象の日付
/// # Returns
/// * 条件を満たした候補の `RangeMatch` ベクタ（見つからない場合は空）
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
) -> Vec<RangeMatch> {
    let latitudes = tools::build_range(lat_min, lat_max, lat_step);
    let longitudes = tools::build_range(lon_min, lon_max, lon_step);
    if latitudes.is_empty() || longitudes.is_empty() {
        info!("Empty latitude or longitude range; skipping search");
        return Vec::new();
    }

    let mut results = Vec::new();
    for lat in &latitudes {
        for lon in &longitudes {
            if let Some(alignment) =
                tools::detect_alignment_for_location(*lat, *lon, year, month, day, false)
            {
                info!(
                    latitude = lat,
                    longitude = lon,
                    unix_time = alignment.unix_time,
                    az_diff = %format!("{:.3}", alignment.az_diff),
                    alt_diff = %format!("{:.3}", alignment.alt_diff),
                    "Diamond Fuji alignment detected in range search"
                );
                results.push(RangeMatch {
                    lat: *lat,
                    lon: *lon,
                    alignment,
                });
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
    let lonlat_vec = tools::create_lonlat_vec(year, month, day);
    vec2geojson(&lonlat_vec)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Datelike, Timelike};
    use serde_json::Value;

    const TOLERANCE: f64 = 1e-6; // 点の一致判定用
    const COLLINEARITY_TOLERANCE: f64 = 1e-10; // 共線性判定用（線分上の点判定）
    const BBOX_MARGIN: f64 = 1e-10; // バウンディングボックスのマージン

    /// is_point_inside_polygon_geojson
    /// polygon関数のGeoJSON出力（Point/LineString/Polygon）に対して、指定の緯度経度が含まれるかを判定する
    fn is_point_inside_polygon_geojson(geojson_str: &str, lat: f64, lon: f64) -> bool {
        let value: Value = serde_json::from_str(geojson_str).expect("valid geojson");
        let feature = match value
            .get("features")
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
        {
            Some(f) => f,
            None => return false,
        };

        let geometry = match feature.get("geometry") {
            Some(g) => g,
            None => return false,
        };

        let geometry_type = geometry.get("type").and_then(|t| t.as_str()).unwrap_or("");

        match geometry_type {
            "Point" => {
                let coords = geometry
                    .get("coordinates")
                    .and_then(|c| c.as_array())
                    .filter(|c| c.len() == 2);
                let Some(coords) = coords else {
                    return false;
                };
                let (x, y) = (coords[0].as_f64(), coords[1].as_f64());
                match (x, y) {
                    (Some(x), Some(y)) => {
                        (x - lon).abs() < TOLERANCE && (y - lat).abs() < TOLERANCE
                    }
                    _ => false,
                }
            }
            "LineString" => {
                // 仕様上は線分上の厳密判定もできるが、ここでは頂点一致のみを見る
                let coords = geometry.get("coordinates").and_then(|c| c.as_array());
                let Some(coords) = coords else {
                    return false;
                };
                coords.iter().any(|p| {
                    let p = p.as_array().filter(|p| p.len() == 2);
                    let Some(p) = p else {
                        return false;
                    };
                    match (p[0].as_f64(), p[1].as_f64()) {
                        (Some(x), Some(y)) => {
                            (x - lon).abs() < TOLERANCE && (y - lat).abs() < TOLERANCE
                        }
                        _ => false,
                    }
                })
            }
            "Polygon" => {
                let ring = geometry
                    .get("coordinates")
                    .and_then(|c| c.as_array())
                    .and_then(|c| c.first())
                    .and_then(|c| c.as_array());
                let Some(ring) = ring else {
                    return false;
                };

                // 頂点一致・辺上を「含まれる」とみなす
                let point_on_segment = |x: f64, y: f64, x1: f64, y1: f64, x2: f64, y2: f64| {
                    // cross product = 0 (collinear) + bounding box check
                    let cross = (x - x1) * (y2 - y1) - (y - y1) * (x2 - x1);
                    if cross.abs() > COLLINEARITY_TOLERANCE {
                        return false;
                    }
                    let min_x = x1.min(x2) - BBOX_MARGIN;
                    let max_x = x1.max(x2) + BBOX_MARGIN;
                    let min_y = y1.min(y2) - BBOX_MARGIN;
                    let max_y = y1.max(y2) + BBOX_MARGIN;
                    x >= min_x && x <= max_x && y >= min_y && y <= max_y
                };

                for i in 0..ring.len().saturating_sub(1) {
                    let a = ring[i].as_array().filter(|p| p.len() == 2);
                    let b = ring[i + 1].as_array().filter(|p| p.len() == 2);
                    let (Some(a), Some(b)) = (a, b) else {
                        continue;
                    };
                    let (x1, y1) = (a[0].as_f64(), a[1].as_f64());
                    let (x2, y2) = (b[0].as_f64(), b[1].as_f64());
                    let (Some(x1), Some(y1), Some(x2), Some(y2)) = (x1, y1, x2, y2) else {
                        continue;
                    };
                    if (x1 - lon).abs() < TOLERANCE && (y1 - lat).abs() < TOLERANCE {
                        return true;
                    }
                    if point_on_segment(lon, lat, x1, y1, x2, y2) {
                        return true;
                    }
                }

                // Ray casting: (lon,lat) 平面近似
                let mut inside = false;
                for i in 0..ring.len().saturating_sub(1) {
                    let a = ring[i].as_array().filter(|p| p.len() == 2);
                    let b = ring[i + 1].as_array().filter(|p| p.len() == 2);
                    let (Some(a), Some(b)) = (a, b) else {
                        continue;
                    };
                    let (x1, y1) = (a[0].as_f64(), a[1].as_f64());
                    let (x2, y2) = (b[0].as_f64(), b[1].as_f64());
                    let (Some(x1), Some(y1), Some(x2), Some(y2)) = (x1, y1, x2, y2) else {
                        continue;
                    };

                    let intersects =
                        (y1 > lat) != (y2 > lat) && (lon < (x2 - x1) * (lat - y1) / (y2 - y1) + x1);
                    if intersects {
                        inside = !inside;
                    }
                }
                inside
            }
            _ => false,
        }
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

    /// point_hit_location_is_inside_polygon
    /// point() でヒットする既知の地点が、polygon() の範囲に含まれることを確認する回帰テスト
    #[test]
    fn point_hit_location_is_inside_polygon() {
        // 既知のヒット地点（2025-11-18）
        let year = 2025;
        let month = 11;
        let day = 18;
        let lat = 35.703999654324605;
        let lon = 139.59943105087748;

        assert!(
            point(lat, lon, year, month, day).is_some(),
            "Expected point() to hit at the known location",
        );

        let polygon_geojson = polygon(year, month, day);
        assert!(
            is_point_inside_polygon_geojson(&polygon_geojson, lat, lon),
            "Known hit location should be inside polygon() output",
        );
    }

    /// multiple_point_hit_locations_are_inside_polygon
    /// point() でヒットする複数地点が、polygon() の範囲に含まれることを確認する回帰テスト
    #[test]
    fn multiple_point_hit_locations_are_inside_polygon() {
        struct Case {
            year: i16,
            month: u8,
            day: u8,
            points: &'static [(f64, f64)],
        }

        // 近傍探索で実際に point() がヒットした座標を固定し、将来の変更で
        // 「point は通るが polygon が包含しない」回帰を検出する。
        let cases = [
            Case {
                year: 2025,
                month: 11,
                day: 18,
                points: &[
                    (35.695_999_654_324_602, 139.589_431_050_877_494),
                    (35.695_999_654_324_602, 139.591_431_050_877_475),
                    (35.697_999_654_324_605, 139.593_431_050_877_484),
                    (35.699_999_654_324_607, 139.591_431_050_877_475),
                    (35.699_999_654_324_607, 139.595_431_050_877_494),
                ],
            },
            Case {
                year: 2025,
                month: 11,
                day: 20,
                points: &[
                    (35.703_999_654_324_605, 139.579_431_050_877_474),
                    (35.703_999_654_324_605, 139.587_431_050_877_484),
                    (35.707_999_654_324_603, 139.583_431_050_877_493),
                    (35.707_999_654_324_603, 139.591_431_050_877_475),
                    (35.707_999_654_324_603, 139.595_431_050_877_494),
                ],
            },
            // BUG-001 回帰: 富士山から ~156 km 東の千葉点。fixed-point iteration の
            // 球面測地補正の符号誤りで center_az が真値から離れ、polygon 出力が
            // 千葉県本土以東に伸びない症状を防ぐ。
            Case {
                year: 2026,
                month: 2,
                day: 24,
                points: &[(35.66, 140.41)],
            },
            // IMPROVE-002 回帰: 富士山から ~195 km の銚子点。`BISECTION_HIGH_DISTANCE`
            // が 200 km 上限のままだと `d_far` の二分法が解けず、銚子方面の方位帯が
            // polygon から落ちる症状を防ぐ。
            Case {
                year: 2026,
                month: 2,
                day: 24,
                points: &[(35.73, 140.83)],
            },
        ];

        for case in cases {
            let polygon_geojson = polygon(case.year, case.month, case.day);

            for &(lat, lon) in case.points {
                assert!(
                    point(lat, lon, case.year, case.month, case.day).is_some(),
                    "Expected point() to hit at (lat={lat}, lon={lon}) for {:04}-{:02}-{:02}",
                    case.year,
                    case.month,
                    case.day,
                );
                assert!(
                    is_point_inside_polygon_geojson(&polygon_geojson, lat, lon),
                    "Expected polygon() to contain (lat={lat}, lon={lon}) for {:04}-{:02}-{:02}",
                    case.year,
                    case.month,
                    case.day,
                );
            }
        }
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
        let candidates = crate::tools::create_lonlat_vec(2025, 11, 20);
        println!(
            "total candidates from create_lonlat_vec: {}",
            candidates.len()
        );
        assert!(!candidates.is_empty());
    }

    /// debug_known_hit_outside_polygon
    /// 既知の point-hit が polygon に入らない原因調査用（無視される）
    #[ignore]
    #[test]
    fn debug_known_hit_outside_polygon() {
        struct Case {
            year: i16,
            month: u8,
            day: u8,
            lat: f64,
            lon: f64,
        }

        let cases = [
            Case {
                year: 2025,
                month: 11,
                day: 18,
                lat: 35.703_999_654_324_605,
                lon: 139.599_431_050_877_48,
            },
            Case {
                year: 2025,
                month: 11,
                day: 20,
                lat: 35.707_999_654_324_6,
                lon: 139.583_431_050_877_5,
            },
        ];

        let tz = chrono::FixedOffset::east_opt(9 * 3600).expect("valid JST offset");

        for c in cases {
            let alignment =
                crate::point(c.lat, c.lon, c.year, c.month, c.day).expect("expected hit");
            let dt = chrono::DateTime::from_timestamp(alignment.unix_time, 0)
                .expect("valid unix")
                .with_timezone(&tz)
                .naive_local();

            let az_from_fuji = dfuji_geo::calc_azimuth(
                dfuji_core::FUJI_LATITUDE,
                dfuji_core::FUJI_LONGITUDE,
                c.lat,
                c.lon,
            );

            let center = crate::tools::debug_estimate_center_az_from_fuji_for_time(dt);

            let fuji_az_from_obs = dfuji_geo::calc_azimuth(
                c.lat,
                c.lon,
                dfuji_core::FUJI_LATITUDE,
                dfuji_core::FUJI_LONGITUDE,
            );
            let fuji_alt_from_obs = dfuji_geo::calc_altitude(
                c.lat,
                c.lon,
                0.0,
                dfuji_core::FUJI_LATITUDE,
                dfuji_core::FUJI_LONGITUDE,
                dfuji_core::FUJI_ALTITUDE,
            );
            let (sun_az, sun_alt) = dfuji_sun::calc_sun_az_and_alt(
                dt.year() as i16,
                dt.month() as u8,
                dt.day() as u8,
                dt.hour() as u8,
                dt.minute() as u8,
                dt.second() as f64,
                9.0,
                c.lat,
                c.lon,
            );

            let az_diff = crate::tools::angular_diff_deg(sun_az, fuji_az_from_obs);
            let alt_diff = (sun_alt - fuji_alt_from_obs).abs();

            let polygon_geojson = crate::polygon(c.year, c.month, c.day);
            let inside = is_point_inside_polygon_geojson(&polygon_geojson, c.lat, c.lon);

            println!("--- {:04}-{:02}-{:02} ---", c.year, c.month, c.day);
            println!("hit time (JST naive): {dt}");
            println!("az_from_fuji(deg)={az_from_fuji:.6}");
            println!("center_estimate={center:.6}");
            let d = crate::tools::angular_diff_deg(az_from_fuji, center);
            println!(
                "angular_diff(az_from_fuji, center)={d:.6} (az_band={:.3})",
                dfuji_core::AZIMUTH_THRESHOLD
            );
            println!(
                "point-metrics: sun_az={sun_az:.3} fuji_az={fuji_az_from_obs:.3} az_diff={az_diff:.6} (thr={:.3})",
                dfuji_core::AZIMUTH_THRESHOLD
            );
            println!(
                "point-metrics: sun_alt={sun_alt:.3} fuji_alt={fuji_alt_from_obs:.3} alt_diff={alt_diff:.6} (thr={:.3})",
                dfuji_core::ELEVATION_THRESHOLD
            );
            println!("inside_polygon={inside}");
        }
    }

    /// diagnose_chiba_polygon_miss
    /// BUG-001 診断専用テスト（無視）。`point()` でヒットする千葉点が `polygon()` に
    /// 内包されない症状について、polygon パイプラインの各段階で何が起きているかを
    /// 実測値ベースで切り分けるためのデバッグ出力を行う。
    ///
    /// 出力内容:
    /// - 千葉点の真方位 `az_from_fuji` と富士山までの距離
    /// - 富士山日没（polygon の base_time）と観測者日没（point の基準）の差
    /// - 各 30 秒刻み時刻での: center_az / 千葉 az が az_band 内か / d_near, d_far / 観測者 az_diff (near, far) / フィルタ通過判定
    /// - 全時刻通して千葉 az 方向の max(d_far) と、千葉までの距離との比較
    #[ignore]
    #[test]
    fn diagnose_chiba_polygon_miss() {
        use chrono::Duration;

        const YEAR: i16 = 2026;
        const MONTH: u8 = 2;
        const DAY: u8 = 24;
        const CHIBA_LAT: f64 = 35.66;
        const CHIBA_LON: f64 = 140.41;

        // 千葉の真方位（富士山視点）と距離（メートル平面近似で十分）
        let az_from_fuji_chiba = dfuji_geo::calc_azimuth(
            dfuji_core::FUJI_LATITUDE,
            dfuji_core::FUJI_LONGITUDE,
            CHIBA_LAT,
            CHIBA_LON,
        );
        let dlat = (CHIBA_LAT - dfuji_core::FUJI_LATITUDE).to_radians();
        let mean_lat = ((CHIBA_LAT + dfuji_core::FUJI_LATITUDE) / 2.0).to_radians();
        let dlon = (CHIBA_LON - dfuji_core::FUJI_LONGITUDE).to_radians() * mean_lat.cos();
        let chiba_distance_m = (dlat.powi(2) + dlon.powi(2)).sqrt() * 6_371_000.0;

        // point() は信頼できる基準
        let alignment = crate::point(CHIBA_LAT, CHIBA_LON, YEAR, MONTH, DAY)
            .expect("Chiba should hit at 2026-02-24");
        let tz = chrono::FixedOffset::east_opt(9 * 3600).expect("valid JST offset");
        let hit_time = chrono::DateTime::from_timestamp(alignment.unix_time, 0)
            .expect("valid unix")
            .with_timezone(&tz)
            .naive_local();

        // polygon の base_time（富士山日没）
        let fuji_sunset = crate::tools::debug_normalize_sunset_naive_datetime(
            YEAR,
            MONTH,
            DAY,
            dfuji_core::FUJI_LATITUDE,
            dfuji_core::FUJI_LONGITUDE,
        )
        .expect("fuji sunset normalization");
        // 観測者日没（point の基準）
        let chiba_sunset = crate::tools::debug_normalize_sunset_naive_datetime(
            YEAR, MONTH, DAY, CHIBA_LAT, CHIBA_LON,
        )
        .expect("chiba sunset normalization");

        let az_band = dfuji_core::AZIMUTH_THRESHOLD + dfuji_core::AZ_FROM_FUJI_PADDING_DEG;
        let start_offset_seconds = -(dfuji_core::OBSERVATION_OFFSET_HOURS * 60 * 60);
        let end_offset_seconds = 0_i64;
        let step_seconds = dfuji_core::CALCULATION_INTERVAL_SECONDS;

        println!("=== Chiba polygon miss diagnosis (BUG-001) ===");
        println!("date: {YEAR:04}-{MONTH:02}-{DAY:02}");
        println!("chiba: lat={CHIBA_LAT}, lon={CHIBA_LON}");
        println!(
            "az_from_fuji(chiba) = {az_from_fuji_chiba:.6} deg, distance to fuji ≈ {:.1} km",
            chiba_distance_m / 1000.0
        );
        println!("hit_time (point, JST naive) = {hit_time}");
        println!("fuji_sunset (polygon base_time) = {fuji_sunset}");
        println!("chiba_sunset (point base_time) = {chiba_sunset}");
        println!(
            "polygon time window: [{}, {}]",
            fuji_sunset + Duration::seconds(start_offset_seconds),
            fuji_sunset + Duration::seconds(end_offset_seconds)
        );
        println!(
            "hit_time vs polygon window: contained = {}",
            hit_time >= fuji_sunset + Duration::seconds(start_offset_seconds)
                && hit_time <= fuji_sunset + Duration::seconds(end_offset_seconds)
        );
        println!(
            "AZIMUTH_THRESHOLD={:.3}  AZ_FROM_FUJI_PADDING_DEG={:.3}  AZ_BIN_WIDTH_DEG={:.3}  ELEVATION_THRESHOLD={:.3}",
            dfuji_core::AZIMUTH_THRESHOLD,
            dfuji_core::AZ_FROM_FUJI_PADDING_DEG,
            dfuji_core::AZ_BIN_WIDTH_DEG,
            dfuji_core::ELEVATION_THRESHOLD,
        );
        println!(
            "BISECTION_HIGH_DISTANCE = {:.0} m",
            dfuji_core::BISECTION_HIGH_DISTANCE
        );
        println!();

        // 各時刻ステップで polygon の動作を再現し、千葉 az 方向に絞って診断
        let mut times_in_band: usize = 0;
        let mut times_filter_passed: usize = 0;
        let mut max_d_far_passed: f64 = f64::NEG_INFINITY;

        let mut offset_seconds = start_offset_seconds;
        let mut samples_count: usize = 0;
        while offset_seconds <= end_offset_seconds {
            let current_time = fuji_sunset + Duration::seconds(offset_seconds);
            let center = crate::tools::debug_estimate_center_az_from_fuji_for_time(current_time);

            let in_band = {
                let d = crate::tools::angular_diff_deg(az_from_fuji_chiba, center);
                d <= az_band
            };

            // 千葉 az 方向で d_near / d_far を直接解く（polygon の 9-sample に依らず純粋な評価）
            let d_near = crate::tools::debug_solve_distance_for_altitude_diff(
                current_time,
                az_from_fuji_chiba,
                dfuji_core::ELEVATION_THRESHOLD,
            );
            let d_far = crate::tools::debug_solve_distance_for_altitude_diff(
                current_time,
                az_from_fuji_chiba,
                -dfuji_core::ELEVATION_THRESHOLD,
            );

            let near_az_diff = d_near.map(|d| {
                let (lat, lon) = dfuji_geo::calc_destination_point(
                    dfuji_core::FUJI_LATITUDE,
                    dfuji_core::FUJI_LONGITUDE,
                    az_from_fuji_chiba,
                    d,
                );
                crate::tools::debug_observer_az_diff_deg(lat, lon, current_time)
            });
            let far_az_diff = d_far.map(|d| {
                let (lat, lon) = dfuji_geo::calc_destination_point(
                    dfuji_core::FUJI_LATITUDE,
                    dfuji_core::FUJI_LONGITUDE,
                    az_from_fuji_chiba,
                    d,
                );
                crate::tools::debug_observer_az_diff_deg(lat, lon, current_time)
            });

            let filter_passed = match (near_az_diff, far_az_diff) {
                (Some(n), Some(f)) => {
                    n < dfuji_core::AZIMUTH_THRESHOLD && f < dfuji_core::AZIMUTH_THRESHOLD
                }
                _ => false,
            };

            if in_band {
                times_in_band += 1;
            }
            if filter_passed {
                times_filter_passed += 1;
                if let Some(d) = d_far
                    && d > max_d_far_passed
                {
                    max_d_far_passed = d;
                }
            }

            // 出力する条件: ヒット時刻 ±2分 / in_band=true / filter_passed=true のいずれか
            let near_hit = (current_time - hit_time).num_seconds().abs() <= 120;
            if near_hit || in_band || filter_passed {
                let n_str = d_near
                    .map(|d| format!("{:.0}m", d))
                    .unwrap_or("None".to_string());
                let f_str = d_far
                    .map(|d| format!("{:.0}m", d))
                    .unwrap_or("None".to_string());
                let na_str = near_az_diff
                    .map(|v| format!("{v:.4}"))
                    .unwrap_or("-".to_string());
                let fa_str = far_az_diff
                    .map(|v| format!("{v:.4}"))
                    .unwrap_or("-".to_string());
                println!(
                    "t={} center_az={:.4} in_band={} d_near={} d_far={} obs_az_diff(near)={} obs_az_diff(far)={} filter_passed={}",
                    current_time.format("%H:%M:%S"),
                    center,
                    in_band,
                    n_str,
                    f_str,
                    na_str,
                    fa_str,
                    filter_passed,
                );
            }

            samples_count += 1;
            offset_seconds += step_seconds;
        }

        println!();
        println!("=== Summary ===");
        println!("total time steps: {samples_count}");
        println!("times where chiba az is within center_az ± {az_band:.2}°: {times_in_band}",);
        println!(
            "times where near AND far observer_az_diff < {:.2}°: {times_filter_passed}",
            dfuji_core::AZIMUTH_THRESHOLD
        );
        if max_d_far_passed.is_finite() {
            println!(
                "max(d_far) across passing times: {:.1} km   (chiba distance ≈ {:.1} km, reaches chiba: {})",
                max_d_far_passed / 1000.0,
                chiba_distance_m / 1000.0,
                max_d_far_passed >= chiba_distance_m,
            );
        } else {
            println!("max(d_far) across passing times: <none — filter rejected all times>");
        }

        // 最終的に polygon の出力を生成し、千葉点の内包判定を出す
        let geojson = crate::polygon(YEAR, MONTH, DAY);
        let inside = is_point_inside_polygon_geojson(&geojson, CHIBA_LAT, CHIBA_LON);
        println!("polygon inside_polygon(chiba) = {inside}");
    }

    /// consistency_among_functions
    /// pointとrange, polygonの整合性を確認するテスト
    /// 観測可能な地点をpoint関数で抽出し、その地点がrange関数とpolygon関数の結果にも含まれることを確認する
    /// （テスト時間短縮のため、抽出地点は先頭3件に制限）
    /// is_point_inside_polygon_geojson関数を使用してpolygon関数の結果を検証する
    #[test]
    fn consistency_among_functions() {
        let year = 2025;
        let month = 11;
        let day = 18;
        // polygonの生成元データから観測可能な地点を抽出し、テスト時間を抑えるため先頭3件に絞る
        let candidates = crate::tools::create_lonlat_vec(year, month, day);
        assert!(!candidates.is_empty(), "Polygon candidate list is empty");

        let mut aligned_samples = Vec::new();
        for (lon, lat) in candidates {
            if let Some(alignment) = point(lat, lon, year, month, day) {
                aligned_samples.push((lat, lon, alignment));
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

        for (lat, lon, alignment) in aligned_samples {
            println!("[consistency] candidate lat/lon = ({:.6}, {:.6})", lat, lon);
            println!("[consistency] point result = {:?}", alignment);

            // range関数で同じ地点が含まれることを確認（刻み幅は最小範囲で固定）
            let results = range(lat, lat, 0.0001, lon, lon, 0.0001, year, month, day);
            println!(
                "[consistency] range search returned {} matches",
                results.len()
            );
            let found_in_range = results.iter().any(|m| {
                (m.lat - lat).abs() < 1e-9
                    && (m.lon - lon).abs() < 1e-9
                    && m.alignment.unix_time == alignment.unix_time
            });
            assert!(
                found_in_range,
                "Point ({lat:.6},{lon:.6}) not found in range results",
            );

            // polygon関数で生成されたGeoJSONに地点が含まれることを確認
            assert!(
                is_point_inside_polygon_geojson(&polygon_geojson, lat, lon),
                "Point ({lat:.6},{lon:.6}) not inside polygon GeoJSON",
            );
        }
    }
}
