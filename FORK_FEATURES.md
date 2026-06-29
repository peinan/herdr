# フォーク独自機能まとめ (peinan/herdr)

このフォークが `develop` で `master`(アップストリームミラー)の上に積んでいる
自分用のカスタマイズ一覧。`master...develop` の差分を機能単位で整理したもの。

- ブランチ運用・同期手順は [`CLAUDE.local.md`](./CLAUDE.local.md) を参照。
- このファイルは `develop` 限定。`CLAUDE.local.md` と同様、`master` には入れない。

## 更新のしかた

新しい機能を `develop` に積んだら、以下で差分を確認して該当行を追記する。

```bash
git log --oneline --no-merges master..develop   # 個人コミット一覧
git diff --stat master...develop                 # 変更ファイル一覧
git diff master...develop -- src/config/model.rs # 追加した設定フィールド
```

最終更新: 2026-06-30

## 機能一覧

| 機能 | 切替 | 概要 | 追加日 | PR / コミット |
|------|------|------|--------|--------------|
| ペインタイトル書式 | `ui.pane_title_format` | starship 風 `$var`＋条件付きグループ `(...)` でペイン枠タイトルを自由記述。真のプロセス名・per-pane git(branch/ahead-behind/dirty) を herdr ネイティブ検出から描画 | 2026-06-30 | `cccf16b`,`5f082c6`,`6c7cf39` |
| 単独ペインの枠 | `ui.single_pane_border` | ペインが 1 つだけでも枠を描く | 2026-06-27 | #11 / `47a97f9` |
| 非アクティブペイン減光 | `ui.dim_inactive_panes` | 非フォーカスのペインを薄くしてアクティブを強調(既定で挙動変更) | 2026-06-26 | #2 / `321db15` |
| サイドバー区切り線 | `ui.sidebar_divider` | サイドバー/ペイン領域の縦区切り線の表示切替 | 2026-06-26 | #3 / `46a8de6` |
| ズームマーカー | `ui.zoom_indicator` | `prefix+z` ズーム時に枠タイトルへ付くマーカー | 2026-06-26 | #4 / `ffcdab9` |
| prefix 待機表示 | `ui.prefix_indicator` | prefix 待機の示し方(バー or 枠+タブ強調) | 2026-06-27 | #5 / `aa108e7` |
| repeatable prefix バインド | `keys.*` / `keys.repeat_timeout` | tmux `bind -r` 風に素キーで繰り返し | 2026-06-26 | #1 / `4874e19` |
| タブバー再設計 | 常時(コード) | 下端・右寄せのマーカーストリップに変更 | 2026-06-27 | #10 / `c19f18b` |
| `▌` マーカー削除 | 常時(コード) | ペイン枠タイトルから `▌` フォーカスマーカーを削除 | 2026-06-26 | `3eb5ef4` |
| 個人用 Makefile | 新規ファイル | build / install ターゲット | 2026-06-26 | `38f7787` |
| `CLAUDE.local.md` | 新規ファイル | フォークのブランチ運用・同期・PR 手順 | 2026-06-28 | `541eb1c`, `926ccc1` |

**「切替」列の読み方**: 設定キーが書かれた機能は `config.toml` で切替可能(設定ファイル経由)。
「常時(コード)」は設定なしで常に適用される直接改修。「新規ファイル」は追加したファイル。

## 追加した設定項目

| キー | 型 | 既定値 | 説明 |
|------|----|--------|------|
| `ui.pane_title_format` | string | `""` | ペイン枠タイトルの書式(starship 風)。空=従来挙動(オプトイン)。詳細は下の補足参照 |
| `ui.single_pane_border` | bool | `false` | ペインが 1 つだけのタブでも枠を描く。`ui.pane_borders = false` のときは無効 |
| `ui.dim_inactive_panes` | bool | `true` | 非フォーカスのペインを減光。prefix/コマンドモード外でも効く。**アップストリーム既定からの挙動変更** |
| `ui.sidebar_divider` | bool | `true` | サイドバーとメインのペイン領域のあいだの縦区切り線。`false` で非表示 |
| `ui.zoom_indicator` | string | `"Z"` | `prefix+z` ズーム時にペイン枠タイトルへ付くマーカー。既存ラベルの後ろに付く。`""` で非表示 |
| `ui.prefix_indicator` | enum | `"status_bar"` | prefix 待機の表示。`status_bar`=下部ヒントバー / `highlight`=バーを隠しフォーカス枠+アクティブタブを再着色 |
| `keys.repeat_timeout` | u64 (ms) | `500` | repeatable バインドがアーム状態を保つ時間。`0` は既定にクランプ |
| `keys.<action>`(テーブル形式) | `{ key, repeat }` | `repeat = false` | `{ key = "prefix+n", repeat = true }` で tmux `bind -r` 風の繰り返しを有効化。文字列/配列形式は repeat しない |

## 補足: ペインタイトル書式 (`pane_title_format`)

`.zshrc` のフック(`herdr pane rename`)を使わず、herdr がネイティブ検出した情報から
ペイン枠タイトルを描く。starship のサブセット書式。

```toml
[ui]
pane_title_format = "$dir $process( ⋅ $branch$ahead_behind$git_status)"
```

- **変数**(該当なしは空文字に解決):
  - `$dir` cwd の basename(`$HOME` は `~`) / `$cwd` フルパス(先頭 `$HOME` を `~` 省略)
  - `$process` 真のフォアグラウンドプロセス名(alias/symlink/ランタイムラッパ解決済み。`cl`→`claude`、素の `nvim`/`zsh` も)
  - `$agent` 検出エージェントラベル
  - `$branch` git ブランチ(detached HEAD は短縮 SHA フォールバック)
  - `$ahead_behind` 上流との差(`⇡2⇣1`、0 は省略)
  - `$git_status` ワーキングツリー状態(`=`衝突 `!`変更 `+`ステージ `?`未追跡)
  - `$zoom` ズームマーカー(`ui.zoom_indicator`) / `$label` 手動ラベル(`pane rename`)
- **条件付きグループ** `( … )`: 中の変数が**すべて空**ならグループ全体(区切り文字・記号含む)を非表示。ネスト可。
  上の例はリポジトリ外では ` ⋅ ` ごと消える。
- **エスケープ**: `\$` `\(` `\)` `\\`。名前境界の明示は `${name}` 形式。
- **優先順位**: OSC タイトル > 手動ラベル(`pane rename`) > この書式。書式が空なら従来挙動のまま(完全に不変)。
- プロセス名・git はすべて herdr 側で検出(シェルフック不要)。git は約1.5秒ポーリングで
  エージェント内の `git checkout` 等にも追従。`$git_status`(dirty)は毎ポーリング再計算。
  per-pane git は repo 単位キーなので、同一リポジトリの別 worktree のペインは各自のブランチを表示。

## 補足: repeatable prefix バインド

```toml
[keys]
swap_pane_up   = { key = "prefix+shift+k", repeat = true }
swap_pane_down = { key = "prefix+shift+j", repeat = true }
next_tab       = { key = "prefix+n", repeat = true }
previous_tab   = { key = "prefix+p", repeat = true }
repeat_timeout = 500
```

- **アーム解除**: prefix キー / `Esc` / 未バインドのキー / `repeat_timeout` 経過
- **対象**: ナビ・サイクル系(端末/ナビュー復帰後の動作)— 次/前のワークスペース・
  エージェント・タブ、ペインの swap / cycle / focus、zoom、サイドバー切替
- **対象外**: ダイアログ/サブモードに入る動作(rename, help, settings, ワークスペース
  ピッカー, ナビゲータ, copy / resize モード)。`repeat = true` を付けても通常遷移して
  アーム終了する

## 変更履歴

新しい順。詳細は各表を参照。

### 2026-06-30
- `pane_title_format` 追加(starship 風のペインタイトル書式)。真のフォアグラウンドプロセス名・
  per-pane git(branch/ahead-behind/ワーキングツリー状態)を herdr ネイティブ検出から描画。
  シェルフック不要・`git checkout` 追従 — `cccf16b`, `5f082c6`, `6c7cf39`

### 2026-06-28
- `CLAUDE.local.md`(フォーク運用ルール)を追加・日本語化 — `541eb1c`, `926ccc1`

### 2026-06-27
- `single_pane_border` 追加 — PR #11 (`47a97f9`)
- タブバーを下端・右寄せのマーカーストリップに再設計 — PR #10 (`c19f18b`)
- `prefix_indicator` 追加 — PR #5 (`aa108e7`)

### 2026-06-26
- `zoom_indicator` 追加(ズームマーカーをタブ→ペイン枠タイトルへ移動)— PR #4 (`ffcdab9`)
- `sidebar_divider` 追加 — PR #3 (`46a8de6`)
- `dim_inactive_panes` 追加(既定 `true`)— PR #2 (`321db15`)
- repeatable prefix バインディング追加(tmux `bind -r` 風)— PR #1 (`4874e19`)
- 個人用 `Makefile` 追加 — `38f7787`
- ペイン枠タイトルから `▌` フォーカスマーカー削除 — `3eb5ef4`
