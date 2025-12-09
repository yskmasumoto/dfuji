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
/// 任意の2点間の方位角と高度角を計算する関数
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
    let (_, dest_lat, dest_lon) = geod.direct(start_lat, start_lon, azimuth, distance);
    (dest_lat, dest_lon)
}

/// # solver_distance_for_altitude
/// ## 概要
/// 指定した高度角と方位角に基づいて、観測地点から目的地までの距離を二分法で求める関数
/// # Arguments
/// * `target_altitude` - ある時刻における太陽の高度角（度）
/// * `obs_azimuth` - 観測地点から目的地への方位角（度）
/// obs_azimuthには太陽の反対側の方位角を指定すること
/// # Returns
/// * `Option<f64>` - 観測地点から目的地までの距離（メートル）
pub fn solver_distance_for_altitude(
    target_altitude: f64,
    obs_azimuth: f64,
) -> Option<f64> {
    // 探索範囲の初期化
    let low = 0.0;
    let high = 200_000.0; // 200 km

    // 二分法による探索
    bisection_method(target_altitude, obs_azimuth, low, high)
}

/// # bisection_method
/// ## 概要
/// 二分法を用いて、指定した高度角に基づいて目的地までの距離を求める関数
/// # Arguments
/// * `target_altitude` - ある時刻における太陽の高度角（度）
/// * `obs_azimuth` - 観測地点から目的地への方位角（度）
/// * `low` - 探索範囲の下限（メートル）
/// * `high` - 探索範囲の上限（メートル）
/// # Returns
/// * `Option<f64>` - 観測地点から目的地までの距離（メートル）
fn bisection_method(
    target_altitude: f64,
    obs_azimuth: f64,
    mut low: f64,
    mut high: f64,
) -> Option<f64> {
    const TOLERANCE: f64 = 0.01; // 許容誤差（度）
    const MAX_ITER: usize = 100; // 最大反復回数

    // 二分法の反復処理
    for _ in 0..MAX_ITER {
        // 中点を計算
        let mid = (low + high) / 2.0;

        // 中点に対応する目的地の緯度・経度を計算
        let (dest_lat, dest_lon) = calc_destination_point(35.3606, 138.7274, obs_azimuth, mid);
        
        // 中点に対応する高度角を計算
        let calculated_altitude = calc_altitude(35.3606, 138.7274, 3776.0, dest_lat, dest_lon, 0.0);

        // 目的の高度角に近いかどうかをチェックして、閾値内であれば解を返す
        if (calculated_altitude - target_altitude).abs() < TOLERANCE {
            return Some(mid);
        }

        // 探索範囲を更新
        if calculated_altitude < target_altitude {
            low = mid;
        } else {
            high = mid;
        }
    }

    None // 解が見つからなかった場合
}
