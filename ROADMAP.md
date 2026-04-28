# ROADMAP

dfuji プロジェクトの今後の対応方針・既知の課題・改善候補を整理する。完了したものは [CHANGELOG.md](./CHANGELOG.md) に移す運用。

---

## 既知バグ

### BUG-001: 時間ループ終端が日没ぴったり固定で、東方面の観測ラインを取りこぼす

**影響範囲:** `polygon` / `point` / `range` の 3 API すべて

**症状:**
- 2026-02-24 の `polygon` を出力すると、千葉県本土以東に伸びず羽田付近で途切れる
- ダイヤモンド・パール富士マップ（dpfm.creazy.net）で同日付を確認すると、千葉県を貫いて太平洋上まで観測ラインが伸びている
- `point` / `range` でも、観測時刻が「富士山地点の日没後」にあたる地点（富士山から東に離れた地点）で `None` を返す可能性

**原因:**
`app/src/tools.rs` の時間ループ終端 `end_offset_seconds = 0`（富士山地点の日没ちょうど）固定。
ダイヤモンド富士の観測時刻は「観測者から見て太陽が富士山頂と重なる瞬間」で、観測者位置によって富士山地点の日没時刻と数分〜数十分ズレる。富士山から遠い東方向の観測者ほど、観測時刻が日没後にずれる。

**修正方針候補:**

1. **案 A: 単純な終端拡張** — `OBSERVATION_AFTER_SUNSET_MINUTES` 等を `core` に追加し、ループ終端を `+N分` に拡張。最小変更だが朝側（日の出方向）の対称性は得られない。
2. **案 B: 「日没近傍 ±N 分」概念に整理** — `OBSERVATION_OFFSET_HOURS` を対称ウィンドウ（`OBSERVATION_WINDOW_BEFORE_HOURS` / `OBSERVATION_WINDOW_AFTER_HOURS`）に分離。
3. **案 C: 観測者地点の現地日没時刻基準に切替** — `point` / `range` は観測者位置の現地日没時刻を起点にする。`polygon` はビン代表方位ごとに観測者位置の日没時刻を起点にする。物理的に最も正しいが、構造変更が大きい。

**推奨:** まず案 A で範囲を伸ばして検証 → 問題が出たら案 B / C を検討。

**関連回帰テスト:**
- 修正後、東方面遠距離地点（千葉等）で `point()` がヒットする回帰テストを新規追加
- 既存の固定座標テスト（`multiple_point_hit_locations_are_inside_polygon` 等）が新時刻範囲で通るかを再確認

---

## 改善候補（バグではない）

### IMPROVE-001: 観測者標高の可変対応

現在、`tools.rs` の `geo::calc_altitude` 呼び出しは観測者標高を `0.0` 固定にしている。実地形では観測者が標高を持つ場合、地球曲率による富士山頂の地平線下没距離が伸びる（例: 筑波山 877m なら +112 km）。

**影響:** 200 km 以遠の山岳地（茨城北部・栃木・福島南部等）でダイヤモンド富士が見える物理的可能性があるが、現実装ではカバーできない。

**検討事項:**
- `point` / `range` の API シグネチャに `observer_altitude_m: f64` を追加（破壊的変更）
- もしくはオーバーロード関数を別名で提供
- `polygon` 用には DEM (Digital Elevation Model) の組み込みが必要だが、これは大幅な依存追加なのでオプション機能で別レイヤとする

### IMPROVE-002: `BISECTION_HIGH_DISTANCE` の 200 km 上限見直し

地平線距離の物理限界は観測者標高 0 m なら ~220 km。標高考慮（IMPROVE-001）と組み合わせて、上限を観測者標高に応じて動的に決める形にするか、単純に 250 km 程度まで拡張する。

### IMPROVE-003: `bin_times` と `bins` の二段階 BTreeMap 一本化

集約処理で `BTreeMap<i32, BTreeSet<NaiveDateTime>>` と `BTreeMap<i32, BinEdge>` を別々に構築している。`BinEntry { times, near, far }` 等に統合すれば構造がよりシンプルに。機能的には問題なし。

### IMPROVE-004: 時間ループの `for i in 0..=loop_n` 形式統一

`detect_alignment_for_location` は `for i in 0..=loop_n` 形式、`create_lonlat_vec` は `while offset_seconds <= end_offset_seconds` 形式。BUG-001 の修正と合わせて統一。

### IMPROVE-005: 360°/0° 跨ぎ日付の回帰テスト

現状の回帰テスト（`multiple_point_hit_locations_are_inside_polygon` 等）はいずれも方位帯が 360°/0° を跨がない日付（11 月）のみ。夏至前後など方位帯が跨ぎを起こしうる日付の回帰テストを追加して、anchor / `relative_az` / リング順序の不変条件を強化したい。

### IMPROVE-006: CHANGELOG 形式の整理

現在 `Notes` セクションを使っているが、Keep a Changelog 1.1.0 仕様外。`Changed` への統合か、設計判断は別ファイル（`ARCHITECTURE.md` 等）に分離するか検討。

---

## 完了済み

CHANGELOG の `[Unreleased]` セクション、および各リリースの履歴を参照。

- 0.1.0-beta.2: polygon 精度改善（凸包 → envelope リング、二分法収束厳格化）
- Unreleased: polygon 頂点削減（envelope リング → 方位ビン集約、~1500 → ~180 頂点）
