mod helpers;

use astro::*;

/// calc_sun_az_and_alt
/// ## 概要
/// 指定した日時・観測地点における太陽の方位角
/// および高度角を計算する関数
/// # Arguments
/// * `year` - 年
/// * `month` - 月
/// * `day` - 日
/// * `hour` - 時
/// * `minute` - 分
/// * `second` - 秒
/// * `time_zone` - タイムゾーン（例: 日本なら9.0）
/// * `obs_lat_deg` - 観測地点の緯度（度）
/// * `obs_lon_deg` - 観測地点の経度（度）
/// # Returns
/// * `(f64, f64)` - (方位角（度）, 高度角（度）)
#[allow(clippy::too_many_arguments)]
pub fn calc_sun_az_and_alt(
    year: i16,
    month: u8,
    day: u8,
    hour: u8,
    minute: u8,
    second: f64,
    time_zone: f64,
    obs_lat_deg: f64,
    obs_lon_deg: f64,
) -> (f64, f64) {
    // 経度の符号を逆にする処理
    let obs_lon_deg = -obs_lon_deg;

    // 観測者の緯度経度をラジアンに変換
    let obs_lat = obs_lat_deg.to_radians();
    let obs_lon = obs_lon_deg.to_radians();

    // date
    let day_of_month = time::DayOfMonth {
        day,
        hr: hour,
        min: minute,
        sec: second,
        time_zone,
    };

    let date = time::Date {
        year,
        month,
        decimal_day: helpers::my_decimal_day(&day_of_month),
        cal_type: time::CalType::Gregorian,
    };

    // julian day
    let julian_day = time::julian_day(&date);

    // nutation and true obliquity
    let (nutation_long, true_obliquity) = helpers::calc_nut_and_oblq(julian_day);

    // equatorial coordinates on the date
    let (td_asc, td_dec) = helpers::get_asc_and_dec(julian_day, nutation_long, true_obliquity);

    // hour angle
    let mn_sidr = time::mn_sidr(julian_day); // 平均恒星時
    let apparent_sidereal_time = time::apprnt_sidr(mn_sidr, nutation_long, true_obliquity);
    let hour_angle = coords::hr_angl_frm_observer_long(apparent_sidereal_time, obs_lon, td_asc);

    // azimuth
    let azimuth = coords::az_frm_eq(hour_angle, td_dec, obs_lat);

    // altitude
    let mut altitude = coords::alt_frm_eq(hour_angle, td_dec, obs_lat);

    // refrac
    let r = if altitude.to_degrees() > 15.0 {
        atmos::refrac_frm_true_alt_15(altitude)
    } else {
        atmos::refrac_frm_true_alt(altitude)
    };

    altitude += r; // 大気屈折を考慮

    // radian to degree conversion
    // 求まるazimuthは南が0度、西が90度のため、180度を足して北が0度、東が90度になるようにする
    let azimuth_deg = azimuth.to_degrees() + 180.0;
    let altitude_deg = altitude.to_degrees();

    (azimuth_deg, altitude_deg)
}

pub fn calc_sunset_time(
    year: i16,
    month: u8,
    day: u8,
    obs_lat_deg: f64,
    obs_lon_deg: f64,
) -> (i64, i64, i64) {
    // 経度の符号を逆にする処理
    let obs_lon_deg = -obs_lon_deg;

    // 観測者の緯度経度をラジアンに変換
    let obs_lat = obs_lat_deg.to_radians();
    let obs_lon = obs_lon_deg.to_radians();

    // 今日の0時、昨日、明日の赤道座標を計算
    // julian day
    let day_of_month_0h = time::DayOfMonth {
        day,
        hr: 0,
        min: 0,
        sec: 0.0,
        time_zone: 9.0,
    };

    let date_0h = time::Date {
        year,
        month,
        decimal_day: helpers::my_decimal_day(&day_of_month_0h),
        cal_type: time::CalType::Gregorian,
    };

    // ユリウス日を計算
    let julian_day_0h = time::julian_day(&date_0h);

    // 0時のnutationの計算
    let (nutation_long_0h, nutation_oblq_0h) = nutation::nutation(julian_day_0h);

    // 0時の真の黄道傾斜角の計算
    let mean_oblq_0h = ecliptic::mn_oblq_IAU(julian_day_0h);
    let true_obliquity_0h = mean_oblq_0h + nutation_oblq_0h;

    // 0時の視恒星時の計算
    let mn_sidr_0h = time::mn_sidr(julian_day_0h); // 平均恒星時
    let apparent_sidereal_time_0h =
        time::apprnt_sidr(mn_sidr_0h, nutation_long_0h, true_obliquity_0h);

    // 0時の黄道座標から赤道座標への変換 (視黄経と真の黄道傾斜角を使用)
    let (td_asc_0h, td_dec_0h) =
        helpers::get_asc_and_dec(julian_day_0h, nutation_long_0h, true_obliquity_0h);
    let (yd_asc, yd_dec) =
        helpers::get_asc_and_dec(julian_day_0h - 1.0, nutation_long_0h, true_obliquity_0h);
    let (tm_asc, tm_dec) =
        helpers::get_asc_and_dec(julian_day_0h + 1.0, nutation_long_0h, true_obliquity_0h);

    // 日没時刻の計算
    let (t_hour, t_min, t_sec) = transit::time(
        &transit::TransitType::Set,
        &transit::TransitBody::Sun,
        &coords::GeographPoint {
            long: obs_lon, // radians
            lat: obs_lat,  // radians
        },
        &coords::EqPoint {
            asc: yd_asc,
            dec: yd_dec,
        },
        &coords::EqPoint {
            asc: td_asc_0h,
            dec: td_dec_0h,
        },
        &coords::EqPoint {
            asc: tm_asc,
            dec: tm_dec,
        },
        apparent_sidereal_time_0h,
        69.0, // 世界時と地球時の差
        0.0,
    );

    const SECONDS_PER_DAY: i64 = 24 * 60 * 60;
    let total_seconds = (t_hour as f64 * 3600.0 + t_min as f64 * 60.0 + t_sec).round() as i64;
    let seconds_in_day = total_seconds.rem_euclid(SECONDS_PER_DAY);

    let hour = seconds_in_day / 3600;
    let minute = (seconds_in_day % 3600) / 60;
    let second = seconds_in_day % 60;

    (hour, minute, second) // 正規化した時、分、秒を返す
}
