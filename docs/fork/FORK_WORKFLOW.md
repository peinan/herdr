# herdr フォーク開発フロー (peinan/herdr)

`ogulcancelik/herdr` の個人フォーク `peinan/herdr` の開発・運用フローをまとめた詳細ドキュメント。

関連ファイル:

- [`CLAUDE.local.md`](../../CLAUDE.local.md) — エージェント向けフォーク規約（要点）。本書はその詳細版。
- [`FORK_FEATURES.md`](./FORK_FEATURES.md) — フォーク独自機能のカタログ（**何を**変えたか）。本書は**どう作業するか**。
- `AGENTS.md`（= `CLAUDE.md`）— アップストリーム管理の共通規約。コードスタイル・テスト・画面検出・プロトコル等のエンジニアリング実務はこちらが正。

> このファイルはフォーク専用。アップストリームには出さない。

## 全体像

- 開発は **`main`**（個人の開発・統合ライン、既定ブランチ）。
- **`master`** は**リリース源**として保持（継承 CI / `preview.yml` / `release.yml` が master 前提のため）。リリース時のみ `main` を `master` へ反映して使う（→ §6）。普段の開発では触らない。
- upstream をミラーする専用ブランチは**持たない**。`upstream/master`（リモート追跡ref）がミラーの実体で、取り込みは **`upstream/master` を `main` に直接 merge**（完全にローカルで解決）。
- fork 関係は**維持**（切り離さない）。cross-repo PR 能力とチープな compare を残す。

### ブランチ / リモート

| 名前 | 役割 |
|---|---|
| `main` | 個人の開発・統合ライン。既定ブランチ。機能をここに積み、upstream もここに merge。|
| `master` | **リリース源**。継承 CI / `preview.yml` / `release.yml` が参照。リリース時に `main` を反映（→ §6）。普段は触らない。|
| `feat/<slug>` / `issue/<id>-<slug>` | 機能ごとの作業ブランチ（worktree）。`main` から切る。|
| `origin` = `peinan/herdr` | 自分の push 先。|
| `upstream` = `ogulcancelik/herdr` | 取り込み元（読み取り専用）。`upstream/master` が“ミラー”の実体。|

旧 `develop` は `main` へリネーム済み（→ 付録）。旧「`master`＝アップストリームミラー」運用は終了し、`master` はリリース源に転用。

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
- ツールチェーンは mise で入れる（`rust-toolchain.toml` は 1.96.1、Zig は `vendor/libghostty-vt/build.zig.zon` の `minimum_zig_version` = 0.16.0）:
  ```bash
  mise use -g rust@1.96.1 just@latest bun@1.3.14 cargo:cargo-nextest node@25
  mise install zig@0.16.0   # 設定ファイルには書かず、使うときに指定する
  ```
  `just check` は Rust に加えて **bun と node** も使う（`docs-contract-test` / `integration-assets-test`）。
- ビルドとテストには zig 0.16.0 が要る（upstream v0.9.1 から。`crates/ghostty-vt/build.rs` が `$ZIG build`、`ZIG` が無ければ PATH の `zig build` を実行する）。
  mise の zig はどの設定ファイルにも書いていないので、`mise exec -- ...` だけでは PATH に乗らない。`ZIG` で直接渡す:
  ```bash
  ZIG=$HOME/.local/share/mise/installs/zig/0.16.0/zig mise exec -- just check
  ```
  `make build` / `make install` は `mise exec zig@0.16.0` で PATH に載せるので、前置は要らない（→ §4）。
- zig 0.16.0 は、この Mac の Command Line Tools の SDK（26.5 / 27.0）にそのままリンクできる（2026-11-06 に確認）。
  zig 0.15.2 の頃の回避策（旧 SDK を返す xcrun シムのラッパ `~/.local/share/herdr-fork/zig` と、ビルド済み `zig-out` の流用 +
  `ZIG=/usr/bin/true`）は要らなくなった。ラッパのリンク先の MacOSX15.4.sdk は 2026-09-28 の CLT 更新で消えており、
  libghostty-vt の出力先も upstream #4661 で cargo の `$OUT_DIR` に移ったので、どちらももう使えない。
- herdr セッションの中でテストを走らせるときは `HERDR_*` 環境変数を全部外す（PTY 系テストが誤動作する）:
  `env -u HERDR_ENV -u HERDR_PANE_ID -u HERDR_SOCKET_PATH -u HERDR_CLIENT_SOCKET_PATH -u HERDR_TAB_ID -u HERDR_WORKSPACE_ID -u HERDR_BIN_PATH -u HERDR_STARTUP_CWD ZIG="$HOME/.local/share/mise/installs/zig/0.16.0/zig" mise exec -- just check`
  `env` を前置する形では `ZIG=~/...` のチルダが展開されない（`env` の引数になるため）ので `$HOME` で書く。

### 既知の環境依存テスト失敗（macOS 26.6）

**この Mac では `just check` は素の upstream でも緑にならない。** 2026-11-06 の 0.9.5 sync で測り直した
（素の upstream `2f3432c6` と同期後のブランチを、それぞれ `cargo nextest run --no-fail-fast` で全件実行）。

| バイナリ | テスト | 状況 |
|---|---|---|
| `herdr`（単体テスト） | `platform::macos::config_file_tests::config_replacement_preserves_macos_acl` | 毎回失敗。素の upstream でも同じ |
| `live_handoff` | `live_handoff_preserves_http_servers_across_multiple_sessions`、`live_handoff_preserves_python_http_server` | 素の upstream の全件実行で失敗。同期後の全件実行では通った |
| `live_handoff` | `live_handoff_keeps_agent_started_pane_after_agent_exits` | 全件実行で 1 回だけ失敗。単体では 3/3 通過 |
| `remote_attach` | `ssh_check_message_is_visible_while_authentication_waits` | 落ちることが多い。テストが書いた偽の ssh スクリプトの初回実行に約 0.8 秒かかるため。交互に 8 回ずつ回すと、素の upstream は 8 回とも、同期後は 7 回失敗 |
| `herdr`（単体テスト） | `app::api::plugins::tests::plugin_pane_open_uses_plugin_root_title_env_and_target_context` | 負荷が高いと落ちる（書き込み途中のファイルを読む）。単体では 10/10 通過 |

nextest の `LEAK` 表示（`persist::writer::tests::snapshot_history_is_bounded_and_does_not_rotate_identical_layouts` など）は成否に影響しない。
0.9.0 の頃の一覧（`live_handoff_preserves_pane_process_io`、`api_ping`、`multi_client`、`client_mode::federated_*` など）は、今回の全件実行では再現しなかった。

`just check` は `ci`（`lint` と `ci-tests`）、`windows-lint`、`docs-contract-test` を順に走らせ、最初の失敗で止まる。
`windows-lint` は xwin と Microsoft の SDK（`just setup-windows-cross`、ライセンス同意が要る）が無いと通らない。
そのためローカルでは `just lint`、`cargo nextest run --no-fail-fast`、`just maintenance-test`、`just ui-hot-path-architecture-test`、
`just integration-assets-test`、`just docs-contract-test` を個別に回し、Windows は CI に任せる。
nextest の失敗は上の一覧と照合する。**失敗件数だけを見て自分の変更のせいだと判断しない。**
同じ系統は CI の macOS ジョブでもたまに落ちる（実例: [#46](https://github.com/peinan/herdr/pull/46) の
`federated_launch_opens_local_directly_while_saved_ssh_is_unavailable`）。その場合は失敗ジョブを再実行して切り分ける。

### CI でだけ観測されたフレーク

ローカルの上表とは別に、CI で再実行すると通る（＝差分と無関係な）ものとして観測済み。#51 / #52 で遭遇。

| ジョブ | テスト |
|---|---|
| `check (macos-latest)` | `cross_area::cross_area_client_and_api_workspace_views_are_consistent` |
| `check (windows-latest)` | `sound::tests::windows_media_player_reports_invalid_media_without_waiting_for_timeout` |

### 「既知だろう」で流さない

**上のどの一覧にも無い失敗は、必ず `gh run view --job <id> --log-failed` でログを引いて失敗テスト名を特定する。**
#52 の作業では「同系統の既知フレークだろう」という見立てが 3 回とも外れ、いずれも自分が入れたテストの実バグだった:

- macOS — 1 サーバに 2 クライアントを繋ぐ構成で、どちらが foreground かに依存する順序依存テスト。
- Windows で 540 秒ハング — テストが `exiting_test_command()` を設定せず実シェルを spawn していた。
  macOS / Linux では 0.12 秒で返るため表面化せず、**全プラットフォームで実プロセスをリークしていた**。
- macOS — `ClientWriter::test_channel` は render を直結・control を背景スレッド経由で送るため、
  `render_rx.recv()` が返っても control の到着は保証されない、というテストハーネスのレース。

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

- 複数リリース分遅れた場合は **段階マージ**する（1 段 merge → 検証 → commit → 次の段）。
  各段の解決が `rerere` に記録され、最後の段の衝突が「フォーク独自変更 vs upstream の書き直し」に絞れる。
  - **区切りはリリースタグではなく、master 上の同期点にする。** upstream は公開済みのプレビューをリリースブランチで stable に昇格させるので、
    `v0.9.x` のタグは master の祖先ではない。master には各リリースのあとに `release: synchronize metadata for vX` が積まれる:
    ```bash
    git log --format='%h %ad %s' --date=short upstream/master --grep='^release: synchronize metadata for v' | head
    git merge --no-ff --no-commit <同期点の SHA>
    ```
    同期点で区切れば master の系列から外れないので、最後の段はそのまま `git merge upstream/master` にできる。
  - **手順やメモはタグ名ではなく SHA で書く。** グローバル設定の `fetch.pruneTags=true` により、origin を fetch するたびに upstream のタグが消える。
  - 2026-09-10 の 0.9.0 sync（367 commits、upstream #3487 の shell クライアント化を含む）はタグ単位（v0.8.0 → v0.8.2 → master）で、
    2026-11-06 の 0.9.5 sync（300 commits）は同期点単位（`956f23ff` → `f9a99bd2` → `2f3432c6`）で段階マージし、
    どちらも `main` へ `--ff-only` で取り込んだ（経緯は [`FORK_FEATURES.md`](FORK_FEATURES.md) の変更履歴）。
- upstream 側のパス変更: `website/latest.json` / `website/preview.json` は `distribution/` 配下へ移動（0.9.0）。

その後、フォークバイナリを更新するならローカル再ビルド → [§4](#4-フォークバイナリの更新再ビルド)。

### 次回の同期で扱うこと（2026-11-06 時点、`2f3432c6` より後の upstream）

- **`CLAUDE.md` の削除**（`b796ca60`、AGENTS.md へのシンボリックリンクを削除）: 削除に従う。
  代わりに `CLAUDE.local.md` の冒頭へ `@AGENTS.md` を足し、Claude Code が引き続き AGENTS.md を読むようにする（2026-11-06 にユーザーが決定）。
- **#5583**（`de8503c1`、クリップボード補助プロセスにタイムアウトを設け、toast を書き込みのあとに出す）: フォークの非同期ワーカー（fork.10 #58）と衝突する。
  フォークは今 toast を書き込みの前に出しているので、書き込み後に出すかどうかを、取り込むときに決める（→ FORK_FEATURES の 0.9.5 sync）。

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

フォーク版は **`0.9.0-fork.9`** のように名乗る。base（`0.9.0`）は upstream 追従、`fork.9` がフォーク機能の世代。

- `Cargo.toml` の `version` は **upstream 管理**。upstream のリリースコミットが bump し、それが `upstream/master` 経由で merge に相乗りして入る → **base は自動で追従**する。
- **`Cargo.toml` の `version` を手で書き換えないこと**。同期のたびに衝突し、版比較もずれる。上げたいのはフォーク側の世代なら `HERDR_BUILD_ID` を上げる。
- `fork.N` は `Makefile` の `HERDR_BUILD_ID` が持ち、`HERDR_BUILD_CHANNEL=fork` とともにビルド時に渡る。
  `src/build_info.rs` の `option_env!` を使うだけなので **Rust ソースの変更はゼロ**（`{BASE_VERSION}-{channel}.{build_id}` を既存コードが組み立てる）。
- **採番ルール**: 1 まとまったトピック = 1 世代。単調増加で、**upstream sync では上げない**（番号が表すのはフォーク機能の集合なので、base だけ動いたときは据え置き）。世代の定義は [`FORK_FEATURES.md`](./FORK_FEATURES.md) の変更履歴の見出しそのもの。
- これで 2 軸が独立して読める。`0.9.0-fork.9` と `0.10.0-fork.9` ならフォーク機能は同一で upstream だけ新しく、`0.9.0-fork.8` と `0.9.0-fork.9` なら upstream は同じでフォーク機能が 1 世代進んでいる。

### env をビルド経路の外に出さないこと

`HERDR_BUILD_CHANNEL` / `HERDR_BUILD_ID` は **`Makefile` の `build` / `install` ターゲットだけ**に置く。
`justfile` に入れたりシェルで export すると、版文字列を `env!("CARGO_PKG_VERSION")` で期待している
テスト（`tests/api_ping.rs`、`tests/cli/sessions.rs`、`src/app/mod.rs` の release notes 判定）が落ちる。
`just check` は素の `0.9.0` で走らせる。

### 既知の副作用

| 事象 | 内容 |
|---|---|
| 異プラットフォームへの SSH attach | ローカルバイナリでリモートを seed できない（別 OS/arch）場合、`src/remote/attach.rs` が stable manifest から `0.9.0-fork.N` の asset を探して必ず失敗する。同一プラットフォームならローカルバイナリをコピーするので無影響。回避は `HERDR_REMOTE_BINARY` の明示指定 |
| live handoff | `src/server/handoff.rs` の `expected_version` は文字列完全一致なので、`HERDR_BUILD_ID` が違う新旧バイナリ間の handoff は拒否される（ビルドが違えば拒否されるのはむしろ正しい） |
| release notes / product announcement | 既読管理が文字列キー照合なので再表示されうる。update 無効化運用なので実害は薄い |
| `herdr status` の `server_binary_stale` | `N` を上げてサーバ再起動前は `true` になる（意図どおり） |

**無影響**: `update.rs` の版比較（`Version::current()` は suffix を含まない `BASE_VERSION` を使う）、endpoint handshake（版ではなく codec を交渉する）、`is_preview()` 分岐（`fork` は preview 扱いにならない）。

## 4. フォークバイナリの更新（＝再ビルド）

`herdr update` は**使わない**。アップデータは upstream（herdr.dev）を見るので、実行すると upstream の公式バイナリでフォークが上書きされ、フォーク機能が消える。取り込み後にローカルでビルドし直す:

```bash
# フォーク機能に変化があったなら、まず Makefile の HERDR_BUILD_ID を上げて
# FORK_FEATURES.md に対応する世代の見出しを足す（→ §3 の採番ルール）
HERDR_BUILD_CHANNEL=fork HERDR_BUILD_ID=10 \
  mise exec zig@0.16.0 -- just build                           # → target/release/herdr
cp ~/.local/bin/herdr ~/.local/bin/herdr.prev                  # 現行を退避
install -m 0755 target/release/herdr ~/.local/bin/herdr        # 入れ替え（`herdr server stop` → 再起動で反映）
herdr --version                                                # → herdr 0.9.5-fork.10
```

**`make install` 一発でも同じ結果になる。** `Makefile` が `HERDR_BUILD_ID` を渡し、zig 0.16.0 を
`mise exec zig@0.16.0` で PATH に載せるので、前置は要らない:

```bash
make install        # ビルド → ~/.local/bin/herdr へ設置
herdr --version     # → herdr 0.9.5-fork.10
```

`make build HERDR_BUILD_ID=11` で番号だけ一時上書きもできる。別の zig を使うときは `ZIG=<zig のパス>` を前置する。

env を付けずにビルドすると素の `0.9.5` になり、upstream のバイナリと見分けが付かなくなる。

## 5. 自動アップデート通知

- アップデータは `herdr.dev/latest.json`（stable）/ `herdr.dev/preview.json`（preview）を参照＝**upstream 向き**。フォークのバージョン < upstream 最新 で「update ready」通知が点灯する。
- `-fork.N` を付けても**この判定は変わらない**。`update.rs` の `Version::current()` は suffix を含まない `BASE_VERSION`（= `0.9.0`）を使うため、採番の導入前後で挙動は同一。
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
| **A. fork release** | **今は使えない（0.9.5 sync 以降）。** 取り込んだ `preview.yml` / `release.yml` は `preview-*` / `v*` タグの push でだけ起動し、どのジョブも `github.repository == 'herdrdev/herdr'` のときしか走らない。手動 dispatch も無いので、フォークでは何も起きない。以前は ①`git push origin main:master` → ②`preview.yml` を手動 dispatch → ③`peinan/herdr` の Release にバイナリ、という手順だった。| 戻すにはフォーク用のリリースワークフローを別に用意する。それまでは B か C で配る。手動 dispatch できる `build-artifacts-manual.yml` は残っているが、成果物は Actions の Artifacts（保持 7 日）で Release には付かず、`HERDR_BUILD_ID` も渡らないので素の `0.9.5` を名乗る（フォークでの実行は未確認）。macOS のバイナリは未署名なので、受け取った側は Gatekeeper の解除（`xattr -d com.apple.quarantine ./herdr`）が要る。|
| **B. ソースビルド** | `cargo install --git https://github.com/peinan/herdr --branch main herdr`（Rust + Zig 0.16.0 必要。`zig` が PATH に無ければ `ZIG=<path>` 前置）| 開発者向け・こちら側のインフラ不要。⚠️ この経路は `Makefile` を通らないので `HERDR_BUILD_ID` が渡らず、素の `0.9.5` を名乗る（`HERDR_BUILD_CHANNEL=fork HERDR_BUILD_ID=N` を前置すれば付く）。|
| **C. バイナリ直渡し** | `target/release/herdr` を渡す | 1 人・同一 OS/arch・即席。macOS は同様に Gatekeeper 対応。|

**配る相手にも必ず伝える**: `version_check = false` を設定する / `herdr update` を実行しない（さもないと upstream バイナリで上書きされる）。ライセンスは root の `LICENSE` の条件下で再配布可（配布物に同梱）。

## やってはいけないこと

- `master` にリリース反映（`main:master` push）以外の個人開発を直接積む／混同する（`master` はリリース源専用）。
- `main` の rebase（公開済み・worktree が積まれている）。
- `Cargo.toml` の `version` を手で書き換え（上げたいのがフォーク側の世代なら `Makefile` の `HERDR_BUILD_ID`）。
- "Sync fork" ボタン / `herdr update`。
- **PR 経由での upstream 取り込み**（ブラウザ conflict 解決になり詰む）。
- upstream 管理ファイル（`AGENTS.md`, `README`, `config/model.rs` の既定など）への個人変更。

## コミット規約（AGENTS.md 準拠）

小文字 conventional commits、絵文字なし、**AI co-author 行なし**。`refs #<n>` は実 issue のみ（`fixes`/`closes` は使わない = release CI 側で閉じる）。コミット文面は事前提案して合意。

---

## 付録 A: `develop` → `main` 移行（2026-07-23 完了）

`develop`（旧・既定/個人）を `main` にリネームし、既定ブランチを `main` に変更、旧 `develop` を削除した。
**`master` は削除せずリリース源として保持**（当初案の「master 削除」は取りやめ — 継承ワークフローが master 前提のため）。実施内容:

```bash
git branch -m develop main
git push -u origin main
gh api -X PATCH repos/peinan/herdr -f default_branch=main   # 既定を main に
git push origin --delete develop                            # 旧既定を削除
git remote set-head origin main
```

feat/issue 各ブランチは commit SHA 不変で無傷（以降の新規は `main` から切る）。

## 付録 B: リリース手順（`main` → `master`）

**0.9.5 sync（2026-11-06）以降、この手順ではリリースできない**（→ §6 A）。取り込んだ `preview.yml` / `release.yml` はタグの push でだけ起動し、
`herdrdev/herdr` 以外では全ジョブが飛ばされる。以下は以前の手順の記録。

継承 CI / `preview.yml` / `release.yml` は `master` を参照する。フォークのバイナリを配るときだけ `main` を `master` へ反映してワークフローを回す（§6 A）:

```bash
git push origin main:master        # master をリリース点へ（diverge 時は --force-with-lease）
# GitHub Actions で preview.yml（or release.yml）を dispatch → Release にバイナリ
```
