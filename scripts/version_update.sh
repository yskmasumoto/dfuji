#!/usr/bin/env bash
#
# version_update.sh - ワークスペースのバージョンを一括更新する
#
# 使い方:
#   ./scripts/version_update.sh <version>
#
# 例:
#   ./scripts/version_update.sh 0.1.0-beta.1
#
# 動作:
#   ルート Cargo.toml の [workspace.package] 直下の version を <version> に書き換え、
#   Cargo.lock を更新する。各メンバークレートは `version.workspace = true` で
#   この値を継承する構成のため、メンバー側の Cargo.toml は触らない。
#
# このスクリプトは git コミット / push は行わない。
# 変更内容を `git diff` で確認し、ユーザー自身でコミット & プッシュしてから
# `./scripts/release.sh <version>` でタグを打つこと。

set -euo pipefail

VERSION="${1:-}"
if [[ -z "$VERSION" ]]; then
  echo "usage: $0 <version>  e.g. 0.1.0-beta.1" >&2
  exit 1
fi

# SemVer 簡易検証 (MAJOR.MINOR.PATCH[-prerelease])
if [[ ! "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$ ]]; then
  echo "error: '$VERSION' is not a valid SemVer string" >&2
  exit 1
fi

# リポジトリルートに移動
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

ROOT_TOML="Cargo.toml"
if [[ ! -f "$ROOT_TOML" ]]; then
  echo "error: $ROOT_TOML not found" >&2
  exit 1
fi

echo "==> updating [workspace.package] version to ${VERSION}"

# [workspace.package] セクション直下の最初の `version = "..."` のみ置換する。
# 行末コメントが付いた `[workspace.package] # comment` 形式にも対応するため
# セクションヘッダ判定は前方一致で行う。
perl -i -pe '
  BEGIN { $in_wpk = 0; $done = 0; }
  if (/^\[workspace\.package\](\s|$|#)/) { $in_wpk = 1; }
  elsif (/^\[/)                          { $in_wpk = 0; }
  if ($in_wpk && !$done && /^version\s*=\s*"[^"]*"/) {
    s/^version\s*=\s*"[^"]*"/version = "'"$VERSION"'"/;
    $done = 1;
  }
' "$ROOT_TOML"

# 確認: 置換できたか
new_line="$(perl -ne '
  BEGIN { $in_wpk = 0; }
  if (/^\[workspace\.package\](\s|$|#)/) { $in_wpk = 1; }
  elsif (/^\[/)                          { $in_wpk = 0; }
  if ($in_wpk && /^version\s*=\s*"[^"]*"/) { print; exit; }
' "$ROOT_TOML")"
if [[ -z "$new_line" ]]; then
  echo "error: failed to update [workspace.package] version (no version line found)" >&2
  exit 1
fi
echo "    $ROOT_TOML -> $new_line"

echo "==> updating Cargo.lock"
cargo update --workspace --offline >/dev/null 2>&1 || cargo generate-lockfile >/dev/null

echo
echo "done. review changes with:"
echo "    git diff --stat"
echo "    git diff"
echo
echo "then commit & push, e.g.:"
echo "    git add Cargo.toml Cargo.lock"
echo "    git commit -m 'chore: bump version to ${VERSION}'"
echo "    git push origin main"
echo
echo "after that, tag with:"
echo "    ./scripts/release.sh ${VERSION}"
