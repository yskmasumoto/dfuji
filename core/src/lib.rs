//! # dfuji-core
//!
//! dfujiプロジェクト全体で使用される定数や構造体を集約したコアクレート。
//!
//! ## 主な定数
//! - 富士山の座標定数（緯度、経度、標高）
//! - WGS84楕円体のパラメータ
//! - ダイヤモンド富士の計算に関する閾値やパラメータ

/// 富士山の緯度（度）
pub const FUJI_LATITUDE: f64 = 35.36063;

/// 富士山の経度（度）
pub const FUJI_LONGITUDE: f64 = 138.72737;

/// 富士山の標高（メートル）
pub const FUJI_ALTITUDE: f64 = 3776.0;

/// WGS84準拠楕円体の長半径（メートル）
pub const WGS84_A: f64 = 6_378_137.0;

/// WGS84楕円体の扁平率
pub const WGS84_F: f64 = 1.0 / 298.257_223_563;

/// WGS84楕円体の第一離心率の二乗
pub const WGS84_E2: f64 = WGS84_F * (2.0 - WGS84_F);

/// 日没時刻からのマイナス時間（時間）
/// ダイヤモンド富士の観測可能時刻を探索する際の開始時刻オフセット
pub const OBSERVATION_OFFSET_HOURS: i64 = 2;

/// 計算の時間間隔（秒）
/// 太陽位置の計算を行う際の時間刻み
pub const CALCULATION_INTERVAL_SECONDS: i64 = 30;

/// 方位角の許容誤差（度）
/// 太陽と富士山の方位角の差がこの値以下であれば一致とみなす
pub const AZIMUTH_THRESHOLD: f64 = 0.2;

/// 高度角の許容誤差（度）
/// 太陽と富士山の高度角の差がこの値以下であれば一致とみなす
pub const ELEVATION_THRESHOLD: f64 = 0.2;

/// 太陽視半径（度）
/// 太陽の見かけの半径を考慮するための定数
pub const SUN_APPARENT_RADIUS: f64 = 0.2666;

/// bisection method関連の定数
/// 高度角から距離を逆算する際の探索範囲
/// 最小距離（メートル）
pub const BISECTION_LOW_DISTANCE: f64 = 100.0;

/// 最大距離（メートル）
pub const BISECTION_HIGH_DISTANCE: f64 = 200_000.0;

/// 最大反復回数
pub const BISECTION_MAX_ITER: usize = 100;

/// 二分法の収束判定に用いる残差の許容値（度）。
///
/// 高度差残差がこの値未満になった時点で収束とみなす。`ELEVATION_THRESHOLD` を
/// 流用すると polygon の境界が数 km レベルでずれるため、距離に換算して数十センチ〜
/// 数メートル相当の精度（1e-4°）を採用している。
pub const BISECTION_RESIDUAL_TOLERANCE_DEG: f64 = 1e-4;

/// 二分法の補助収束判定として用いる、探索区間幅の許容値（メートル）。
///
/// 残差ベースで収束しないケースでも、区間幅がこの値未満になれば距離精度として
/// 十分とみなして打ち切る。
pub const BISECTION_INTERVAL_TOLERANCE_M: f64 = 1.0;
