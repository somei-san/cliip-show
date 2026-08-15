---
name: vrt-update
description: VRT を実行し、閾値を超えたケースを提示してから baseline を更新する
disable-model-invocation: true
---

# VRT baseline 更新

`./scripts/visual_regression.sh --update` は全ケースの baseline を一括で上書きする。意図しない HUD 変更まで baseline に焼き付けないよう、更新前に差分の中身を人が見る。

## 手順

1. 判定だけを実行する。

   ```bash
   ./scripts/visual_regression.sh
   ```

2. 出力から `ng:` のケースを拾い、ケース ID・`pixels` 比・diff PNG のパスを一覧にしてユーザーに示す。全ケースが `ok:` なら baseline は既に一致しているので、ここで終了する。

3. diff PNG（`tests/visual/artifacts/<id>.diff.png`）と現行 PNG（`tests/visual/artifacts/<id>.current.png`）を Read で開き、差分が意図した UI 変更かを述べる。

4. 「baseline を更新しますか？ (Y/n)」で確認する。承認されるまで更新しない。

5. 承認されたら更新し、再実行して `visual regression passed` になることを確かめる。

   ```bash
   ./scripts/visual_regression.sh --update
   ./scripts/visual_regression.sh
   ```

## 注意

- 更新は全ケース一括。特定ケースだけを更新する経路はスクリプトに無いので、`ng` 以外のケースにも差分が無いことを手順 2 で確認してから進める。
- 許容差は環境変数 `MAX_DIFF_PERMILLE`（既定 120/1000）。許容内の揺れは `ok` になり baseline 更新の対象にならない。
