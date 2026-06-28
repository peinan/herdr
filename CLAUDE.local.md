# 個人フォークの開発フロー (peinan/herdr)

`AGENTS.md`（`CLAUDE.md` がシンボリックリンクしている、アップストリーム管理の
規約）の上に重ねるフォーク固有の指示。コードスタイル・テスト・画面検出・
プロトコル・ドキュメントといったエンジニアリング実務は引き続き `AGENTS.md` が
正となる。このファイルが扱うのは *このフォーク* のブランチ運用・同期・取り込み
だけで、**同じフォーク固有のワークフローについて両者が食い違う場合はこのファイルを
優先する**（特にブランチの向き先と「`master` へ反映する」手順）。

このファイルは `develop` にのみコミットする。`master` には存在しないため、
アップストリームと衝突しない。`.gitignore` には追加しないこと。

## フォークの構成

アップストリーム `ogulcancelik/herdr` の個人フォーク。基本はアップストリームの
エンジニアリング規約に従い、このファイルが明記した箇所だけ逸脱する。ブランチは
2 本、役割も 2 つ:

- **`master`** — アップストリーム `ogulcancelik/herdr` をミラーする。個人の
  コミットは一切ここに入れない。常にアップストリームへのクリーンな fast-forward
  を保つ。
- **`develop`** — 個人の統合ライン。カスタマイズと機能開発はすべて `master` の
  上に積む。これが既定の作業ブランチ。

`origin` は `peinan/herdr`。`upstream` リモートは無く、必要も無い。

## フォーク独自機能のカタログ

`develop` に積んだ自分用のカスタマイズ（設定オプション・キーバインド・UI 改修）は
[`FORK_FEATURES.md`](./FORK_FEATURES.md) に一覧化してある。`master...develop` の差分を
機能単位で整理したもので、機能一覧・追加した設定項目・変更履歴の表を持つ。
`CLAUDE.local.md` と同じく `develop` 限定で、`master` には入れない。

新しい機能を `develop` にランディングさせたら `FORK_FEATURES.md` を更新する —
機能一覧の行、（設定を追加したなら）設定項目、変更履歴に、概要・PR 番号・機能コミットの
短縮 SHA を追記する。冒頭の再生成コマンド（`git diff master...develop`）で差分を拾える。

## アップストリームを `master`、そして `develop` へ同期する

`master` はアップストリームのミラーとしてのみ進み、`develop` はその変更を定期的に
取り込む。これは `master → develop` の一方向の "merge up" であり、逆向きには
しない。

1. `peinan/herdr` の GitHub ページで **"Sync fork"** を使い、フォークの `master`
   を `ogulcancelik/herdr` から fast-forward する。
2. それをローカルに取り込み、新しいアップストリームのベースを `develop` に運ぶ:

```bash
git checkout master && git pull          # ローカル master を fast-forward
git checkout develop && git merge master # アップストリームを develop に取り込む
just check                               # 検証 — ZIG が必要、「引き継ぐ規約」を参照
git push origin develop                  # develop は保護なし、直接 push でよい
```

`develop` はブランチ保護されていないため、このローカル merge + push が既定かつ
最も簡単な経路。`master → develop` の PR は任意 — CI やレビュー可能な差分が
欲しいときだけ使う。コンフリクト解消はどちらにせよローカルで行う。

### 注意点（定期的な `master → develop`）

- **向きは一方向のみ。** 常に `master → develop`（`head=master`, `base=develop`）。
  `develop` を `master` にマージしないこと — 個人のコミットが `master` に到達
  すると "Sync fork" が fast-forward できなくなる。
- **マージコミットにする。squash は禁止。** PR 経由にする場合は **マージコミット**
  でマージする。リポジトリでは 3 種のマージ方式すべてが有効なので squash は
  ワンクリックで選べてしまう — そして `master → develop` を squash すると、
  アップストリームのコミット群が `master` の履歴とリンクを持たない新しい SHA に
  潰される。以降のマージのたびに git が同じアップストリーム変更を再適用し、永遠に
  コンフリクトし続ける。ローカルの `git merge master` なら常に正しく処理される。
- **`develop` を `master` に rebase しないこと。** `develop` は公開済みで、
  worktree や機能ブランチが上に積まれている。rebase は個人コミットを書き換え、
  それらを壊す。merge up で取り込む。
- **コンフリクトはカスタマイズ箇所に集中する。** アップストリームと自分の UI 改修
  （タブバー、ペイン枠、prefix/zoom インジケータ）は同じ描画・設定ファイルに触れる。
  **両方** を取り込む形で解消すること — `-X ours`/`-X theirs` の一括指定は個人機能を
  黙って落としたり、アップストリームの修正を巻き戻したりする。
- **`just check` は UI 機能の欠落を検出できない。** 正しさの確認には実行するが、
  マージ後に自分のカスタマイズがちゃんと描画されるかも目視する。
- **アップストリーム管理のファイル（`AGENTS.md`, `README` など）に個人の変更を
  入れない。** 取り込みのたびのコンフリクトを抑えるため — 個人設定を `AGENTS.md`
  ではなくこのファイルに置いているのはそのため。
- **開いている `feat/*` worktree が少ないときに取り込む。** `develop` が
  アップストリームを取り込んだ後、進行中の機能ブランチは古いベースの上に残り、
  ランディング前にそれぞれ `develop` のマージが必要になることがある。

## 機能開発: 機能ごとに worktree を 1 つ

新しい機能やカスタマイズは、メインのチェックアウトではなく `develop` から切った
専用 worktree で行う。

- **場所:** `../herdr-worktrees/<slug>`（メインチェックアウトの兄弟ディレクトリ）。
  ディレクトリ名 `<slug>` はブランチ名から `feat/` を**除いた**もの — ブランチ
  `feat/zoom-indicator` → `../herdr-worktrees/zoom-indicator`。
- **ブランチ:** `develop` から `feat/<slug>`。GitHub issue がある場合は
  `issue/<id>-<slug>`。
- 編集・テスト・コミットはすべて worktree 内で行う。

```bash
git worktree add ../herdr-worktrees/<slug> -b feat/<slug> develop
```

PR を作るのは推奨 — この作業は使い捨ててよい。方針が合わなければブランチと
worktree を消すだけでよく、失うものは無い。

```bash
git worktree remove ../herdr-worktrees/<slug>
git branch -D feat/<slug>          # 併せて: git push origin --delete feat/<slug>
```

## プルリクエスト（フォーク内 → `develop`）

PR は**フォーク内の `develop`** を向き先にする。アップストリームにも `master` にも
向けない。

`gh` はフォークの PR を既定でアップストリームの親に向ける。さらに `gh pr create`
は非対話・バックグラウンド実行で**ハングする**（TTY が無い状態でベースリポジトリの
選択を促すため）。代わりに API で作る — 非対話で、構造上フォーク内に閉じる:

```bash
gh api repos/peinan/herdr/pulls \
  -f title='feat: ...' -F body=@<body-file> \
  -f head=<branch> -f base=develop --jq '.html_url'
```

owner プレフィックスの無い `head` は `peinan/herdr` 内に留まる。新規クローンで
`develop` がローカルにしか無い場合は、先に `git push origin develop`。TTY がある
場合の対話版: `gh pr create --repo peinan/herdr --base develop`。

PR が `develop` にマージされたら、その機能を
[`FORK_FEATURES.md`](./FORK_FEATURES.md) に追記する（上記「フォーク独自機能のカタログ」）。

## `AGENTS.md` から引き継ぐ規約（1 点だけ上書き）

- **「`master` へ反映する」手順は上書きする。** `AGENTS.md`（Multi-agent
  isolation）は共有チェックアウトを fast-forward して `origin/master` に push せよ
  と書くが、このフォークでは個人の作業は上記のフォーク内 PR で `develop` に
  ランディングする — `master` に push することはしない。`master` はアップストリーム
  同期でのみ動く。
- **コミット規約はそのまま適用:** 小文字の conventional commits、絵文字なし、
  **AI の co-author 行なし**。`refs #<n>`（`fixes`/`closes` ではなく）は実際の
  issue のみに使い、discussion には使わない。コミットメッセージは事前に提案して
  合意を取る。
- **ビルド・テストには `ZIG` の設定が必要** — mise 管理の zig は非対話シェルの
  PATH に乗らない:

```bash
ZIG=~/.local/share/mise/installs/zig/latest/bin/zig just check
```
