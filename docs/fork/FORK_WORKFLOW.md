# herdr フォーク開発フロー (peinan/herdr)

`ogulcancelik/herdr` の個人フォーク `peinan/herdr` の開発・運用フローをまとめた詳細ドキュメント。

関連ファイル:

- [`CLAUDE.local.md`](../../CLAUDE.local.md) — エージェント向けフォーク規約（要点）。本書はその詳細版。
- [`FORK_FEATURES.md`](./FORK_FEATURES.md) — フォーク独自機能のカタログ（**何を**変えたか）。本書は**どう作業するか**。
- `AGENTS.md`（= `CLAUDE.md`）— アップストリーム管理の共通規約。コードスタイル・テスト・画面検出・プロトコル等のエンジニアリング実務はこちらが正。

> このファイルはフォーク限定。個人ブランチ（`main`）にのみ置き、アップストリームには入れない。

> **移行中の注意**: 現状のリポジトリはまだ `develop`(既定) + `master`(ミラー)。本書は**移行後の姿を正**として書いてある。`develop`+`master` から `main` 1本へ寄せる手順は末尾の[付録](#付録-現状-developmaster-から-main-1-本への移行一度だけ)にある。

## 全体像

- ブランチは **`main` 1 本**（個人の開発・統合ライン、既定ブランチ）。
- upstream ミラー用のブランチは**持たない**。`upstream/master`（リモート追跡ref）がその役割を担う。
- upstream の取り込みは **`upstream/master` を `main` に直接 merge**（完全にローカルで解決）。
- fork 関係は**維持**（切り離さない）。cross-repo PR 能力とチープな compare を残す。

### ブランチ / リモート

| 名前 | 役割 |
|---|---|
| `main` | 個人の開発・統合ライン。既定ブランチ。機能をここに積み、upstream もここに merge。|
| `feat/<slug>` / `issue/<id>-<slug>` | 機能ごとの作業ブランチ（worktree）。`main` から切る。|
| `origin` = `peinan/herdr` | 自分の push 先。|
| `upstream` = `ogulcancelik/herdr` | 取り込み元（読み取り専用）。`upstream/master` が“ミラー”の実体。|

`master` / `develop` ブランチは**廃止**。

## 0. 初回セットアップ（一度だけ）

```bash
# upstream リモート（fork 関係とは独立した、ローカルの取り込み経路）
git remote add upstream https://github.com/ogulcancelik/herdr.git
git fetch upstream

# conflict を楽にする設定（このリポジトリに保存。worktree 全体で共有される）
git config rerere.enabled true          # 解決内容を記録し、同じ衝突を自動再適用
git config merge.conflictStyle zdiff3   # 共通ベースを表示（古い git なら diff3）
```

- 自動アップデート通知は `~/.config/herdr/config.toml`（dotfiles 側）の `[update] version_check = false` で停止済み → [§5](#5-自動アップデート通知)。
- ビルド/テストには zig が要る（`build.rs` が `zig build` を実行）。非対話 PATH に無いので前置する:
  ```bash
  export ZIG=~/.local/share/mise/installs/zig/latest/bin/zig
  ```

## 1. 機能開発（日常の主作業）

機能ごとに `main` から worktree を切る。

```bash
git worktree add ../herdr-worktrees/<slug> -b feat/<slug> main   # issue があれば issue/<id>-<slug>

# 編集・テスト・コミットは worktree 内で
just check                                    # ZIG 前置が必要
git commit ...                                # 小文字 conventional / 文面は事前合意
git push origin feat/<slug>

# PR はフォーク内 main 向き（gh pr create はハングするので API を使う）
gh api repos/peinan/herdr/pulls \
  -f title='feat: ...' -F body=@<body-file> \
  -f head=feat/<slug> -f base=main --jq '.html_url'
```

`gh` はフォークの PR を既定でアップストリーム親に向け、`gh pr create` は非対話でハングする。上記 API 形式は owner プレフィックス無しの `head` で `peinan/herdr` 内に閉じる。TTY がある対話環境なら `gh pr create --repo peinan/herdr --base main` でもよい。

マージ後:

- [`FORK_FEATURES.md`](./FORK_FEATURES.md) に機能行・設定・変更履歴を追記（差分基準 `git diff upstream/master...main`。`git fetch upstream` 済みが前提）。
- worktree / ブランチは方針次第で保持 or 削除。

## 2. upstream 追従（定期）

**GitHub の "Sync fork" ボタンは使わない**。ボタンは既定ブランチに紐づき、diverged した個人ブランチでは「コミット破棄」か「拒否」になり危険。ローカルで直 merge する。

```bash
git fetch upstream
git checkout main
git merge upstream/master     # ← ローカルの 3-way merge。ブラウザは一切関与しない
# コンフリクトは UI 改修箇所（タブバー / ペイン枠 / prefix・zoom インジケータ等）を「両取り」で解消
just check
git push origin main
```

その後、フォークバイナリを更新するならローカル再ビルド → [§4](#4-フォークバイナリの更新再ビルド)。

### タイミング / 頻度

- **こまめに**（upstream リリース毎、または週次）取り込むほど 1 回あたりの conflict が小さい。溜めると痛い。
- 進行中の feat worktree が少ないタイミングで実施する（各 feat ブランチが古いベースの上に残るため）。

### conflict を楽にする（重要）

このフォークは UI 改修が upstream と同じ描画・設定ファイルを触るため、取り込みのたびに conflict が集中する。**ブランチの切り方（トポロジー）を変えても conflict の量は減らない**（直 merge も、使い捨てブランチも、mirror 経由も、同じ 3-way merge で同じ衝突になる）。効くのは以下:

1. **頻度**（小さいバッチで取り込む）← 最大の効き目。
2. **`git rerere`**（§0 で有効化済み）— 同じ hunk の再衝突を自動で再解決。カスタマイズ箇所が毎回ぶつかるフォークに刺さる。
3. **`zdiff3`**（§0）+ 3-way mergetool — 誰が何を変えたか一目で分かる。
4. すべて**ローカル解決**。`git merge upstream/master` は fetch 済みの追跡ref を merge するだけで完全にローカル。「ブラウザ解決」になるのは GitHub 上で PR を Merge した時だけなので、**PR 経由で upstream を取り込まない**。
5. **rebase しない**。個人コミットを乗せ直すと同じ箇所を何度も解決する羽目になり、公開済み＋worktree 積みで禁止でもある。merge が正解（過去の解決が `main` の履歴に焼き込まれ、再解決が不要になる）。

## 3. バージョン

- バージョンは `Cargo.toml` の `version` をコンパイル時に埋め込むだけ（`build_info::BASE_VERSION = env!("CARGO_PKG_VERSION")`）。git ハッシュ等は入らない。
- `Cargo.toml` の `version` は **upstream 管理**。upstream のリリースコミットが version を bump し、それが `upstream/master` 経由で merge に相乗りして入る。→ **バージョンは自動で upstream に追従**する（手作業不要）。
- **`Cargo.toml` の `version` を手で書き換えないこと**。同期のたびに衝突し、版比較もずれる。
- 副作用: 純正版とフォーク版が同一バージョンを名乗り、バイナリだけでは区別できない（許容）。

## 4. フォークバイナリの更新（＝再ビルド）

`herdr update` は**使わない**。アップデータは upstream（herdr.dev）を見るので、実行すると upstream の公式バイナリでフォークが上書きされ、フォーク機能が消える。取り込み後にローカルでビルドし直す:

```bash
just build          # → target/release/herdr
# PATH 上の herdr に入れ替え（例: cargo install --path .）
```

## 5. 自動アップデート通知

- アップデータは `herdr.dev/latest.json`（stable）/ `herdr.dev/preview.json`（preview）を参照＝**upstream 向き**。フォークのバージョン < upstream 最新 で「update ready」通知が点灯する。
- 通知は**通知のみ**（自動インストールはしない）。ただし紛らわしく、誤って `herdr update` すると危険。
- 停止設定（ユーザ config = dotfiles 側の `~/.config/herdr/config.toml`）:
  ```toml
  [update]
  version_check = false
  # manifest_check = false   # 任意: エージェント検出マニフェストの取得も止める場合
  ```
- **コード側の既定（`config/model.rs` の `true`）は書き換えないこと**。upstream 管理ファイルなので同期で衝突する。

## 6. 他人へフォーク版を配る

公式インストール経路（Homebrew / Nix / install script / `herdr update`）は全部 upstream 向きなので**使えない**。選択肢:

| 方法 | 手順 | 備考 |
|---|---|---|
| **A. fork release** | 継承した Preview / release ワークフローを手動 dispatch → `main` からバイナリをビルドして `peinan/herdr` の Release に添付。相手は自分の OS/arch のバイナリを DL → `chmod +x` → PATH。| 非開発者向けに楽。過去に Preview ワークフローで全プラットフォームのバイナリを Release 添付した実績あり。⚠️ macOS 未署名 → Gatekeeper（`xattr -d com.apple.quarantine ./herdr`）。ワークフローの upstream 前提の副作用（`website/preview.json` 等）は手で触らない。|
| **B. ソースビルド** | `cargo install --git https://github.com/peinan/herdr --branch main herdr`（Rust + Zig 必要。`zig` が PATH に無ければ `ZIG=<path>` 前置）| 開発者向け・こちら側のインフラ不要。|
| **C. バイナリ直渡し** | `target/release/herdr` を渡す | 1 人・同一 OS/arch・即席。macOS は同様に Gatekeeper 対応。|

**配る相手にも必ず伝える**: `version_check = false` を設定する / `herdr update` を実行しない（さもないと upstream バイナリで上書きされる）。ライセンスは root の `LICENSE` の条件下で再配布可（配布物に同梱）。

## やってはいけないこと

- `main` をアップストリームや誤って復活させた `master` にマージ／混同する（upstream 追従が壊れる）。
- `main` の rebase（公開済み・worktree が積まれている）。
- `Cargo.toml` の `version` を手で書き換え。
- "Sync fork" ボタン / `herdr update`。
- **PR 経由での upstream 取り込み**（ブラウザ conflict 解決になり詰む）。
- upstream 管理ファイル（`AGENTS.md`, `README`, `config/model.rs` の既定など）への個人変更。

## コミット規約（AGENTS.md 準拠）

小文字 conventional commits、絵文字なし、**AI co-author 行なし**。`refs #<n>` は実 issue のみ（`fixes`/`closes` は使わない = release CI 側で閉じる）。コミット文面は事前提案して合意。

---

## 付録: 現状 (`develop`+`master`) から `main` 1 本への移行（一度だけ）

現状は `develop`(既定・個人) + `master`(ミラー)。以下で `main` 1 本へ寄せる。

```bash
# 1. develop を main にリネームして push
git checkout develop
git branch -m develop main
git push -u origin main

# 2. GitHub の既定ブランチを main に変更（UI: Settings → Branches、または API）
gh api -X PATCH repos/peinan/herdr -f default_branch=main

# 3. 旧 develop を削除
#    ※ 既定を main にした後で行う。develop を base にした未マージ PR があれば
#      先に base を main へ付け替える（放置して削除すると PR が閉じられる）。
git push origin --delete develop

# 4. master（ミラー）を削除
git branch -D master
git push origin --delete master

# 5. ローカルの origin/HEAD を main に更新
git remote set-head origin main
```

移行後に追随させるもの:

- **feat / issue 各ブランチ**: commit SHA は不変なので中身は無事。以降の新規は `main` から切る。
- **[`CLAUDE.local.md`](../../CLAUDE.local.md)**: 「develop」表記を「main」に、同期は upstream リモート経由（"Sync fork" ボタン不使用）に更新。
- **[`FORK_FEATURES.md`](./FORK_FEATURES.md)**: 再生成の差分基準を `git diff master...develop` → `git diff upstream/master...main` に。
- **エージェントのメモリ**（PR ワークフロー）: PR の base を `develop` → `main` に。
