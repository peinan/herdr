# フォーク独自機能まとめ (peinan/herdr)

このフォークが `main` で upstream の上に積んでいる自分用のカスタマイズ一覧。
`upstream/master...main` の差分を機能単位で整理したもの。

- ブランチ運用・同期・開発フローは [`FORK_WORKFLOW.md`](./FORK_WORKFLOW.md) と [`CLAUDE.local.md`](../../CLAUDE.local.md) を参照。
- フォーク専用ドキュメント（`CLAUDE.local.md` と同様、アップストリームには出さない）。

## 更新のしかた

新しい機能を `main` に積んだら、以下で差分を確認して該当行を追記する。

```bash
git log --oneline --no-merges upstream/master..main   # 個人コミット一覧
git diff --stat upstream/master...main                 # 変更ファイル一覧
git diff upstream/master...main -- src/config/model.rs # 追加した設定フィールド
```

最終更新: 2026-09-10

## 機能一覧

| 機能 | 切替 | 概要 | 追加日 | PR / コミット |
|------|------|------|--------|--------------|
| ペイン内余白 | `ui.pane_padding` | ペイン枠(または端)と端末内容の間に上下左右セル単位の余白。省略辺は 0、既定は余白なし | 2026-07-07 | [#15](https://github.com/peinan/herdr/pull/15) / [`b23578e`](https://github.com/peinan/herdr/commit/b23578e) |
| ペインタイトル書式 | `ui.pane_title_format` | starship 風 `$var`＋条件付きグループ `(...)` でペイン枠タイトルを自由記述。真のプロセス名・per-pane git(branch/ahead-behind/dirty) を herdr ネイティブ検出から描画 | 2026-06-30 | [`cccf16b`](https://github.com/peinan/herdr/commit/cccf16b), [`5f082c6`](https://github.com/peinan/herdr/commit/5f082c6), [`6c7cf39`](https://github.com/peinan/herdr/commit/6c7cf39) |
| 非アクティブペイン減光 | `ui.dim_inactive_panes` | 非フォーカスのペインを薄くしてアクティブを強調。**既定 off**(減光なし、アップストリーム一致)、`true` で常時減光。0.9.0 sync でサーバ側ペイン面描画＋retained patch へ再実装 | 2026-06-26 | [#2](https://github.com/peinan/herdr/pull/2) / [`321db15`](https://github.com/peinan/herdr/commit/321db15), [#22](https://github.com/peinan/herdr/pull/22) / [`84a52ef`](https://github.com/peinan/herdr/commit/84a52ef) |
| フォーカスマーカー | `ui.show_pane_focus_marker` | focused ペイン枠タイトル頭の `▌` マーカー。**既定 on**(アップストリーム)、`false` で色/太字のみ | 2026-06-26 | [`3eb5ef4`](https://github.com/peinan/herdr/commit/3eb5ef4), [#23](https://github.com/peinan/herdr/pull/23) / [`fbcc969`](https://github.com/peinan/herdr/commit/fbcc969) |
| サイドバー区切り線 | `ui.sidebar_divider` | サイドバー/ペイン領域の縦区切り線の表示切替(0.9.0 sync でクライアントシェル `ClientShellConfig` へ移植) | 2026-06-26 | [#3](https://github.com/peinan/herdr/pull/3) / [`46a8de6`](https://github.com/peinan/herdr/commit/46a8de6) |
| ズームマーカー | `ui.zoom_indicator` / `ui.zoom_indicator_position` | `prefix+z` ズーム時のマーカー文字と表示位置(`tab`/`pane`/`both`/`none`、**既定 `tab`**)。**タブ側は 0.9.0 sync 以降 未実装**(`tab` は何も表示せず、`both` はペインのみ) | 2026-06-26 | [#4](https://github.com/peinan/herdr/pull/4) / [`ffcdab9`](https://github.com/peinan/herdr/commit/ffcdab9), [#25](https://github.com/peinan/herdr/pull/25) / [`da3dbd8`](https://github.com/peinan/herdr/commit/da3dbd8) |
| タブバースタイル | `ui.tab_bar_{style,align,title}` | `classic`/`minimal`(グリフ帯)を選択、**既定 `classic`**。**`minimal` は 0.9.0 sync 以降 未実装**(設定は受理・inert、[#34](https://github.com/peinan/herdr/issues/34))。`tab_bar_position` は upstream 同名キーに統一(既定 `top`) | 2026-06-27 | [#10](https://github.com/peinan/herdr/pull/10) / [`c19f18b`](https://github.com/peinan/herdr/commit/c19f18b), [#24](https://github.com/peinan/herdr/pull/24) / [`68fe136`](https://github.com/peinan/herdr/commit/68fe136) |
| prefix 待機表示 | `ui.prefix_indicator` | prefix 待機の示し方(バー or 枠+タブ強調)。**`highlight` は 0.9.0 sync 以降 未実装**(inert、[#35](https://github.com/peinan/herdr/issues/35)) | 2026-06-27 | [#5](https://github.com/peinan/herdr/pull/5) / [`aa108e7`](https://github.com/peinan/herdr/commit/aa108e7) |
| repeatable prefix バインド | `keys.*` / `keys.repeat_timeout` | tmux `bind -r` 風に素キーで繰り返し。**0.9.0 sync 以降 未実装**(設定は受理・inert、[#36](https://github.com/peinan/herdr/issues/36)) | 2026-06-26 | [#1](https://github.com/peinan/herdr/pull/1) / [`4874e19`](https://github.com/peinan/herdr/commit/4874e19) |
| 個人用 Makefile | 新規ファイル | build / install ターゲット | 2026-06-26 | [`38f7787`](https://github.com/peinan/herdr/commit/38f7787) |
| `CLAUDE.local.md` | 新規ファイル | フォークのブランチ運用・同期・PR 手順 | 2026-06-28 | [`541eb1c`](https://github.com/peinan/herdr/commit/541eb1c), [`926ccc1`](https://github.com/peinan/herdr/commit/926ccc1) |

**「切替」列の読み方**: 設定キーが書かれた機能は `config.toml` で切替可能(設定ファイル経由)。
「新規ファイル」は追加したファイル。UI 系の独自機能は既定でアップストリーム相当の挙動になり、
カスタムは設定でオプトインする(このフォークの方針、統合 [#16](https://github.com/peinan/herdr/issues/16))。

**0.9.0 sync(2026-09-10)での整理**: upstream [#3487](https://github.com/ogulcancelik/herdr/pull/3487) で shell 描画(タブバー・サイドバー・モードバー・prefix・キー配送)が
クライアントへ移ったため、`ui.single_pane_border` は廃止(upstream の `pane_borders = "always"`＋`pane_outer_borders = true` が同等)、
フォーク独自の `tab_bar_position` は upstream 同名キーに統一(既定 `top`、classic にも適用)、minimal タブバー /
`prefix_indicator = "highlight"` / repeatable prefix は再実装待ち([#34](https://github.com/peinan/herdr/issues/34) / [#35](https://github.com/peinan/herdr/issues/35) / [#36](https://github.com/peinan/herdr/issues/36)。設定キーは parse のみで inert)。

## 追加した設定項目

| キー | 型 | 既定値 | 説明 |
|------|----|--------|------|
| `ui.pane_padding` | table | `{}`(全 0) | ペイン枠(端)と端末内容の間の余白(セル)。`{ top, right, bottom, left }` で省略辺 0。狭いペインでは内容 1x1 を残しクランプ |
| `ui.pane_title_format` | string | `""` | ペイン枠タイトルの書式(starship 風)。空=従来挙動(オプトイン)。詳細は下の補足参照 |
| `ui.dim_inactive_panes` | bool | `false` | 非フォーカスのペインを減光。既定 `false`=アップストリーム挙動(減光なし)、`true`=常時減光 |
| `ui.show_pane_focus_marker` | bool | `true` | focused ペイン枠タイトル頭の `▌` マーカー。既定 `true`=アップストリーム、`false` で非表示(フォーカスは色/太字のみ) |
| `ui.sidebar_divider` | bool | `true` | サイドバーとメインのペイン領域のあいだの縦区切り線。`false` で非表示 |
| `ui.zoom_indicator` | string | `"Z"` | `prefix+z` ズーム時のマーカー文字。表示位置は `zoom_indicator_position`。`""` で非表示 |
| `ui.zoom_indicator_position` | enum | `"tab"` | ズームマーカーの表示位置。`tab`(既定)/`pane`/`both`/`none`。`pane_title_format` の `$zoom` もこれに従う。文字が空 or `none` で非表示。タブ側は再実装待ち(`tab` は非表示、`both` はペインのみ) |
| `ui.tab_bar_style` | enum | `"classic"` | タブバーのスタイル。`classic`=テキストタブ(スクロール可、アップストリーム)/`minimal`=グリフ帯。**`minimal` は未実装(inert、[#34](https://github.com/peinan/herdr/issues/34))** |
| `ui.tab_bar_align` | enum | `"right"` | `minimal` のときの寄せ。`left`/`right`。`classic` では無効。inert([#34](https://github.com/peinan/herdr/issues/34)) |
| `ui.tab_bar_title` | bool | `false` | `minimal` でタブ名も併記するか。`classic` では常にテキスト。inert([#34](https://github.com/peinan/herdr/issues/34)) |
| `ui.prefix_indicator` | enum | `"status_bar"` | prefix 待機の表示。`status_bar`=下部ヒントバー / `highlight`=バーを隠しフォーカス枠+アクティブタブを再着色。**`highlight` は未実装(inert、[#35](https://github.com/peinan/herdr/issues/35))** |
| `keys.repeat_timeout` | u64 (ms) | `500` | repeatable バインドがアーム状態を保つ時間。**未実装(inert、[#36](https://github.com/peinan/herdr/issues/36))** |
| `keys.<action>`(テーブル形式) | `{ key, repeat }` | `repeat = false` | `{ key = "prefix+n", repeat = true }` で tmux `bind -r` 風の繰り返しを有効化。文字列/配列形式は repeat しない。**未実装(inert、[#36](https://github.com/peinan/herdr/issues/36))** |

## 補足: ペインタイトル書式 (`pane_title_format`)

`.zshrc` のフック(`herdr pane rename`)を使わず、herdr がネイティブ検出した情報から
ペイン枠タイトルを描く。starship のサブセット書式。

```toml
[ui]
pane_title_format = "$dir $process( ⋅ $branch$ahead_behind$git_status)( $zoom)"
```

- **変数**(該当なしは空文字に解決):
  - `$dir` cwd の basename(`$HOME` は `~`) / `$cwd` フルパス(先頭 `$HOME` を `~` 省略)
  - `$process` 真のフォアグラウンドプロセス名(alias/symlink/ランタイムラッパ解決済み。`cl`→`claude`、素の `nvim`/`zsh` も)
  - `$agent` 検出エージェントラベル
  - `$branch` git ブランチ(detached HEAD は短縮 SHA フォールバック)
  - `$ahead_behind` 上流との差(`⇡2⇣1`、0 は省略)
  - `$git_status` ワーキングツリー状態(`=`衝突 `!`変更 `+`ステージ `?`未追跡)
  - `$zoom` ズームマーカー(`ui.zoom_indicator`。zoom 中かつ `ui.zoom_indicator_position` が `pane`/`both` のときのみ非空) / `$label` 手動ラベル(`pane rename`)
- **条件付きグループ** `( … )`: 中の変数が**すべて空**ならグループ全体(区切り文字・記号含む)を非表示。ネスト可。
  上の例はリポジトリ外では ` ⋅ ` ごと消える。
- **エスケープ**: `\$` `\(` `\)` `\\`。名前境界の明示は `${name}` 形式。
- **優先順位**: OSC タイトル > 手動ラベル(`pane rename`) > この書式。書式が空なら従来挙動のまま(完全に不変)。
- **zoom マーカー**: 書式を指定したときは `$zoom` が配置を制御(末尾への自動付加は抑止)。ただし
  `zoom_indicator_position` が `pane`/`both` を含まないと `$zoom` は空に解決する(既定 `tab` では
  ペイン側には出ずタブへ表示)。書式が空(デフォルト)のときだけ従来どおり末尾へ自動付加し、これも
  `zoom_indicator_position` に従う。
- cwd・プロセス名・git はすべて herdr 側で検出(シェルフック/OSC 7 不要)。`$dir`/`$cwd` は
  git と同じ syscall ベースの cwd 解決に約1.5秒 tick で同期するので、`cd`(`rp` 等)や
  エージェント内のディレクトリ移動に追従する。git も約1.5秒ポーリングで `git checkout` 等に
  追従し、`$git_status`(dirty)は毎ポーリング再計算・作業ツリー変化でも即再描画。
  per-pane git は repo 単位キーなので、同一リポジトリの別 worktree のペインは各自のブランチを表示。

## 補足: タブバースタイル (`tab_bar_style`)

タブバーは 2 系統から選べる。既定は `classic`(アップストリーム相当)。**0.9.0 sync 以降、`minimal` は未実装**
(upstream [#3487](https://github.com/ogulcancelik/herdr/pull/3487) でタブバーがクライアントシェルへ移ったため描画コードを削除。設定は受理されるが `classic` で描かれる。
再実装は [#34](https://github.com/peinan/herdr/issues/34))。以下は旧実装の仕様。

```toml
[ui]
tab_bar_style    = "minimal"   # classic | minimal
tab_bar_position = "bottom"    # upstream 共通キー: top | bottom(classic にも適用、既定 top)
tab_bar_align    = "right"     # minimal のみ: left | right
tab_bar_title    = false       # minimal のみ: グリフの横にタブ名を併記
```

- **`classic`**(既定): アップストリームの上部テキストタブバーを忠実に復元。自動採番・カスタム名・
  タブ多数時の横スクロール(スクロールボタン/ヒット領域)を持つ。`position`/`align`/`title` は無効
  (常に上・テキスト)。
- **`minimal`**: このフォークが導入したグリフ帯。`position`(上/下)・`align`(左/右)・
  `title`(タブ名併記の有無)で調整する。
- **旧フォークの見た目(下端・右寄せ・グリフ帯)に戻すには** `tab_bar_style = "minimal"` と `tab_bar_position = "bottom"` を書く
  (`align=right` / `title=false` が既定)。**現在は未実装のため反映されない**([#34](https://github.com/peinan/herdr/issues/34))。
- ズームマーカーのタブ側表示(`zoom_indicator_position` が `tab`/`both`)は 0.9.0 sync 以降 未実装。upstream の
  `tab_bar_right = [{ type = "zoom" }]` で右端に `ZOOM` を出せる。

## 補足: repeatable prefix バインド

**0.9.0 sync 以降 未実装**(upstream [#3487](https://github.com/ogulcancelik/herdr/pull/3487) でキー配送がクライアントシェルへ移ったため動作コードを削除。
`repeat = true` / `repeat_timeout` は受理されるが無効。再実装は [#36](https://github.com/peinan/herdr/issues/36))。以下は旧実装の仕様。

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

### 2026-09-10
- upstream v0.9.0(+14 commits、`120c6820`)まで段階マージ(v0.8.0 → v0.8.2 → master)で取り込み — [`69321a3`](https://github.com/peinan/herdr/commit/69321a3), [`d7a381f`](https://github.com/peinan/herdr/commit/d7a381f), [`f48d90e`](https://github.com/peinan/herdr/commit/f48d90e)
  - upstream [#3487](https://github.com/ogulcancelik/herdr/pull/3487) で shell(タブバー・サイドバー・モードバー・prefix・キー配送)がクライアント側描画へ移行。
  - **廃止**: `single_pane_border`(upstream `pane_borders = "always"` で代替)、フォーク独自 `tab_bar_position`(upstream 同名キーに統一、既定 `top`)、タブバー上下配置・モードバー相乗りの独自実装。
  - **維持**: `pane_title_format`(git 情報は upstream の demand 駆動 `git_refresh.rs` に `working_tree` demand を足して供給)、`pane_padding`、`show_pane_focus_marker`、ペイン側ズームマーカー、`dim_inactive_panes`(サーバ側ペイン面描画＋retained patch で再実装)、`sidebar_divider`(クライアントシェルへ移植)。
  - **再実装待ち(設定は parse のみ)**: minimal タブバー [#34](https://github.com/peinan/herdr/issues/34)、`prefix_indicator = "highlight"` [#35](https://github.com/peinan/herdr/issues/35)、repeatable prefix [#36](https://github.com/peinan/herdr/issues/36)。タブ側ズームマーカーも当面非表示。

### 2026-07-08
- フォーク独自 UI を「既定 off/opt-in・素の起動時はアップストリーム相当の見た目」に整理(統合 [#16](https://github.com/peinan/herdr/issues/16))。
- `dim_inactive_panes` を既定 `false` に(opt-in 化)。既定でアップストリーム挙動(prefix/コマンド時のみ減光)に一致 — [#22](https://github.com/peinan/herdr/pull/22) ([`84a52ef`](https://github.com/peinan/herdr/commit/84a52ef))
- `show_pane_focus_marker` 追加(既定 `true`)。focused ペイン枠頭の `▌` マーカーを復活し、既定でアップストリーム挙動。`false` で色/太字のみ — [#23](https://github.com/peinan/herdr/pull/23) ([`fbcc969`](https://github.com/peinan/herdr/commit/fbcc969))
- タブバーをスタイル選択制に(`tab_bar_style` `classic`/`minimal`、既定 `classic`)。`classic` はアップストリーム上部テキストタブを復元、`minimal` は従来グリフ帯を `position`/`align`/`title` でパラメータ化 — [#24](https://github.com/peinan/herdr/pull/24) ([`68fe136`](https://github.com/peinan/herdr/commit/68fe136))
- `zoom_indicator_position` 追加(`tab`/`pane`/`both`/`none`、既定 `tab`)。ズームマーカーの表示位置を制御し、既定でタブに表示(アップストリーム的)。`pane`/`both` でペイン枠に。`pane_title_format` の `$zoom` も position に従う — [#25](https://github.com/peinan/herdr/pull/25) ([`da3dbd8`](https://github.com/peinan/herdr/commit/da3dbd8))

### 2026-07-07
- `ui.pane_padding` 追加。ペイン枠(または端)と端末内容の間に上下左右セル単位の余白を挿入(省略辺 0・既定は余白なし)。inner_rect を border の内側・scrollbar gutter の手前で縮めるため PTY サイズと描画が一致し、scrollbar は余白の内側に寄る。狭いペインは内容 1x1 を残しクランプ — [#15](https://github.com/peinan/herdr/pull/15) ([`b23578e`](https://github.com/peinan/herdr/commit/b23578e))
- `dim_inactive_panes` の減光を retained レンダリングの fast path でも再適用。フルレンダリングは
  非フォーカスペインを 2 パス目で減光するが、dirty patch の fast path は素の ghostty セルを減光せず
  描いていたため、忙しい非フォーカスペインが tick ごとに明滅(flicker)していた。両パスが一致するよう
  fast path でも `Modifier::DIM` を再適用 — [#14](https://github.com/peinan/herdr/pull/14) ([`76fa31b`](https://github.com/peinan/herdr/commit/76fa31b))

### 2026-07-05
- `pane_title_format` の cwd/git status 追従を修正。`$dir`/`$cwd` と per-pane git ルックアップが
  OSC 7 非依存で実 cwd を追う(git と同じ syscall 解決を約1.5秒 tick で同期)。作業ツリーのみの
  変化でも `$git_status` マークが約1.5秒で再描画されるように — [#13](https://github.com/peinan/herdr/pull/13) ([`5b64b53`](https://github.com/peinan/herdr/commit/5b64b53))

### 2026-06-30
- `pane_title_format` 追加(starship 風のペインタイトル書式)。真のフォアグラウンドプロセス名・
  per-pane git(branch/ahead-behind/ワーキングツリー状態)を herdr ネイティブ検出から描画。
  シェルフック不要・`git checkout` 追従 — [`cccf16b`](https://github.com/peinan/herdr/commit/cccf16b), [`5f082c6`](https://github.com/peinan/herdr/commit/5f082c6), [`6c7cf39`](https://github.com/peinan/herdr/commit/6c7cf39)

### 2026-06-28
- `CLAUDE.local.md`(フォーク運用ルール)を追加・日本語化 — [`541eb1c`](https://github.com/peinan/herdr/commit/541eb1c), [`926ccc1`](https://github.com/peinan/herdr/commit/926ccc1)

### 2026-06-27
- `single_pane_border` 追加 — [#11](https://github.com/peinan/herdr/pull/11) ([`47a97f9`](https://github.com/peinan/herdr/commit/47a97f9))
- タブバーを下端・右寄せのマーカーストリップに再設計 — [#10](https://github.com/peinan/herdr/pull/10) ([`c19f18b`](https://github.com/peinan/herdr/commit/c19f18b))
- `prefix_indicator` 追加 — [#5](https://github.com/peinan/herdr/pull/5) ([`aa108e7`](https://github.com/peinan/herdr/commit/aa108e7))

### 2026-06-26
- `zoom_indicator` 追加(ズームマーカーをタブ→ペイン枠タイトルへ移動)— [#4](https://github.com/peinan/herdr/pull/4) ([`ffcdab9`](https://github.com/peinan/herdr/commit/ffcdab9))
- `sidebar_divider` 追加 — [#3](https://github.com/peinan/herdr/pull/3) ([`46a8de6`](https://github.com/peinan/herdr/commit/46a8de6))
- `dim_inactive_panes` 追加(既定 `true`)— [#2](https://github.com/peinan/herdr/pull/2) ([`321db15`](https://github.com/peinan/herdr/commit/321db15))
- repeatable prefix バインディング追加(tmux `bind -r` 風)— [#1](https://github.com/peinan/herdr/pull/1) ([`4874e19`](https://github.com/peinan/herdr/commit/4874e19))
- 個人用 `Makefile` 追加 — [`38f7787`](https://github.com/peinan/herdr/commit/38f7787)
- ペイン枠タイトルから `▌` フォーカスマーカー削除 — [`3eb5ef4`](https://github.com/peinan/herdr/commit/3eb5ef4)
