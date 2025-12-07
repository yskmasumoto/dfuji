use geographiclib_rs::{Geodesic, InverseGeodesic};

const WGS84_A: f64 = 6_378_137.0; // WGS84準拠楕円体の長半径
const WGS84_F: f64 = 1.0 / 298.257_223_563; // 扁平率
const WGS84_E2: f64 = WGS84_F * (2.0 - WGS84_F); // 第一離心率^2

// const DEFAULT_INTERVAL: f64 = 0.01; // デフォルトの緯度経度の間隔
// const DEFAULT_LAT_LIMIT_UPPER: f64 = 36.0; // 緯度の上限
// const DEFAULT_LAT_LIMIT_LOWER: f64 = 34.0; // 緯度の下限
// const DEFAULT_LON_LIMIT_UPPER: f64 = 141.0; // 経度の上限
// const DEFAULT_LON_LIMIT_LOWER: f64 = 139.2; // 経度の下限

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
    let elevation = up.atan2(horizontal).to_degrees();
    elevation
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

/// # calc_azimuth_and_altitude
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

// /// 緯度のベクタを返すようにシグネチャを変更
// pub fn make_lat_vec(
//     interval: Option<f64>,
//     lat_limit_upper: Option<f64>,
//     lat_limit_lower: Option<f64>,
// ) -> Vec<f64> {
//     // デフォルト値
//     let interval = interval.unwrap_or(DEFAULT_INTERVAL);
//     let lat_min = lat_limit_lower.unwrap_or(DEFAULT_LAT_LIMIT_LOWER);
//     let lat_max = lat_limit_upper.unwrap_or(DEFAULT_LAT_LIMIT_UPPER);

//     // ステップ数を計算
//     // ステップ数は緯度も経度も同じなので、緯度で計算
//     let steps = ((lat_max - lat_min) / interval).ceil() as usize;
//     let mut lat_vec: Vec<f64> = Vec::with_capacity(steps);
//     for i in 0..=steps {
//         lat_vec.push(lat_min + i as f64 * interval);
//     }

//     lat_vec
// }
