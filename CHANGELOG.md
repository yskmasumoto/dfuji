# Changelog

このプロジェクトの主要な変更を記録する。フォーマットは [Keep a Changelog](https://keepachangelog.com/ja/1.1.0/) に準拠し、バージョニングは [Semantic Versioning](https://semver.org/lang/ja/) に従う。

## [Unreleased]

## [0.1.0-beta.2] - 2026-04-28

### Changed

- `polygon` の精度を改善: 凸包による over-approximation を **順序付き envelope リング** に置き換え、バナナ状の真の可視帯形状を保つようにした
- polygon 候補点に対し、**観測者視点の方位角差**（`point` / `range` と同じ判定基準）を実計算する事後フィルタを追加。これにより富士山視点サンプリング由来の過大評価が解消された
- 二分法（`solve_distance_for_altitude_diff`）の収束判定を厳格化（残差 `< ELEVATION_THRESHOLD (0.2°)` → `< 1e-4°`、区間幅 1 m のフォールバック追加）。距離精度が数 km レベルから数 m レベルに向上

### Added

- `BISECTION_RESIDUAL_TOLERANCE_DEG` / `BISECTION_INTERVAL_TOLERANCE_M` を `core` クレートに追加（既存 `BISECTION_*` 定数と同グループ）

### Fixed

- 富士山視点の方位角サンプリングが 360°/0° 境界を跨ぐ稀な日付で、polygon リングが自己交差する潜在バグを修正（anchor 基準の相対角でソート）

### Notes

- 公開 API は変更なし（`polygon()` のシグネチャと GeoJSON 出力フォーマットは互換）
- 既知の制約: 出力多角形の頂点数が増加（典型値 ~30 → ~1500）。実用に堪える頂点数への削減は次バージョン以降で検討

## [0.1.0-beta.1] - 2026-04-28

初回ベータリリース。ダイヤモンド富士の観測可能性を計算するライブラリ・CLI の最初の公開バージョン。

### Added

- ワークスペース構成（`core` / `geo` / `sun` / `app` / `cli`）と公開ライブラリ API
  - `dfuji_app::point` — 単一観測地点でのアライメント判定
  - `dfuji_app::range` — 緯度経度範囲のグリッドサンプリング探索
  - `dfuji_app::polygon` — 観測可能領域を凸包で近似した GeoJSON 出力
  - 戻り値の構造体 `Alignment { unix_time, az_diff, alt_diff }` / `RangeMatch { lat, lon, alignment }`
- `dfuji-cli` バイナリ（clap ベース）
  - `point` / `range` / `polygon` サブコマンド
  - `-v` / `-vv` / `-vvv` のログ詳細度切替（`tracing-subscriber` ベース）
- アルゴリズム
  - 日没 2 時間前～日没を 30 秒刻みでサンプリングするベストマッチ方式
  - 閾値（方位角・高度角ともに 0.2°）内候補のうち球面角距離 σ が最小の時刻を採用し、粗ステップ起因のカスケード偽陽性を排除
  - 富士山中心の方位帯ごとに二分法で観測可能距離を解いて凸包を取る over-approximation 型ポリゴン生成
- WGS84 楕円体に基づく方位角・高度角・距離計算（`dfuji-geo`）
- 大気屈折補正付き太陽位置計算（`dfuji-sun`、`astro` クレートのラッパ）
- 整合性回帰テスト（`point` ヒット地点が `polygon` に必ず内包されること、`range` でも一致することを検証）
- ベストマッチ不変条件のリグレッションテスト（`detect_alignment_picks_minimum_sigma_within_threshold`）
- リリース運用スクリプト `scripts/version_update.sh` / `scripts/release.sh`
- CI ワークフロー（fmt-check / clippy `-D warnings` / test / build）

[Unreleased]: https://github.com/yskmasumoto/dfuji/compare/v0.1.0-beta.2...HEAD
[0.1.0-beta.2]: https://github.com/yskmasumoto/dfuji/releases/tag/v0.1.0-beta.2
[0.1.0-beta.1]: https://github.com/yskmasumoto/dfuji/releases/tag/v0.1.0-beta.1
