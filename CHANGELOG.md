# Changelog

このプロジェクトの主要な変更を記録する。フォーマットは [Keep a Changelog](https://keepachangelog.com/ja/1.1.0/) に準拠し、バージョニングは [Semantic Versioning](https://semver.org/lang/ja/) に従う。

## [Unreleased]

## [0.1.0-beta.4] - 2026-04-29

### Changed

- `estimate_center_az_from_fuji_for_time` の戻り値を `Option<f64>` から `f64` に変更（後述 BUG-001 修正で失敗ケースが消えたため）。不要になった内部関数 `solve_distance_for_altitude_match` と定数 `FIXED_POINT_MAX_ITER` / `AZ_CONVERGENCE_THRESHOLD_DEG` を削除

### Added

- BUG-001 回帰テストとして、富士山から ~156 km 東の千葉点 (35.66, 140.41) / 2026-02-24 を `multiple_point_hit_locations_are_inside_polygon` に追加

### Fixed

- **BUG-001**: `polygon` が富士山から遠い東方面（千葉県本土以東）に伸びない症状を修正。`estimate_center_az_from_fuji_for_time` の固定点反復が `sun_az_from_observer + 180° = az_from_fuji` の自己一致点に収束する設計だったが、これは球面測地の forward bearing と back bearing のズレを **逆符号** で積み上げる動作であり、観測距離が伸びるほど center_az が真値から離れていた（d=156 km で約 1° のズレ、`AZ_FROM_FUJI_PADDING_DEG = 0.2°` ベースのサンプリング帯から外れる）。太陽は実質無限遠（1 AU）にあり観測者位置による視差は < 0.001° のため、固定点反復を撤廃して「富士山地点の太陽方位の真逆方向」を直接採用する形に簡素化した

## [0.1.0-beta.3] - 2026-04-28

### Changed

- `polygon` の出力頂点数を大幅削減（典型値 ~1500 → ~180）。アルゴリズムを **方位ビン集約** 方式に切替: 富士山視点方位を `AZ_BIN_WIDTH_DEG` (0.2°) 刻みのビンに振り分け、各ビン代表方位で全時刻を通じた近端最小・遠端最大の境界点を採用する形に変更。時刻シフト由来のギザギザが原理的に消え、リングが滑らかな envelope になる
- `point ⊆ polygon` の不変条件は維持（ビン内全時刻の包絡を取るため、リングが外側に広がる方向への集約のみ発生）
- ビン割り当て anchor を「最初のサンプル方位」から「全サンプル方位の循環平均」に変更し、`samples[0]` 依存の非決定性を排除
- 同一時刻の重複登録を `BTreeSet` で排除し、集約フェーズの二分法呼び出しを削減

### Added

- `dfuji_core::AZ_BIN_WIDTH_DEG` / `dfuji_core::AZ_FROM_FUJI_PADDING_DEG` を `core` クレートに追加（旧 `tools.rs` 内ローカル定数を昇格）

### Fixed

- 集約フェーズで `d_near >= d_far` となる単調性逆転サンプルを検出し棄却するガードを追加（リングのトポロジー破綻を防止）
- ビン全時刻で境界点解なしの場合に `tracing::debug` でログ出力するようにし、リングの「穴」を可視化

### Notes

- ビン代表方位での集約フェーズでは観測者視点 az_diff フィルタを再適用しない。サンプリング時に既に通過済みであり、再適用すると hit location 周辺のビンを過剰に消失させる原因となるため
- 「南北マージンの精度」は時刻幅と二分法残差で従来通り担保され、遠端ほど時刻幅が広いため自然に「遠いほど大雑把」な精度配分になる
- 既知の未修正バグや今後の方針は `ROADMAP.md` を参照

## [0.1.0-beta.2] - 2026-04-28

### Changed

- `polygon` の精度を改善: 凸包による over-approximation を **順序付き envelope リング** に置き換え、バナナ状の真の可視帯形状を保つようにした
- polygon 候補点に対し、**観測者視点の方位角差**（`point` / `range` と同じ判定基準）を実計算する事後フィルタを追加。これにより富士山視点サンプリング由来の過大評価が解消された
- 二分法（`solve_distance_for_altitude_diff`）の収束判定を厳格化（残差 `< ELEVATION_THRESHOLD (0.2°)` → `< 1e-4°`、区間幅 1 m のフォールバック追加）。距離精度が数 km レベルから数 m レベルに向上

### Added

- `BISECTION_RESIDUAL_TOLERANCE_DEG` / `BISECTION_INTERVAL_TOLERANCE_M` を `core` クレートに追加（既存 `BISECTION_*` 定数と同グループ）

### Fixed

- 富士山視点の方位角サンプリングが 360°/0° 境界を跨ぐ稀な日付で、polygon リングが自己交差する潜在バグを修正(anchor 基準の相対角でソート)

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

[Unreleased]: https://github.com/yskmasumoto/dfuji/compare/v0.1.0-beta.4...HEAD
[0.1.0-beta.4]: https://github.com/yskmasumoto/dfuji/releases/tag/v0.1.0-beta.4
[0.1.0-beta.3]: https://github.com/yskmasumoto/dfuji/releases/tag/v0.1.0-beta.3
[0.1.0-beta.2]: https://github.com/yskmasumoto/dfuji/releases/tag/v0.1.0-beta.2
[0.1.0-beta.1]: https://github.com/yskmasumoto/dfuji/releases/tag/v0.1.0-beta.1
