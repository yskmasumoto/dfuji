# Changelog

このプロジェクトの主要な変更を記録する。フォーマットは [Keep a Changelog](https://keepachangelog.com/ja/1.1.0/) に準拠し、バージョニングは [Semantic Versioning](https://semver.org/lang/ja/) に従う。

## [Unreleased]

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

[Unreleased]: https://github.com/yskmasumoto/dfuji/compare/v0.1.0-beta.1...HEAD
[0.1.0-beta.1]: https://github.com/yskmasumoto/dfuji/releases/tag/v0.1.0-beta.1
