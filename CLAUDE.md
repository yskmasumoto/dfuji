# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## プロジェクト概要

ダイヤモンド富士（太陽が富士山頂に重なる現象）の観測可能性を計算する Rust 製ライブラリ・CLI。Cargo ワークスペースで 5 クレートに分割されている。

## 言語・コミュニケーション

- **コメント / コミットメッセージ / ドキュメント / PR 説明はすべて日本語**（`.github/copilot-instructions.md` で規定）
- 公開関数・モジュール・構造体には docstring（`///`）必須

## 開発コマンド

ツールチェーンは `rust-toolchain.toml` で **1.91.1 に固定**。`rust-analyzer-preview` / `rustfmt` / `clippy` が同梱される。

```bash
# ビルド
cargo build --workspace            # debug
cargo build --release              # release バイナリ → target/release/dfuji-cli

# テスト
cargo test --workspace             # 全テスト
cargo test -p dfuji-app            # クレート単位
cargo test -p dfuji-app point_invalid_date_returns_none   # 単一テスト
cargo test -p dfuji-app -- --ignored                       # #[ignore] 付きデバッグテストのみ

# Lint / フォーマット（CI と同じ条件）
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings

# CLI 実行
./target/release/dfuji-cli point --latitude 35.69 --longitude 139.58 --year 2025 --month 11 --day 18
./target/release/dfuji-cli range --lat-min 35.69 --lat-max 35.71 --lat-step 0.001 \
                                 --lon-min 139.57 --lon-max 139.62 --lon-step 0.001 \
                                 --year 2025 --month 11 --day 18
./target/release/dfuji-cli polygon --year 2025 --month 11 --day 18 > area.geojson

# ログ詳細度: -v=info, -vv=debug, -vvv=trace（RUST_LOG でも制御可）
```

CI（`.github/workflows/ci.yml`）は **fmt-check → clippy(-D warnings) → test → build** の順で全クレート対象に実行され、`cargo clippy --workspace -- -D warnings` を通すことが必須。

## アーキテクチャ

### ワークスペース構成と依存方向

```
cli  →  app  →  { core, geo, sun }
                  geo → core
                  sun は独立
```

| クレート | 役割 |
|---|---|
| `core` | 定数のみ（富士山座標、WGS84、閾値、計算間隔、二分法パラメータ）。**ロジックを置かない** |
| `geo`  | 地理座標計算（方位角・高度角・WGS84/ENU 距離・凸包・GeoJSON 出力） |
| `sun`  | 太陽位置計算（`astro` クレートのラッパ + 大気屈折補正）。**他クレートに依存しない** |
| `app`  | `point` / `range` / `polygon` の高レベル API。`tools.rs` に検出ロジック本体 |
| `cli`  | `clap` ベースの CLI バイナリ。引数パースとフォーマット出力のみ |

`Cargo.toml` の `[workspace.metadata]` に rust-analyzer 用の `linkedProjects` 設定がある。

### 層分離の方針（`.github/instructions/rust.instructions.md`）

- **Interface 層**（`cli/src/main.rs`）: 引数パース・出力整形のみ。計算は持たない
- **Domain 層**（`app/src/app.rs`, `app/src/tools.rs`, `geo`, `sun`）: 純粋関数中心。`#[instrument]` で観測点を入れつつ、フレームワーク非依存
- 単体テストは `app/src/app.rs` に集中。**polygon が候補点を内包しているか** などの整合性回帰テストが含まれる（`point_hit_location_is_inside_polygon`, `consistency_among_functions`）

### コア検出アルゴリズム（`app/src/tools.rs`）

`detect_alignment_for_location` が中心関数で、**ベストマッチ方式**を採用している:

1. 日没 2 時間前から日没までを `CALCULATION_INTERVAL_SECONDS = 30` 秒刻みでサンプリング
2. 各時刻で `az_diff < AZIMUTH_THRESHOLD` **かつ** `alt_diff < ELEVATION_THRESHOLD` を満たす候補のみを選別
3. 選別後の候補のうち**球面角距離 σ（`angular_separation_deg`）が最小の時刻を採用**
4. UNIX 変換は best 確定後に 1 回だけ実施（`BestCandidate` 構造体に `NaiveDateTime` を保持）

「最初に閾値内に入った時刻」ではなく「閾値内のうち σ 最小」を選ぶことで、30 秒ステップ起因のカスケード偽陽性を排除している。この不変条件は `tests::detect_alignment_picks_minimum_sigma_within_threshold` で直接守られているため、ループの早期打ち切り最適化などのリファクタ時はこのテストを必ず通すこと。

### ポリゴン生成（`tools::create_lonlat_vec` → `polygon`）

富士山中心の方位帯ごとに二分法（`BISECTION_LOW_DISTANCE` 〜 `BISECTION_HIGH_DISTANCE`）で「太陽高度＝富士高度±閾値」となる距離を解き、観測候補点群の凸包を取る over-approximation。`point` でヒットする地点が `polygon` に必ず含まれることが回帰テストで保証されている（`multiple_point_hit_locations_are_inside_polygon`）。

## リリース運用

`scripts/` 配下にバージョン管理スクリプトがある。crates.io への publish は行わない方針:

```bash
# 1. 全クレートの Cargo.toml の version を一括更新 + Cargo.lock 更新
./scripts/version_update.sh 0.1.0-beta.1
# 2. 内容を確認してコミット & push（main 上で）
git diff
git add Cargo.lock app/Cargo.toml cli/Cargo.toml core/Cargo.toml geo/Cargo.toml sun/Cargo.toml
git commit -m "chore: bump version to 0.1.0-beta.1"
git push origin main
# 3. annotated tag を打って push
./scripts/release.sh 0.1.0-beta.1
```

タグは **`v` プレフィックス付き SemVer**（`v0.1.0-beta.1` / `v0.1.0-rc.1` / `v0.1.0`）で統一。`release.sh` は「main 上 / 作業ツリー clean / origin と同期 / タグ重複なし」を必須化しているため、ステップ 2 を忘れるとステップ 3 で弾かれる安全装置になっている。

## 既知の留意点

- `range-accuracy-analysis.md` は手元の解析メモで意図的に未追跡
- `.vscode/` は無視対象（個人エディタ設定）
- `chrono` の `LocalResult::Ambiguous` は最古の候補を採用、`LocalResult::None` は `None` を返す（`tools.rs` の UNIX 変換）
- `tracing` は `app` 側で `#[instrument]` を貼り、`cli` 側で `tracing-subscriber` を初期化する分業
