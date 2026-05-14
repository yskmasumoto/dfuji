use astro::*;
/// # 概要
/// 観測日時を UTC に換算した十進表現へ変換するヘルパー関数。
/// `astro::time::decimal_day` では期待した値が得られなかったため自前実装として切り出している。
///
/// # 引数
/// - `day`: 日付・時刻・タイムゾーンを保持する `time::DayOfMonth` への参照。
///
/// # 戻り値
/// - UTC 基準の十進日。
pub(crate) fn my_decimal_day(day: &time::DayOfMonth) -> f64 {
    let day_fraction_local: f64 =
        (day.hr as f64) / 24.0 + (day.min as f64) / (24.0 * 60.0) + day.sec / (24.0 * 60.0 * 60.0);
    let local_decimal_day: f64 = (day.day as f64) + day_fraction_local;
    let utc_decimal_day: f64 = local_decimal_day - day.time_zone / 24.0;
    utc_decimal_day
}

/// # 概要
/// 指定したユリウス日・章動・黄道傾斜角を用いて太陽の赤経・赤緯を計算する。
///
/// # 引数
/// - `jd`: ユリウス日。
/// - `nutation_long`: 黄経方向の章動（ラジアン）。
/// - `true_obliquity`: 真の黄道傾斜角（ラジアン）。
///
/// # 戻り値
/// - `(f64, f64)`: 太陽の赤経・赤緯（ラジアン）。
pub(crate) fn get_asc_and_dec(jd: f64, nutation_long: f64, true_obliquity: f64) -> (f64, f64) {
    let (sun_ecl_point, _) = sun::geocent_ecl_pos(jd); // 平均黄道座標
    let sun_apparent_ecl_long = sun_ecl_point.long + nutation_long; // 太陽の視黄経

    let declination = coords::dec_frm_ecl(sun_apparent_ecl_long, sun_ecl_point.lat, true_obliquity);
    let ascension = coords::asc_frm_ecl(sun_apparent_ecl_long, sun_ecl_point.lat, true_obliquity);

    (ascension, declination)
}

/// # 概要
/// 与えられたユリウス日に対して章動と真の黄道傾斜角を求める。
///
/// # 引数
/// - `julian_day`: ユリウス日。
///
/// # 戻り値
/// - `(f64, f64)`: 黄経方向の章動と真の黄道傾斜角（ともにラジアン）。
pub(crate) fn calc_nut_and_oblq(julian_day: f64) -> (f64, f64) {
    // calculate nutation in longitude and obliquity
    let (nutation_long, nutation_oblq) = nutation::nutation(julian_day);

    // calculate mean obliquity of the ecliptic
    let mean_oblq = ecliptic::mn_oblq_IAU(julian_day);

    // calculate true obliquity of the ecliptic
    let true_obliquity = mean_oblq + nutation_oblq;

    (nutation_long, true_obliquity)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `my_decimal_day` が `time::DayOfMonth` を UTC 基準の十進日へ変換する。
    /// `astro::time::decimal_day` 由来の不整合を踏まえた自前実装の挙動を固定する。
    #[test]
    fn my_decimal_day_converts_local_time_to_utc_decimal_day() {
        let day = time::DayOfMonth {
            day: 21,
            hr: 11,
            min: 45,
            sec: 0.0,
            time_zone: 9.0,
        };
        let decimal_day = my_decimal_day(&day);
        let expected = 21.0 + 11.0 / 24.0 + 45.0 / (24.0 * 60.0) - 9.0 / 24.0;
        assert!((decimal_day - expected).abs() < 1e-9);
    }
}
