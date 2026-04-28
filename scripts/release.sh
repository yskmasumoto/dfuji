#!/usr/bin/env bash
#
# release.sh - main の最新コミットに annotated tag を打って push する
#
# 使い方:
#   ./scripts/release.sh <version>
#
# 例:
#   ./scripts/release.sh 0.1.0-beta.1
#   ./scripts/release.sh 0.1.0
#

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

TAG="v${VERSION}"

# main 上 / 作業ツリー clean / origin と同期
[[ "$(git rev-parse --abbrev-ref HEAD)" == "main" ]] || { echo "error: must be on main" >&2; exit 1; }
[[ -z "$(git status --porcelain)" ]] || { echo "error: working tree not clean" >&2; exit 1; }

git fetch origin --tags --quiet
[[ "$(git rev-parse HEAD)" == "$(git rev-parse origin/main)" ]] \
  || { echo "error: local main not in sync with origin/main" >&2; exit 1; }

# タグ重複チェック
git rev-parse "$TAG" >/dev/null 2>&1 \
  && { echo "error: tag '$TAG' already exists locally" >&2; exit 1; }
git ls-remote --tags --exit-code origin "refs/tags/${TAG}" >/dev/null 2>&1 \
  && { echo "error: tag '$TAG' already exists on origin" >&2; exit 1; }

git tag -a "$TAG" -m "Release ${TAG}"
git push origin "$TAG"

echo "pushed ${TAG}"
