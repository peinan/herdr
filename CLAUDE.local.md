# 個人フォークの開発フロー (peinan/herdr)

`AGENTS.md`（`CLAUDE.md` がシンボリックリンクしているアップストリーム管理の規約）の上に
重ねるフォーク固有の指示。コードスタイル・テスト・画面検出・プロトコル等のエンジニアリング
実務は引き続き `AGENTS.md` が正。このファイルはこのフォークのブランチ運用・同期・取り込みを
扱い、**フォーク固有ワークフローで両者が食い違う場合はこのファイルを優先する**。

このファイルはフォーク専用。アップストリーム（`ogulcancelik/herdr`）には出さない。
`.gitignore` には追加しない。

> **詳細な開発フローは [`FORK_WORKFLOW.md`](docs/fork/FORK_WORKFLOW.md) が正。** 本ファイルは要点のみ。

## フォークの構成

アップストリーム `ogulcancelik/herdr` の個人フォーク。ブランチ / リモートの役割:

- **`main`** — 個人の開発・統合ライン。**既定ブランチ**。カスタマイズ・機能開発・upstream 取り込みはすべてここ。
- **`master`** — **リリース源**として保持（継承 CI / `preview.yml` / `release.yml` が master 前提）。リリース時のみ `git push origin main:master` で反映してワークフローを回す。普段は触らない。
- `origin` = `peinan/herdr`（push 先）。`upstream` = `ogulcancelik/herdr`（取り込み元・読み取り専用、`upstream/master` がミラーの実体）。

（旧 `develop` は 2026-07-23 に `main` へリネーム・廃止。旧「`master`＝アップストリームミラー」運用も終了。）

## upstream 追従（`upstream/master` → `main`）

GitHub の **"Sync fork" ボタンは使わない**（既定ブランチに紐づき、diverged した個人ブランチでは危険）。ローカルで直 merge する:

```bash
git fetch upstream
git checkout main && git merge upstream/master
just check
git push origin main
```

- コンフリクトは UI 改修箇所（タブバー / ペイン枠 / prefix・zoom）を **両取り**で解消。`-X ours`/`-X theirs` の一括指定は禁止（個人機能を黙って落とす／アップストリーム修正を巻き戻す）。
- **`main` を rebase しない**（公開済み・worktree 積み）。merge で取り込む。
- `rerere` + `merge.conflictStyle=zdiff3` は有効化済み（→ FORK_WORKFLOW §0）。
- `just check` は UI 機能の欠落を検出できない。マージ後に自分のカスタマイズの描画も目視する。
- 進行中の `feat/*` worktree が少ないときに取り込む。

## 機能開発 / PR

機能ごとに `main` から worktree を切る（`../herdr-worktrees/<slug>`、ブランチ `feat/<slug>`、issue があれば `issue/<id>-<slug>`）。編集・テスト・コミットは worktree 内。PR は**フォーク内 `main` 向き**。`gh pr create` はハングするので API で作る:

```bash
gh api repos/peinan/herdr/pulls \
  -f title='feat: ...' -F body=@<body-file> \
  -f head=feat/<slug> -f base=main --jq '.html_url'
```

マージ後、[`FORK_FEATURES.md`](docs/fork/FORK_FEATURES.md) に追記（差分基準 `git diff upstream/master...main`）。worktree は保持してよい（削除は都度確認）。

## `AGENTS.md` から引き継ぐ規約（フォークでの上書き）

- **「`master` へ反映する」手順の上書き:** `AGENTS.md`（Multi-agent isolation）は共有チェックアウトを fast-forward して `origin/master` に push せよと書くが、このフォークでは個人の作業は上記フォーク内 PR で `main` にランディングする。`master` はリリース時の `main:master` 反映でのみ動く。
- **アップストリーム管理ファイル（`AGENTS.md`, `README`, `.github/workflows/*`, `config/model.rs` の既定 等）に個人変更を入れない**（取り込みのたびのコンフリクトを抑えるため。個人設定はこのファイルやユーザ config に置く）。
- **コミット規約:** 小文字 conventional commits、絵文字なし、**AI の co-author 行なし**。`refs #<n>`（`fixes`/`closes` ではなく）は実 issue のみ。コミット文面は事前に提案して合意を取る。
- **ビルド・テストには `ZIG` の設定が必要**（mise 管理の zig は非対話シェルの PATH に乗らない）:

```bash
ZIG=~/.local/share/mise/installs/zig/latest/bin/zig just check
```
