#!/usr/bin/env bash
#
# version_update.sh - ワークスペース内の全 Cargo.toml の version を一括更新する
#
# 使い方:
#   ./scripts/version_update.sh <version>
#
# 例:
#   ./scripts/version_update.sh 0.1.0-beta.1
#
# 動作:
#   1. app, cli, core, geo, sun の各 Cargo.toml の
#      [package] 直下の version を <version> に書き換える
#   2. Cargo.lock を更新する
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

CRATES=(app cli core geo sun)

# リポジトリルートに移動
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

echo "==> updating workspace versions to ${VERSION}"

# [package] セクション直下の最初の `version = "..."` のみ置換する。
# ([dependencies] 内の version 指定は触らない)
for c in "${CRATES[@]}"; do
  toml="${c}/Cargo.toml"
  if [[ ! -f "$toml" ]]; then
    echo "error: $toml not found" >&2
    exit 1
  fi
  perl -i -pe '
    BEGIN { $in_pkg = 0; $done = 0; }
    if (/^\[package\]\s*$/)        { $in_pkg = 1; }
    elsif (/^\[/)                  { $in_pkg = 0; }
    if ($in_pkg && !$done && /^version\s*=\s*"[^"]*"/) {
      s/^version\s*=\s*"[^"]*"/version = "'"$VERSION"'"/;
      $done = 1;
    }
  ' "$toml"
  echo "    $toml -> $(grep -m1 '^version' "$toml")"
done

echo "==> updating Cargo.lock"
cargo update --workspace --offline >/dev/null 2>&1 || cargo generate-lockfile >/dev/null

echo
echo "done. review changes with:"
echo "    git diff --stat"
echo "    git diff"
echo
echo "then commit & push, e.g.:"
echo "    git add Cargo.lock app/Cargo.toml cli/Cargo.toml core/Cargo.toml geo/Cargo.toml sun/Cargo.toml"
echo "    git commit -m 'chore: bump version to ${VERSION}'"
echo "    git push origin main"
echo
echo "after that, tag with:"
echo "    ./scripts/release.sh ${VERSION}"
