use chrono::{DateTime, Utc};
use clap::{Parser, Subcommand};
use colored::Colorize;
use tracing::info;

/// main
/// CLI エントリポイント
/// コマンド例:
/// ```sh
/// # 単一地点での判定
/// dfuji-cli point --latitude 35.697638293191105 --longitude 139.58268645295962 --year 2025 --month 11 --day 18
/// # 緯度・経度の範囲を走査
/// dfuji-cli range --lat-min 35.6 --lat-max 35.8 --lat-step 0.01 --lon-min 139.5 --lon-max 139.7 --lon-step 0.01 --year 2025 --month 11 --day 18
/// # ポリゴン出力
/// dfuji-cli polygon --year 2025 --month 11 --day 18
/// ```
fn main() {
    let cli = Cli::parse();
    init_tracing(cli.verbose);

    match cli.command {
        Commands::Point {
            latitude,
            longitude,
            year,
            month,
            day,
        } => {
            if !validate_lat_lon(latitude, longitude) {
                eprintln!(
                    "入力値の緯度/経度が範囲外です。latitude は -90～90、longitude は -180～180 の範囲にしてください。"
                );
                std::process::exit(1);
            }
            match dfuji_app::point(latitude, longitude, year, month, day) {
                Some(unix_time) => {
                    let ts = DateTime::<Utc>::from_timestamp(unix_time, 0)
                        .expect("valid UNIX timestamp");
                    info!(
                        unix_time,
                        iso8601 = %ts.to_rfc3339(),
                        "Diamond Fuji alignment detected"
                    );

                    // println!でcolored表示するために、時間を代入したメッセージを作っておく
                    let msg = format!(
                        "🟢 Diamond Fuji is visible at UNIX time {unix_time} ({})",
                        ts.with_timezone(&chrono::Local)
                            .format("%Y-%m-%d %H:%M:%S %Z")
                    );

                    println!("{}", msg.green());
                }
                None => {
                    println!(
                        "{}",
                        "❌️ Diamond Fuji alignment not detected for the provided point.".red(),
                    );
                }
            }
        }
        Commands::Range {
            lat_min,
            lat_max,
            lat_step,
            lon_min,
            lon_max,
            lon_step,
            year,
            month,
            day,
        } => {
            let matches = dfuji_app::range(
                lat_min, lat_max, lat_step, lon_min, lon_max, lon_step, year, month, day,
            );

            if matches.is_empty() {
                println!(
                    "{}",
                    "❌️ No Diamond Fuji alignments detected in the specified range.".red(),
                );
            } else {
                println!("{}", "🟢 Diamond Fuji alignments found:".green());
                println!("Found {} candidate(s):", matches.len());
                for (lat, lon, unix_time) in matches {
                    let ts = DateTime::<Utc>::from_timestamp(unix_time, 0)
                        .expect("valid UNIX timestamp");
                    println!(
                        "lat={lat:.5}, lon={lon:.5}, unix={unix_time} ({} Local)",
                        ts.with_timezone(&chrono::Local)
                            .format("%Y-%m-%d %H:%M:%S %Z")
                    );
                }
            }
        }
        Commands::Polygon { year, month, day } => {
            let geojson = dfuji_app::polygon(year, month, day);
            println!("{}", geojson);
        }
    }
}

/// CLI 引数の定義
#[derive(Parser)]
#[command(author, version, about = "Diamond Fuji visibility CLI", long_about = None)]
struct Cli {
    /// ログ出力の詳細度 ( -v で debug, -vv で trace )
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,

    #[command(subcommand)]
    command: Commands,
}

/// サブコマンドの定義
#[derive(Subcommand)]
enum Commands {
    /// 単一地点でダイヤモンド富士が見えるかを判定
    Point {
        /// 観測地点の緯度 (度)
        #[arg(long)]
        latitude: f64,
        /// 観測地点の経度 (度)
        #[arg(long)]
        longitude: f64,
        /// 観測する年 (例: 2025)
        #[arg(long)]
        year: i16,
        /// 観測する月 (1-12)
        #[arg(long)]
        month: u8,
        /// 観測する日 (1-31)
        #[arg(long)]
        day: u8,
    },
    /// 緯度・経度の範囲を走査して候補を列挙
    Range {
        #[arg(long)]
        lat_min: f64,
        #[arg(long)]
        lat_max: f64,
        #[arg(long, default_value_t = 0.01)]
        lat_step: f64,
        #[arg(long)]
        lon_min: f64,
        #[arg(long)]
        lon_max: f64,
        #[arg(long, default_value_t = 0.01)]
        lon_step: f64,
        #[arg(long)]
        year: i16,
        #[arg(long)]
        month: u8,
        #[arg(long)]
        day: u8,
    },
    /// ダイヤモンド富士の観測可能な範囲をポリゴンとしてGeoJSON形式で出力
    Polygon {
        /// 観測する年 (例: 2025)
        #[arg(long)]
        year: i16,
        /// 観測する月 (1-12)
        #[arg(long)]
        month: u8,
        /// 観測する日 (1-31)
        #[arg(long)]
        day: u8,
    },
}

fn init_tracing(verbosity: u8) {
    use tracing_subscriber::EnvFilter;

    let default_level = match verbosity {
        0 => "error",
        1 => "info",
        2 => "debug",
        _ => "trace",
    };

    let env_filter = std::env::var("RUST_LOG")
        .ok()
        .unwrap_or_else(|| default_level.to_string());

    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(env_filter))
        .with_target(false)
        .try_init();
}

/// 緯度・経度が有効な範囲内かどうかを検証するヘルパー関数。
///
/// 単一地点(Point)コマンドの入力値バリデーションに加えて、
/// 緯度・経度の範囲(Range)指定時の各点検証などにも再利用できる。
///
/// # 引数
/// * `lat` - 検証対象の緯度（度数法）。有効範囲は -90.0〜90.0。
/// * `lon` - 検証対象の経度（度数法）。有効範囲は -180.0〜180.0。
///
/// # 戻り値
/// * `true` - 緯度・経度がいずれも有効範囲内の場合。
/// * `false` - いずれかが有効範囲外の場合。
fn validate_lat_lon(lat: f64, lon: f64) -> bool {
    lat.abs() <= 90.0 && lon.abs() <= 180.0
}
