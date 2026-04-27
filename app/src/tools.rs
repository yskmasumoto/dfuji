use crate::app::Alignment;
use chrono::{Datelike, FixedOffset, LocalResult, TimeZone, Timelike};
use dfuji_core::{
    AZIMUTH_THRESHOLD, BISECTION_HIGH_DISTANCE, BISECTION_LOW_DISTANCE, BISECTION_MAX_ITER,
    CALCULATION_INTERVAL_SECONDS, ELEVATION_THRESHOLD, FUJI_ALTITUDE, FUJI_LATITUDE,
    FUJI_LONGITUDE, OBSERVATION_OFFSET_HOURS,
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
        if f_mid.abs() < ELEVATION_THRESHOLD {
            return Some(mid);
        }

        if f_low * f_mid <= 0.0 {
            high = mid;
        } else {
            low = mid;
            f_low = f_mid;
        }
    }

    None
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

#[cfg(test)]
pub(crate) fn debug_estimate_center_az_from_fuji_for_time(
    current_time: chrono::NaiveDateTime,
) -> Option<f64> {
    estimate_center_az_from_fuji_for_time(current_time)
}

/// cross
/// 3点の外積を計算するヘルパー関数
/// # Arguments
/// * `o` - 原点の座標 (x, y)
/// * `a` - 点Aの座標 (x, y)
/// * `b` - 点Bの座標 (x, y)
/// # Returns
/// * 外積の値
fn cross(o: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    (a.0 - o.0) * (b.1 - o.1) - (a.1 - o.1) * (b.0 - o.0)
}

/// 重複点を除去する際に用いる座標誤差の許容値。
///
/// 緯度経度などの地理座標は f64 で表現しても、実データの精度はせいぜい
/// 1e-8 度（赤道付近で約ミリメートルオーダー）程度であり、1e-12 度未満の
/// 差分は浮動小数点の丸め誤差レベルとみなせるため、同一点として扱う。
const DEDUP_TOLERANCE: f64 = 1e-12;

/// convex_hull
/// 2D点群の凸包を計算する関数（Andrew's monotone chainアルゴリズム）
/// # Arguments
/// * `points` - 2D点群のベクタ
/// # Returns
/// * 凸包を構成する点群のベクタ
fn convex_hull(mut points: Vec<(f64, f64)>) -> Vec<(f64, f64)> {
    points.sort_by(|a, b| {
        a.0.partial_cmp(&b.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
    });
    points.dedup_by(|a, b| {
        (a.0 - b.0).abs() < DEDUP_TOLERANCE && (a.1 - b.1).abs() < DEDUP_TOLERANCE
    });

    if points.len() <= 2 {
        return points;
    }

    let mut lower: Vec<(f64, f64)> = Vec::new();
    for p in &points {
        while lower.len() >= 2 {
            let n = lower.len();
            if cross(lower[n - 2], lower[n - 1], *p) <= 0.0 {
                lower.pop();
            } else {
                break;
            }
        }
        lower.push(*p);
    }

    let mut upper: Vec<(f64, f64)> = Vec::new();
    for p in points.iter().rev() {
        while upper.len() >= 2 {
            let n = upper.len();
            if cross(upper[n - 2], upper[n - 1], *p) <= 0.0 {
                upper.pop();
            } else {
                break;
            }
        }
        upper.push(*p);
    }

    if lower.len() > 1 {
        lower.pop();
    }
    if upper.len() > 1 {
        upper.pop();
    }
    lower.extend(upper);
    lower
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

    // ベストマッチ追跡: 球面角距離が最小の時刻を保持する
    let mut best: Option<(i64, f64, f64, f64)> = None; // (unix_time, az_diff, alt_diff, sigma)

    for i in 0..=loop_n {
        let current_time =
            sunset_time_minus_2h + chrono::Duration::seconds(i * CALCULATION_INTERVAL_SECONDS);
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
                current_time = %current_time_str,
                sun_azimuth_deg = %format!("{:.2}", sun_az_deg),
                sun_altitude_deg = %format!("{:.2}", sun_alt_deg),
                fuji_azimuth_deg = %format!("{:.2}", fuji_az_deg),
                fuji_altitude_deg = %format!("{:.2}", fuji_alt_deg),
                angular_separation_deg = %format!("{:.4}", sigma),
                "Apparent positions"
            );
        }

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

        match best {
            None => best = Some((unix_time, az_diff, alt_diff, sigma)),
            Some((_, _, _, best_sigma)) if sigma < best_sigma => {
                best = Some((unix_time, az_diff, alt_diff, sigma));
            }
            _ => {}
        }
    }

    let (unix_time, az_diff, alt_diff, sigma) = best?;

    if az_diff >= AZIMUTH_THRESHOLD || alt_diff >= ELEVATION_THRESHOLD {
        if log_enabled {
            info!(
                latitude = obs_lat,
                longitude = obs_lon,
                best_az_diff = %format!("{:.3}", az_diff),
                best_alt_diff = %format!("{:.3}", alt_diff),
                best_angular_separation_deg = %format!("{:.4}", sigma),
                "Diamond Fuji alignment not detected (best candidate exceeds threshold)"
            );
        }
        return None;
    }

    if log_enabled {
        info!(
            latitude = obs_lat,
            longitude = obs_lon,
            azimuth_diff = %format!("{:.3}", az_diff),
            altitude_diff = %format!("{:.3}", alt_diff),
            angular_separation_deg = %format!("{:.4}", sigma),
            unix_time,
            "Diamond Fuji alignment detected"
        );
    }
    Some(Alignment {
        unix_time,
        az_diff,
        alt_diff,
    })
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

    let alt_deltas = [-ELEVATION_THRESHOLD, ELEVATION_THRESHOLD];

    // time×az_from_fuji のサンプル点群から凸包を取って over-approximation する。
    // 「時刻によって許容領域が回転/移動する」ケースで、単純な time-sweep リングは
    // under-approx になりやすいため（既知 point-hit の取りこぼし対策）。
    // point() の閾値は「観測点での az_diff」だが、polygon 生成は「富士山から見た方位 (az_from_fuji)」で近似する。
    // この2つは一致しないため、過小評価を避けるための最小限のパディングを入れる。
    // 富士山から見た方位で近似する際の安全側パディング[deg]。
    // 0.2deg は、実測ケースでの az_diff と az_from_fuji のズレを十分に覆いつつ、
    // パディングを増やしすぎて領域が過度に膨らまない（false positive が増えすぎない）
    // 範囲として経験的に決めている。
    const AZ_FROM_FUJI_PADDING_DEG: f64 = 0.2;
    let az_band = AZIMUTH_THRESHOLD + AZ_FROM_FUJI_PADDING_DEG;
    // 方位帯域 [center_az_from_fuji_deg - az_band, +az_band] を線形サンプリングする分割数。
    // 9 サンプルとすることで、約 4deg 幅の帯域に対して ≒0.5deg 間隔のサンプル密度となり、
    // 計算コスト（distance 解を 2 回ずつ求めるコスト）を抑えつつ、az 方向の穴を作りにくい
    // バランスを取っている。
    const AZ_SAMPLES: usize = 9;

    let mut candidates: Vec<(f64, f64)> = Vec::new();

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

            let Some(d_top) = solve_distance_for_altitude_diff(current_time, az, alt_deltas[1])
            else {
                continue;
            };
            let Some(d_bottom) = solve_distance_for_altitude_diff(current_time, az, alt_deltas[0])
            else {
                continue;
            };

            let (top_lat, top_lon) =
                geo::calc_destination_point(FUJI_LATITUDE, FUJI_LONGITUDE, az, d_top);
            let (bottom_lat, bottom_lon) =
                geo::calc_destination_point(FUJI_LATITUDE, FUJI_LONGITUDE, az, d_bottom);

            candidates.push((top_lon, top_lat));
            candidates.push((bottom_lon, bottom_lat));
        }

        offset_seconds += step_seconds;
    }

    convex_hull(candidates)
}
