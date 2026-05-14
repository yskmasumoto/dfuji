//! # dfuji-geo
//!
//! 地理座標計算のライブラリクレート
//!
//! ## 主な機能
//! - 任意の2点間の高度角・方位角の計算
//! - ECEF座標系への変換
//! - WGS84楕円体を用いた測地計算

use dfuji_core::{WGS84_A, WGS84_E2};
use geographiclib_rs::{DirectGeodesic, Geodesic, InverseGeodesic};
use geojson::{Feature, FeatureCollection, Geometry, Value};

/// # calc_altitude
/// 任意の2点間の高度角を計算する関数
/// # Arguments
/// * `obs_lat` - 観測者の緯度（度）
/// * `obs_lon` - 観測者の経度（度）
/// * `obs_alt` - 観測者の標高（メートル）
/// * `dest_lat` - 目的地の緯度（度）
/// * `dest_lon` - 目的地の経度（度）
/// * `dest_alt` - 目的地の標高（メートル）
/// # Returns
/// * 目的地の高度角（度）
#[allow(clippy::too_many_arguments)]
pub fn calc_altitude(
    obs_lat: f64,
    obs_lon: f64,
    obs_alt: f64,
    dest_lat: f64,
    dest_lon: f64,
    dest_alt: f64,
) -> f64 {
    let (obs_x, obs_y, obs_z) = geodetic_to_ecef(obs_lat, obs_lon, obs_alt);
    let (dest_x, dest_y, dest_z) = geodetic_to_ecef(dest_lat, dest_lon, dest_alt);

    let dx = dest_x - obs_x;
    let dy = dest_y - obs_y;
    let dz = dest_z - obs_z;

    let lat_rad = obs_lat.to_radians();
    let lon_rad = obs_lon.to_radians();
    let sin_lat = lat_rad.sin();
    let cos_lat = lat_rad.cos();
    let sin_lon = lon_rad.sin();
    let cos_lon = lon_rad.cos();

    // 観測地点の局所 ENU 座標系に変換
    let east = -sin_lon * dx + cos_lon * dy;
    let north = -sin_lat * cos_lon * dx - sin_lat * sin_lon * dy + cos_lat * dz;
    let up = cos_lat * cos_lon * dx + cos_lat * sin_lon * dy + sin_lat * dz;

    let slant_range = (dx * dx + dy * dy + dz * dz).sqrt();
    if slant_range < f64::EPSILON {
        return 90.0;
    }

    let horizontal = (east * east + north * north).sqrt();
    up.atan2(horizontal).to_degrees()
}

/// # geodetic_to_ecef
/// 緯度・経度・高度からECEF座標を計算する関数
/// # Arguments
/// * `lat_deg` - 緯度（度）
/// * `lon_deg` - 経度（度）
/// * `alt_m` - 高度（メートル）
/// # Returns
/// * `(f64, f64, f64)` - ECEF座標 (X, Y, Z)（メートル）
fn geodetic_to_ecef(lat_deg: f64, lon_deg: f64, alt_m: f64) -> (f64, f64, f64) {
    let lat = lat_deg.to_radians();
    let lon = lon_deg.to_radians();
    let sin_lat = lat.sin();
    let cos_lat = lat.cos();
    let sin_lon = lon.sin();
    let cos_lon = lon.cos();

    let n = WGS84_A / (1.0 - WGS84_E2 * sin_lat * sin_lat).sqrt();
    let x = (n + alt_m) * cos_lat * cos_lon;
    let y = (n + alt_m) * cos_lat * sin_lon;
    let z = (n * (1.0 - WGS84_E2) + alt_m) * sin_lat;

    (x, y, z)
}

/// # calc_azimuth
/// ## 概要
/// 任意の2点間の方位角を計算する関数
/// # Arguments
/// * `obs_lat` - 観測者の緯度（度）
/// * `obs_lon` - 観測者の経度（度）
/// * `dest_lat` - 目的地の緯度（度）
/// * `dest_lon` - 目的地の経度（度）
/// # Returns
/// * 目的地の方位角（度）
pub fn calc_azimuth(obs_lat: f64, obs_lon: f64, dest_lat: f64, dest_lon: f64) -> f64 {
    // 観測点と富士山との方位角と距離を計算
    let geod = Geodesic::wgs84();
    let (az_deg, _, _) = geod.inverse(obs_lat, obs_lon, dest_lat, dest_lon);
    az_deg
}

/// # calc_destination_point
/// 測地線順解法を用いて、ある地点から、指定した方位角と距離に基づいて目的地の緯度・経度を計算する関数
/// # Arguments
/// * `start_lat` - 出発点の緯度（度）
/// * `start_lon` - 出発点の経度（度）
/// * `azimuth` - 方位角（度）
/// * `distance` - 距離（メートル）
/// # Returns
/// * `(f64, f64)` - 目的地の緯度・経度（度）
pub fn calc_destination_point(
    start_lat: f64,
    start_lon: f64,
    azimuth: f64,
    distance: f64,
) -> (f64, f64) {
    let geod = Geodesic::wgs84();
    let (dest_lat, dest_lon, _) = geod.direct(start_lat, start_lon, azimuth, distance);
    (dest_lat, dest_lon)
}

/// # vec2geojson
/// 座標のベクタをGeoJSON形式の文字列に変換する関数
/// # Arguments
/// * `coords` - 座標のベクタ（(経度, 緯度)のタプルのベクタ）
/// # Returns
/// * GeoJSON形式の文字列
pub fn vec2geojson(coords: &[(f64, f64)]) -> String {
    let feature_collection = if coords.is_empty() {
        FeatureCollection {
            features: Vec::new(),
            bbox: None,
            foreign_members: None,
        }
    } else {
        let geometry = match coords.len() {
            1 => {
                let (lon, lat) = coords[0];
                Value::Point(vec![lon, lat])
            }
            2 => {
                let line_string: Vec<Vec<f64>> =
                    coords.iter().map(|&(lon, lat)| vec![lon, lat]).collect();
                Value::LineString(line_string)
            }
            _ => {
                let mut ring: Vec<Vec<f64>> =
                    coords.iter().map(|&(lon, lat)| vec![lon, lat]).collect();
                if ring.first() != ring.last()
                    && let Some(first) = ring.first().cloned()
                {
                    ring.push(first);
                } else {
                    // 先頭と末尾が同じ場合は何もしない
                }
                Value::Polygon(vec![ring])
            }
        };

        FeatureCollection {
            features: vec![Feature {
                geometry: Some(Geometry::new(geometry)),
                properties: None,
                id: None,
                bbox: None,
                foreign_members: None,
            }],
            bbox: None,
            foreign_members: None,
        }
    };

    let geojson = geojson::GeoJson::from(feature_collection);
    geojson.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ring.first() != ring.last()の条件がtrueの場合のテストケース
    #[test]
    fn test_vec2geojson_open_polygon() {
        let coords = vec![(139.0, 35.0), (140.0, 36.0), (141.0, 35.5)];
        let geojson_str = vec2geojson(&coords);
        let expected = r#"{"type":"FeatureCollection","features":[{"type":"Feature","geometry":{"type":"Polygon","coordinates":[[[139.0,35.0],[140.0,36.0],[141.0,35.5],[139.0,35.0]]]},"properties":null}]}"#;
        assert_eq!(geojson_str, expected);
    }

    /// ring.first() != ring.last()の条件がfalseの場合でも適切に動作するかのテストケース
    #[test]
    fn test_vec2geojson_already_closed_polygon() {
        let coords = vec![(139.0, 35.0), (140.0, 36.0), (139.0, 35.0)];
        let geojson_str = vec2geojson(&coords);
        let expected = r#"{"type":"FeatureCollection","features":[{"type":"Feature","geometry":{"type":"Polygon","coordinates":[[[139.0,35.0],[140.0,36.0],[139.0,35.0]]]},"properties":null}]}"#;
        assert_eq!(geojson_str, expected);
    }
}
