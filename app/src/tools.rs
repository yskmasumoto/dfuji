use crate::app::Alignment;
use chrono::{Datelike, FixedOffset, LocalResult, TimeZone, Timelike};
use dfuji_core::{
    AZ_BIN_WIDTH_DEG, AZ_FROM_FUJI_PADDING_DEG, AZIMUTH_THRESHOLD, BISECTION_HIGH_DISTANCE,
    BISECTION_INTERVAL_TOLERANCE_M, BISECTION_LOW_DISTANCE, BISECTION_MAX_ITER,
    BISECTION_RESIDUAL_TOLERANCE_DEG, CALCULATION_INTERVAL_SECONDS, ELEVATION_THRESHOLD,
    FUJI_ALTITUDE, FUJI_LATITUDE, FUJI_LONGITUDE, OBSERVATION_OFFSET_HOURS,
};
use dfuji_geo as geo;
use dfuji_sun as sun;
use tracing::{debug, info};

/// normalize_sunset_naive_datetime
/// 指定した緯度経度の日没時刻を計算し、日付をまたぐ場合に対応して正規化する関数
/// # Arguments
/// * `year` - 年
/// * `month` - 月
/// * `day` - 日
/// * `lat` - 緯度（度）
/// * `lon` - 経度（度）
/// # Returns
/// * 正規化された日没時刻の `chrono::NaiveDateTime`
fn normalize_sunset_naive_datetime(
    year: i16,
    month: u8,
    day: u8,
    lat: f64,
    lon: f64,
) -> Option<chrono::NaiveDateTime> {
    let sunset_time = sun::calc_sunset_time(year, month, day, lat, lon);

    const SECONDS_PER_DAY: i64 = 24 * 60 * 60;
    let total_seconds = sunset_time.0 * 3600 + sunset_time.1 * 60 + sunset_time.2;
    let day_offset = total_seconds.div_euclid(SECONDS_PER_DAY);
    let seconds_in_day = total_seconds.rem_euclid(SECONDS_PER_DAY);

    let hour = (seconds_in_day / 3600) as u32;
    let minute = ((seconds_in_day % 3600) / 60) as u32;
    let second = (seconds_in_day % 60) as u32;

    let date = chrono::NaiveDate::from_ymd_opt(year as i32, month as u32, day as u32)?
        .checked_add_signed(chrono::Duration::days(day_offset))?;

    date.and_hms_opt(hour, minute, second)
}

/// alignment_altitude_diff_at_distance
/// 指定した観測地点からの距離における富士山と太陽の高度差を計算する関数
/// # Arguments
/// * `current_time` - 現在の日時（`chrono::NaiveDateTime`）
/// * `az_from_fuji_deg` - 富士山から見た方位角（度）
/// * `distance_m` - 観測地点からの距離（メートル）
/// # Returns
/// * 富士山と太陽の高度差（度）
fn alignment_altitude_diff_at_distance(
    current_time: chrono::NaiveDateTime,
    az_from_fuji_deg: f64,
    distance_m: f64,
) -> f64 {
    let (obs_lat, obs_lon) =
        geo::calc_destination_point(FUJI_LATITUDE, FUJI_LONGITUDE, az_from_fuji_deg, distance_m);

    let fuji_alt_deg = geo::calc_altitude(
        obs_lat,
        obs_lon,
        0.0,
        FUJI_LATITUDE,
        FUJI_LONGITUDE,
        FUJI_ALTITUDE,
    );

    let (_, sun_alt_deg) = sun::calc_sun_az_and_alt(
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

    fuji_alt_deg - sun_alt_deg
}

/// alignment_altitude_diff_minus_target_at_distance
/// 指定した観測地点からの距離における富士山と太陽の高度差から目標高度差を引いた値を計算する関数
/// # Arguments
/// * `current_time` - 現在の日時（`chrono::NaiveDateTime`）
/// * `az_from_fuji_deg` - 富士山から見た方位角（度）
/// * `distance_m` - 観測地点からの距離（メートル）
/// * `target_altitude_diff_deg` - 目標高度差（度）
/// # Returns
/// * 富士山と太陽の高度差から目標高度差を引いた値（度）
fn alignment_altitude_diff_minus_target_at_distance(
    current_time: chrono::NaiveDateTime,
    az_from_fuji_deg: f64,
    distance_m: f64,
    target_altitude_diff_deg: f64,
) -> f64 {
    alignment_altitude_diff_at_distance(current_time, az_from_fuji_deg, distance_m)
        - target_altitude_diff_deg
}

/// solve_distance_for_altitude_diff
/// 指定した高度差に基づいて、観測地点から目的地までの距離を二分法で求める関数
/// # Arguments
/// * `current_time` - 現在の日時（`chrono::NaiveDateTime`）
/// * `az_from_fuji_deg` - 富士山から見た方位角（度）
/// * `target_altitude_diff_deg` - 目標高度差（度）
/// # Returns
/// * 観測地点から目的地までの距離（メートル）
fn solve_distance_for_altitude_diff(
    current_time: chrono::NaiveDateTime,
    az_from_fuji_deg: f64,
    target_altitude_diff_deg: f64,
) -> Option<f64> {
    let mut low = BISECTION_LOW_DISTANCE;
    let mut high = BISECTION_HIGH_DISTANCE;

    let mut f_low = alignment_altitude_diff_minus_target_at_distance(
        current_time,
        az_from_fuji_deg,
        low,
        target_altitude_diff_deg,
    );
    let f_high = alignment_altitude_diff_minus_target_at_distance(
        current_time,
        az_from_fuji_deg,
        high,
        target_altitude_diff_deg,
    );

    if f_low * f_high > 0.0 {
        return None;
    }

    for _ in 0..BISECTION_MAX_ITER {
        let mid = (low + high) / 2.0;
        let f_mid = alignment_altitude_diff_minus_target_at_distance(
            current_time,
            az_from_fuji_deg,
            mid,
            target_altitude_diff_deg,
        );
        if f_mid.abs() < BISECTION_RESIDUAL_TOLERANCE_DEG
            || (high - low) < BISECTION_INTERVAL_TOLERANCE_M
        {
            return Some(mid);
        }

        if f_low * f_mid <= 0.0 {
            high = mid;
        } else {
            low = mid;
            f_low = f_mid;
        }
    }

    Some((low + high) / 2.0)
}

/// solve_distance_for_altitude_match
/// 指定した観測地点からの距離における富士山と太陽の高度を一致させる距離を二分法で求める関数
/// # Arguments
/// * `current_time` - 現在の日時（`chrono::NaiveDateTime`）
/// * `az_from_fuji_deg` - 富士山から見た方位角（度）
/// # Returns
/// * 観測地点から目的地までの距離（メートル）
fn solve_distance_for_altitude_match(
    current_time: chrono::NaiveDateTime,
    az_from_fuji_deg: f64,
) -> Option<f64> {
    solve_distance_for_altitude_diff(current_time, az_from_fuji_deg, 0.0)
}

/// 二分法や固定点反復の最大反復回数の定数定義
const FIXED_POINT_MAX_ITER: usize = 10;

/// 収束判定の閾値（度単位）
const AZ_CONVERGENCE_THRESHOLD_DEG: f64 = 1e-6;

/// estimate_center_az_from_fuji_for_time
/// 指定した日時における、富士山から見た中心方位角を推定する関数
/// # Arguments
/// * `current_time` - 現在の日時（`chrono::NaiveDateTime`）
/// # Returns
/// * 富士山から見た中心方位角（度）
fn estimate_center_az_from_fuji_for_time(current_time: chrono::NaiveDateTime) -> Option<f64> {
    // 「観測地点依存の太陽方位」を吸収するための固定点反復。
    // 反復: az := sun_az(observer_at(az, alt_match)) + 180
    let (sun_az_deg_fuji, _) = sun::calc_sun_az_and_alt(
        current_time.year() as i16,
        current_time.month() as u8,
        current_time.day() as u8,
        current_time.hour() as u8,
        current_time.minute() as u8,
        current_time.second() as f64,
        9.0,
        FUJI_LATITUDE,
        FUJI_LONGITUDE,
    );

    let mut az_from_fuji_deg = (sun_az_deg_fuji + 180.0).rem_euclid(360.0);
    for _ in 0..FIXED_POINT_MAX_ITER {
        let distance_m = solve_distance_for_altitude_match(current_time, az_from_fuji_deg)?;
        let (obs_lat, obs_lon) = geo::calc_destination_point(
            FUJI_LATITUDE,
            FUJI_LONGITUDE,
            az_from_fuji_deg,
            distance_m,
        );
        let (sun_az_deg, _) = sun::calc_sun_az_and_alt(
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
        let next = (sun_az_deg + 180.0).rem_euclid(360.0);
        if angular_diff_deg(next, az_from_fuji_deg) < AZ_CONVERGENCE_THRESHOLD_DEG {
            az_from_fuji_deg = next;
            break;
        }
        az_from_fuji_deg = next;
    }
    Some(az_from_fuji_deg)
}

/// observer_az_diff_deg
/// 指定した観測地点と時刻における「観測者視点の方位角差」（太陽方位角と富士山方位角の差）を返す。
///
/// `point` / `range` のヒット判定（`detect_alignment_for_location`）と同じ基準であり、
/// polygon 候補点に対してこの値が `AZIMUTH_THRESHOLD` 未満かどうかを事後フィルタすることで、
/// 富士山視点の方位サンプリング（`az_from_fuji`）が観測者視点に対してずれている分の
/// 過大評価を排除できる。
///
/// # Arguments
/// * `obs_lat` - 観測者の緯度（度）
/// * `obs_lon` - 観測者の経度（度）
/// * `current_time` - 評価時刻（`chrono::NaiveDateTime`）
/// # Returns
/// * 観測者から見た太陽方位と富士山方位の絶対差（度）
fn observer_az_diff_deg(obs_lat: f64, obs_lon: f64, current_time: chrono::NaiveDateTime) -> f64 {
    let fuji_az_deg = geo::calc_azimuth(obs_lat, obs_lon, FUJI_LATITUDE, FUJI_LONGITUDE);
    let (sun_az_deg, _) = sun::calc_sun_az_and_alt(
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
    angular_diff_deg(sun_az_deg, fuji_az_deg)
}

#[cfg(test)]
pub(crate) fn debug_estimate_center_az_from_fuji_for_time(
    current_time: chrono::NaiveDateTime,
) -> Option<f64> {
    estimate_center_az_from_fuji_for_time(current_time)
}

/// angular_diff_deg
/// 2つの角度の差の絶対値を0〜180度の範囲で返す
/// # Arguments
/// * `a` - 角度a（度）
/// * `b` - 角度b（度）
/// # Returns
/// * 2つの角度の差の絶対値（度）
pub(crate) fn angular_diff_deg(a: f64, b: f64) -> f64 {
    let diff = (a - b).rem_euclid(360.0);
    if diff > 180.0 { 360.0 - diff } else { diff }
}

/// angular_separation_deg
/// 球面三角法による2点間の真の角距離を計算する
/// celestial sphere 上の (az1, alt1) と (az2, alt2) の大円距離を返す
/// # Arguments
/// * `az1` / `alt1` - 1点目の方位角・高度角（度）
/// * `az2` / `alt2` - 2点目の方位角・高度角（度）
/// # Returns
/// * 大円距離（度）
fn angular_separation_deg(az1: f64, alt1: f64, az2: f64, alt2: f64) -> f64 {
    let alt1_r = alt1.to_radians();
    let alt2_r = alt2.to_radians();
    let daz_r = (az1 - az2).to_radians();
    let cos_sigma = alt1_r.sin() * alt2_r.sin() + alt1_r.cos() * alt2_r.cos() * daz_r.cos();
    cos_sigma.clamp(-1.0, 1.0).acos().to_degrees()
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
pub(crate) fn build_range(start: f64, end: f64, step: f64) -> Vec<f64> {
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
///
/// 時間窓全体（日没2時間前〜日没）をスキャンし、太陽中心と富士山頂の球面角距離が
/// 最小となる時刻を選び、その時刻の方位角差・高度角差が閾値内であればアライメント
/// として返す。最初に閾値を満たした時刻で打ち切る方式と異なり、サンプリングステップ
/// 起因のカスケード偽陽性が原理的に発生しない。
///
/// # Arguments
/// * `obs_lat` - 観測者の緯度（度）
/// * `obs_lon` - 観測者の経度（度）
/// * `year` - 年
/// * `month` - 月
/// * `day` - 日
/// * `log_enabled` - ログ出力を有効にするかどうか
/// # Returns
/// * 条件を満たした場合は `Alignment` を `Some` で返し、見つからなければ `None`
pub(crate) fn detect_alignment_for_location(
    obs_lat: f64,
    obs_lon: f64,
    year: i16,
    month: u8,
    day: u8,
    log_enabled: bool,
) -> Option<Alignment> {
    let fuji_az_deg = geo::calc_azimuth(obs_lat, obs_lon, FUJI_LATITUDE, FUJI_LONGITUDE);
    let fuji_alt_deg = geo::calc_altitude(
        obs_lat,
        obs_lon,
        0.0, // 標高を0mと仮定
        FUJI_LATITUDE,
        FUJI_LONGITUDE,
        FUJI_ALTITUDE,
    );

    let sunset_time = sun::calc_sunset_time(year, month, day, obs_lat, obs_lon);

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

    let sunset_time_minus_2h = sunset_time - chrono::Duration::hours(OBSERVATION_OFFSET_HOURS);
    let loop_n = OBSERVATION_OFFSET_HOURS * 60 * 60 / CALCULATION_INTERVAL_SECONDS;
    let tz = FixedOffset::east_opt(9 * 3600).expect("valid JST offset");

    // ベストマッチ追跡: 閾値内候補のうち球面角距離が最小の時刻を保持する。
    // 閾値内候補のみを対象にすることで、グローバル最小 sigma が閾値外でも
    // 別の閾値内候補を取りこぼさない（偽陰性回避）。
    let mut best: Option<BestCandidate> = None;

    for i in 0..=loop_n {
        let current_time =
            sunset_time_minus_2h + chrono::Duration::seconds(i * CALCULATION_INTERVAL_SECONDS);
        if log_enabled {
            debug!(
                current_time = %current_time.format("%Y-%m-%d %H:%M:%S"),
                latitude = obs_lat,
                longitude = obs_lon,
                hour = current_time.hour(),
                minute = current_time.minute(),
                second = current_time.second(),
                "Evaluating observer position"
            );
        }

        let (sun_az_deg, sun_alt_deg) = sun::calc_sun_az_and_alt(
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
        let sigma = angular_separation_deg(sun_az_deg, sun_alt_deg, fuji_az_deg, fuji_alt_deg);

        if log_enabled {
            debug!(
                current_time = %current_time.format("%Y-%m-%d %H:%M:%S"),
                sun_azimuth_deg = %format!("{:.2}", sun_az_deg),
                sun_altitude_deg = %format!("{:.2}", sun_alt_deg),
                fuji_azimuth_deg = %format!("{:.2}", fuji_az_deg),
                fuji_altitude_deg = %format!("{:.2}", fuji_alt_deg),
                angular_separation_deg = %format!("{:.4}", sigma),
                "Apparent positions"
            );
        }

        // 閾値外は best 追跡対象外（偽陰性回避）
        if az_diff >= AZIMUTH_THRESHOLD || alt_diff >= ELEVATION_THRESHOLD {
            continue;
        }

        if best.as_ref().is_none_or(|b| sigma < b.sigma) {
            best = Some(BestCandidate {
                naive_time: current_time,
                az_diff,
                alt_diff,
                sigma,
            });
        }
    }

    let Some(best) = best else {
        if log_enabled {
            info!(
                latitude = obs_lat,
                longitude = obs_lon,
                "Diamond Fuji alignment not detected"
            );
        }
        return None;
    };

    // UNIX 変換はベスト確定後に1回だけ実施する（パフォーマンス）
    let unix_time = match tz.from_local_datetime(&best.naive_time) {
        LocalResult::Single(dt) => dt.timestamp(),
        LocalResult::Ambiguous(dt1, dt2) => {
            if log_enabled {
                debug!(
                    naive_time = %best.naive_time.format("%Y-%m-%d %H:%M:%S"),
                    first_candidate = %dt1,
                    second_candidate = %dt2,
                    "Ambiguous local time; selecting earliest candidate"
                );
            }
            dt1.timestamp()
        }
        LocalResult::None => {
            if log_enabled {
                info!(
                    naive_time = %best.naive_time.format("%Y-%m-%d %H:%M:%S"),
                    "Failed to convert best candidate time to UNIX timestamp"
                );
            }
            return None;
        }
    };

    if log_enabled {
        info!(
            latitude = obs_lat,
            longitude = obs_lon,
            azimuth_diff = %format!("{:.3}", best.az_diff),
            altitude_diff = %format!("{:.3}", best.alt_diff),
            angular_separation_deg = %format!("{:.4}", best.sigma),
            unix_time,
            "Diamond Fuji alignment detected"
        );
    }
    Some(Alignment {
        unix_time,
        az_diff: best.az_diff,
        alt_diff: best.alt_diff,
    })
}

/// BestCandidate
/// `detect_alignment_for_location` の内部でベストマッチを追跡するための構造体
/// # Fields
/// * `naive_time` - 候補時刻（タイムゾーン未確定）
/// * `az_diff` - 方位角の差（度）
/// * `alt_diff` - 高度角の差（度）
/// * `sigma` - 太陽中心と富士山頂の球面角距離（度）
struct BestCandidate {
    naive_time: chrono::NaiveDateTime,
    az_diff: f64,
    alt_diff: f64,
    sigma: f64,
}

/// create_lonlat_vec
/// 富士山の周囲でダイヤモンド富士が観測可能な地点の経度緯度ペアを生成する
/// # Arguments
/// * `year` - 年
/// * `month` - 月
/// * `day` - 日
/// # Returns
/// * ダイヤモンド富士が観測可能な地点の経度緯度ペアのベクタ
pub(crate) fn create_lonlat_vec(year: i16, month: u8, day: u8) -> Vec<(f64, f64)> {
    // 富士山の緯度経度における日没時刻を正規化して取得
    let Some(base_time) =
        normalize_sunset_naive_datetime(year, month, day, FUJI_LATITUDE, FUJI_LONGITUDE)
    else {
        return Vec::new();
    };

    // ## 設計方針（方位ビン集約版）
    //
    // 旧実装は (time × az_from_fuji) のサンプル点を時刻順 envelope リングにそのまま並べていたため、
    // 時刻ごとに中心方位がシフトし、サンプル粒度由来のステップが重畳して、出力リングが
    // 時刻数 × AZ_SAMPLES × 2 ≈ 1500 頂点でギザつく問題があった。
    //
    // 新実装では時刻軸を **方位ビンごとに包絡** へ畳み込む:
    //   1. 従来通り (time × az_from_fuji) でサンプリングし `(az, d_near, d_far)` を集める
    //   2. 富士山視点方位を `AZ_BIN_WIDTH_DEG` 刻みのビンに振り分け、ビン内で
    //        - `d_near` が **最小**（最も富士山寄り）の (lon, lat) を採用
    //        - `d_far`  が **最大**（最も富士山から遠い）の (lon, lat) を採用
    //   3. ビンを方位昇順に並べ、far 昇順 + near 降順でリング化
    //
    // 効果:
    //   - 出力頂点数 ≈ 2 × (使用ビン数) で激減（典型 ~1500 → ~120）
    //   - 各方位 1 ペアのみなので、ソート後のリングが原理的に滑らかになる
    //   - 「全時刻の near 最小・far 最大」を取るため `point ⊆ polygon` の不変条件は
    //     維持される（包絡を広げる方向の集約のみ）
    //   - 時刻幅が遠端ほど大きいため、自然と「遠いほど大雑把」な精度配分になる
    //
    // 観測者視点 az_diff フィルタは候補点採用時に従来通り適用し、point/range と判定基準を揃える。
    // パディング `AZ_FROM_FUJI_PADDING_DEG` は富士山視点と観測者視点のズレ吸収のための余裕として残す。
    let az_band = AZIMUTH_THRESHOLD + AZ_FROM_FUJI_PADDING_DEG;
    const AZ_SAMPLES: usize = 9;

    // EdgeSample: 観測者視点フィルタ通過済みの (時刻, az_from_fuji) ペア。
    // 最終的な near/far 距離計算はビン代表方位で再実施するため、ここでは座標を持たない。
    struct EdgeSample {
        naive_time: chrono::NaiveDateTime,
        az_from_fuji_deg: f64,
    }

    let mut samples: Vec<EdgeSample> = Vec::new();

    let start_offset_seconds = -(OBSERVATION_OFFSET_HOURS * 60 * 60);
    let end_offset_seconds = 0;
    let step_seconds = CALCULATION_INTERVAL_SECONDS;

    let mut offset_seconds = start_offset_seconds;
    while offset_seconds <= end_offset_seconds {
        let offset = chrono::Duration::seconds(offset_seconds);
        let Some(current_time) = base_time.checked_add_signed(offset) else {
            offset_seconds += step_seconds;
            continue;
        };

        let Some(center_az_from_fuji_deg) = estimate_center_az_from_fuji_for_time(current_time)
        else {
            offset_seconds += step_seconds;
            continue;
        };

        for i in 0..AZ_SAMPLES {
            let t = if AZ_SAMPLES == 1 {
                0.0
            } else {
                (i as f64) / ((AZ_SAMPLES - 1) as f64)
            };
            let az = (center_az_from_fuji_deg + (-az_band + 2.0 * az_band * t)).rem_euclid(360.0);

            let Some(d_near) =
                solve_distance_for_altitude_diff(current_time, az, ELEVATION_THRESHOLD)
            else {
                continue;
            };
            let Some(d_far) =
                solve_distance_for_altitude_diff(current_time, az, -ELEVATION_THRESHOLD)
            else {
                continue;
            };

            // 観測者視点 az_diff で事後フィルタ。near と far の両方が閾値内のときだけ
            // 採用する。片側のみ通過するケースは方位帯の端で稀に発生するが、リング構築時の
            // 自己交差 / 穴あきリスクを避けるために両側通過を必須とする。
            let (near_lat, near_lon) =
                geo::calc_destination_point(FUJI_LATITUDE, FUJI_LONGITUDE, az, d_near);
            if observer_az_diff_deg(near_lat, near_lon, current_time) >= AZIMUTH_THRESHOLD {
                continue;
            }
            let (far_lat, far_lon) =
                geo::calc_destination_point(FUJI_LATITUDE, FUJI_LONGITUDE, az, d_far);
            if observer_az_diff_deg(far_lat, far_lon, current_time) >= AZIMUTH_THRESHOLD {
                continue;
            }

            samples.push(EdgeSample {
                naive_time: current_time,
                az_from_fuji_deg: az,
            });
        }

        offset_seconds += step_seconds;
    }

    if samples.is_empty() {
        return Vec::new();
    }

    // 方位ビン集約: 360°/0° 跨ぎを安全に扱うため、全サンプルの **循環平均** を anchor として
    // [-180°, 180°) の相対角でビンキー（i32）を決める。`BTreeMap<i32, _>` がキー昇順に
    // 自動整列するため、追加のソートは不要。
    // 循環平均を使う理由: 「最初のサンプル」を anchor にすると `samples[0]` が方位帯の端に
    // ある場合に anchor が偏り、ビン割り当てが非決定的になる。観測帯の中心付近に anchor が
    // 来るよう循環平均を採用することで、`anchor` 由来のビン境界の揺らぎを抑える。
    let anchor = {
        let (sin_sum, cos_sum) = samples.iter().fold((0.0_f64, 0.0_f64), |(s, c), x| {
            let r = x.az_from_fuji_deg.to_radians();
            (s + r.sin(), c + r.cos())
        });
        sin_sum.atan2(cos_sum).to_degrees().rem_euclid(360.0)
    };
    let relative_az = |az: f64| -> f64 {
        let d = (az - anchor).rem_euclid(360.0);
        if d > 180.0 { d - 360.0 } else { d }
    };
    let bin_key = |az: f64| (relative_az(az) / AZ_BIN_WIDTH_DEG).round() as i32;

    // 各サンプルをビンキーごとの時刻集合に振り分ける。`BTreeSet` を使うのは、同一時刻が
    // 複数の方位サンプルから同じビンに落ちて重複登録されると、集約フェーズで同じ二分法を
    // 余分に呼び出すため。重複排除で集約フェーズの計算量を削減する。
    let mut bin_times: std::collections::BTreeMap<
        i32,
        std::collections::BTreeSet<chrono::NaiveDateTime>,
    > = std::collections::BTreeMap::new();
    for s in &samples {
        bin_times
            .entry(bin_key(s.az_from_fuji_deg))
            .or_default()
            .insert(s.naive_time);
    }

    // BinEdge: 方位ビンの代表方位における境界点ペア。
    // - near: 全時刻のうち最も富士山に近い（near 最小距離）境界の (lon, lat)
    // - far:  全時刻のうち最も富士山から遠い（far 最大距離）境界の (lon, lat)
    struct BinEdge {
        near: (f64, f64),
        far: (f64, f64),
    }

    // 各ビンについて、ビン中央方位（= 代表方位）で `solve_distance_for_altitude_diff` を
    // ビン内の各時刻について再計算し、ビン全時刻を通じた最近端 / 最遠端の境界点を採用する。
    // サンプル時の方位（az_band 範囲内のいずれか）ではなく代表方位で距離を解き直すことで、
    // 隣接ビン間の near/far が同じ方位刻みに揃い、リングが滑らかな折れ線になる。
    //
    // 観測者視点 az_diff フィルタはサンプリング時に既に通過済みで、ここでは再適用しない
    // （代表方位とサンプル時方位の差は最大 az_band 程度で、再フィルタはビンを過剰に消失
    // させ point の取りこぼしを招くため）。polygon は本来 over-approximation であり、
    // point/range の判定とは個別に成立する。
    let mut bins: std::collections::BTreeMap<i32, BinEdge> = std::collections::BTreeMap::new();
    for (key, times) in &bin_times {
        let bin_center_az = (anchor + (*key as f64) * AZ_BIN_WIDTH_DEG).rem_euclid(360.0);

        let mut min_d_near = f64::INFINITY;
        let mut max_d_far = f64::NEG_INFINITY;
        let mut found_near: Option<(f64, f64)> = None;
        let mut found_far: Option<(f64, f64)> = None;

        for t in times {
            let Some(d_near) =
                solve_distance_for_altitude_diff(*t, bin_center_az, ELEVATION_THRESHOLD)
            else {
                continue;
            };
            let Some(d_far) =
                solve_distance_for_altitude_diff(*t, bin_center_az, -ELEVATION_THRESHOLD)
            else {
                continue;
            };

            // 単調性の防御: 想定では d_near < d_far（高度差が距離に対して単調減少のため）だが、
            // 方位帯の極端な端や日付境界では稀に逆転しうる。逆転したサンプルを採用するとリング
            // のトポロジーが破綻するため、ここで弾く。
            if d_near >= d_far {
                continue;
            }

            let (near_lat, near_lon) =
                geo::calc_destination_point(FUJI_LATITUDE, FUJI_LONGITUDE, bin_center_az, d_near);
            let (far_lat, far_lon) =
                geo::calc_destination_point(FUJI_LATITUDE, FUJI_LONGITUDE, bin_center_az, d_far);

            if d_near < min_d_near {
                min_d_near = d_near;
                found_near = Some((near_lon, near_lat));
            }
            if d_far > max_d_far {
                max_d_far = d_far;
                found_far = Some((far_lon, far_lat));
            }
        }

        if let (Some(near), Some(far)) = (found_near, found_far) {
            bins.insert(*key, BinEdge { near, far });
        } else {
            // ビン内の全時刻で代表方位の二分法が解なし、もしくは単調性逆転で全棄却。
            // 該当方位帯にリングの「穴」ができるため、可視化のため debug ログに残す。
            debug!(
                bin_key = key,
                bin_center_az, "polygon: ビン内全時刻で境界点解なし、ビンをスキップ"
            );
        }
    }

    // 3 ビン未満では Polygon として有効なリングが構成できず、`vec2geojson` が
    // Point/LineString を返すため内包判定が偽陰性になる。実用上は起きない。
    if bins.len() < 3 {
        return Vec::new();
    }

    // far 昇順 + near 降順で閉リングを構築。バナナが東向きの場合、
    // 「東側の遠端を南→北」「西側の近端を北→南」となり外周は CCW を取る。
    let mut ring: Vec<(f64, f64)> = Vec::with_capacity(bins.len() * 2);
    for b in bins.values() {
        ring.push(b.far);
    }
    for b in bins.values().rev() {
        ring.push(b.near);
    }
    ring
}

#[cfg(test)]
mod tests {
    use super::*;

    /// detect_alignment_picks_minimum_sigma_within_threshold
    /// `detect_alignment_for_location` のベストマッチ方式（=「最初に閾値内に入った時刻」ではなく
    /// 「閾値内候補のうち sigma 最小の時刻」を選ぶ）が将来のリファクタで壊れないことを直接保証する。
    /// 同じ時間窓を独立に走査して閾値内候補と各 sigma を再構築し、選ばれた時刻が窓内最小 sigma の
    /// 候補と一致することを確認する。
    #[test]
    fn detect_alignment_picks_minimum_sigma_within_threshold() {
        // 既知ヒット地点（point_hit_location_is_inside_polygon と同じ座標）
        let cases: &[(f64, f64, i16, u8, u8)] = &[
            (35.703_999_654_324_605, 139.599_431_050_877_48, 2025, 11, 18),
            (35.703_999_654_324_605, 139.587_431_050_877_48, 2025, 11, 20),
        ];

        let mut saw_multi_candidate = false;

        for &(lat, lon, year, month, day) in cases {
            let alignment = detect_alignment_for_location(lat, lon, year, month, day, false)
                .expect("known location should detect an alignment");

            let sunset_naive = normalize_sunset_naive_datetime(year, month, day, lat, lon)
                .expect("sunset normalization should succeed");
            let start = sunset_naive - chrono::Duration::hours(OBSERVATION_OFFSET_HOURS);
            let loop_n = OBSERVATION_OFFSET_HOURS * 60 * 60 / CALCULATION_INTERVAL_SECONDS;
            let tz = FixedOffset::east_opt(9 * 3600).expect("valid JST offset");

            let fuji_az_deg = geo::calc_azimuth(lat, lon, FUJI_LATITUDE, FUJI_LONGITUDE);
            let fuji_alt_deg =
                geo::calc_altitude(lat, lon, 0.0, FUJI_LATITUDE, FUJI_LONGITUDE, FUJI_ALTITUDE);

            let mut candidates: Vec<(i64, f64)> = Vec::new();
            for i in 0..=loop_n {
                let t = start + chrono::Duration::seconds(i * CALCULATION_INTERVAL_SECONDS);
                let (sun_az, sun_alt) = sun::calc_sun_az_and_alt(
                    t.year() as i16,
                    t.month() as u8,
                    t.day() as u8,
                    t.hour() as u8,
                    t.minute() as u8,
                    t.second() as f64,
                    9.0,
                    lat,
                    lon,
                );
                let az_diff = angular_diff_deg(sun_az, fuji_az_deg);
                let alt_diff = (sun_alt - fuji_alt_deg).abs();
                if az_diff >= AZIMUTH_THRESHOLD || alt_diff >= ELEVATION_THRESHOLD {
                    continue;
                }
                let sigma = angular_separation_deg(sun_az, sun_alt, fuji_az_deg, fuji_alt_deg);
                let unix_time = match tz.from_local_datetime(&t) {
                    LocalResult::Single(dt) => dt.timestamp(),
                    LocalResult::Ambiguous(dt1, _) => dt1.timestamp(),
                    LocalResult::None => continue,
                };
                candidates.push((unix_time, sigma));
            }

            assert!(
                !candidates.is_empty(),
                "閾値内候補が1つも無いケースは想定外 (lat={lat}, lon={lon}, {year:04}-{month:02}-{day:02})"
            );

            if candidates.len() >= 2 {
                saw_multi_candidate = true;
            }

            let (best_time, best_sigma) = candidates
                .iter()
                .copied()
                .min_by(|a, b| a.1.partial_cmp(&b.1).expect("sigma is finite"))
                .expect("non-empty candidates");

            assert_eq!(
                alignment.unix_time, best_time,
                "選ばれた時刻は閾値内候補の中で sigma 最小である必要がある (lat={lat}, lon={lon}, {year:04}-{month:02}-{day:02})"
            );

            for &(_, sigma) in &candidates {
                assert!(
                    sigma >= best_sigma,
                    "best より小さい sigma が在閾値内に存在してはならない"
                );
            }
        }

        assert!(
            saw_multi_candidate,
            "テストの意義のため、少なくとも1ケースは複数の閾値内候補を含むべき"
        );
    }
}
