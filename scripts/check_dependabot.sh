#!/usr/bin/env bash
set -euo pipefail

# dependabot は半年に 1 度しか動かない設定（.github/dependabot.yml）。
# 溜まった更新を見落としたままリリースしないよう、未処理の PR が残っていれば止める。

OPEN_BOT_PRS="$(gh pr list --repo somei-san/cliip-show --author "app/dependabot" --state open --json number --jq 'length')"
if [[ "$OPEN_BOT_PRS" != "0" ]]; then
  echo "ERROR: dependabot の PR が ${OPEN_BOT_PRS} 件残っています。片付けてからリリースしてください" >&2
  gh pr list --repo somei-san/cliip-show --author "app/dependabot" --state open >&2
  exit 1
fi
