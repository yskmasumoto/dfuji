# dfuji

ダイヤモンド富士（太陽が富士山頂に重なる現象）の観測可能性を計算する Rust 製ライブラリ・CLI。

任意の観測地点・日付に対して、

- 太陽の方位角・高度角（`astro` クレート + 大気屈折補正）
- 富士山頂の方位角・高度角（WGS84 楕円体 + ENU 座標系）

を計算し、両者のアライメントが閾値内に収まる時刻を探索します。

## 機能

| サブコマンド | 概要 |
|------------|------|
| `point`    | 単一地点・指定日でアライメントを判定し、最も差が小さい観測時刻を返す |
| `range`    | 緯度・経度の範囲をグリッドサンプリングし、観測候補を一括列挙 |
| `polygon`  | 観測可能領域を方位ビン集約で envelope リング化し、GeoJSON 形式のポリゴンとして出力 |

## プロジェクト構成

Cargo ワークスペースで、5 つのクレートに分割されています。

```
dfuji/
├── core/   定数・パラメータ（富士山座標、WGS84、閾値など）
├── geo/    地理座標計算（方位角・高度角・距離）
├── sun/    太陽位置計算（astro クレートのラッパ）
├── app/    point / range / polygon の高レベル API
└── cli/    clap ベースの CLI バイナリ
```

依存関係: `cli → app → {core, geo, sun}`、`geo → core`、`sun` は独立。

## ビルド

Rust 1.91.1 以上が必要（`Cargo.toml` の `rust-version` で最低バージョンを宣言）。

```sh
cargo build --release
```

## 使い方（CLI）

### 単一地点判定

```sh
./target/release/dfuji-cli point \
  --latitude 35.697638293191105 \
  --longitude 139.58268645295962 \
  --year 2025 --month 11 --day 18
```

出力例:
```
🟢 Diamond Fuji is visible at UNIX time 1763450316 (2025-11-18 16:18:36 +09:00)  az_diff=0.002°  alt_diff=0.004°
```

### 範囲走査

```sh
./target/release/dfuji-cli range \
  --lat-min 35.69 --lat-max 35.71 --lat-step 0.001 \
  --lon-min 139.57 --lon-max 139.62 --lon-step 0.001 \
  --year 2025 --month 11 --day 18
```

出力例（抜粋）:
```
🟢 Diamond Fuji alignments found:
Found 464 candidate(s):
lat=35.69500, lon=139.58300, unix=1763450313 (2025-11-18 16:18:33 +09:00 Local)  az_diff=0.041°  alt_diff=0.082°
...
```

### ポリゴン出力（GeoJSON）

```sh
./target/release/dfuji-cli polygon --year 2025 --month 11 --day 18 > area.geojson
```

GeoJSON `FeatureCollection` を標準出力に出力。`geojson.io` などにそのまま貼り付け可能。

### ログ詳細度

`-v` で `info`、`-vv` で `debug`、`-vvv` で `trace`。`RUST_LOG` 環境変数でも制御可能。

## 使い方（ライブラリ）

`Cargo.toml`:
```toml
[dependencies]
dfuji-app = { path = "path/to/dfuji/app" }
```

```rust
use dfuji_app::{point, range, polygon, Alignment, RangeMatch};

// 単一地点
if let Some(a) = point(35.6976, 139.5827, 2025, 11, 18) {
    println!("unix={} az_diff={:.3} alt_diff={:.3}", a.unix_time, a.az_diff, a.alt_diff);
}

// 範囲走査
let matches: Vec<RangeMatch> = range(35.69, 35.71, 0.001, 139.57, 139.62, 0.001, 2025, 11, 18);

// ポリゴン
let geojson: String = polygon(2025, 11, 18);
```

## アルゴリズム概要

- **時間窓**: 日没 2 時間前 ～ 日没を 30 秒刻みでサンプリング
- **判定指標**: 太陽中心と富士山頂の球面角距離 σ = arccos(sin(alt₁)sin(alt₂) + cos(alt₁)cos(alt₂)cos(Δaz))
- **ベストマッチ方式**: 閾値（`AZIMUTH_THRESHOLD=0.2°`, `ELEVATION_THRESHOLD=0.2°`）内の候補のうち σ が最小の時刻を採用。30 秒ステップ起因のカスケード偽陽性を排除
- **ポリゴン生成**: 富士山中心の方位帯ごとに、二分法で「太陽高度＝富士高度±閾値」となる距離 d_near / d_far を解き、`AZ_BIN_WIDTH_DEG` 刻みの方位ビンに集約。各ビン代表方位で全時刻を通じた近端最小・遠端最大の境界点を採用する envelope リング方式（`point ⊆ polygon` を維持）

## テスト

```sh
cargo test
```

`app/src/app.rs` に point / range / polygon の整合性テスト・回帰テストを含む。

## ライセンス

MIT License — [LICENSE](./LICENSE) を参照。
