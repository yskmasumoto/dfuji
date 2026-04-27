use dfuji::app;

fn init_tracing() {
    use tracing_subscriber::EnvFilter;

    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_target(false)
        .try_init();
}

fn main() {
    init_tracing();

    // 東京の緯度経度を指定
    // astroクレードの仕様で、経度は東側がマイナス、緯度は北側がプラス
    // let orig_lat: f64 = 35.6544; // 北緯35度
    // let orig_lon: f64 = 139.7447; // 東経139度
    // 井の頭公園駅の緯度経度
    let orig_lat: f64 = 35.697638293191105; // 北緯35度
    let orig_lon: f64 = 139.58268645295962; // 東経139度

    // 日付
    let year: i16 = 2025; // 年
    let month: u8 = 11; // 月
    let day: u8 = 18; // 日

    // 実行
    match app::point(orig_lat, orig_lon, year, month, day) {
        Some(alignment) => println!(
            "Diamond Fuji alignment detected at UNIX time {} (az_diff={:.3}°, alt_diff={:.3}°)",
            alignment.unix_time, alignment.az_diff, alignment.alt_diff
        ),
        None => println!("Diamond Fuji alignment not detected in the evaluated window."),
    }
}
