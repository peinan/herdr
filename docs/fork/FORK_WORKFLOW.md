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
- ツールチェーンは mise で入れる（`rust-toolchain.toml` は 1.96.1、Zig は `vendor/libghostty-vt/build.zig.zon` の 0.15.2）:
  ```bash
  mise use -g rust@1.96.1 just@latest bun@1.3.14 cargo:cargo-nextest
  mise use -g zig@0.15.2 node@25   # 未導入なら
  ```
  `just check` は Rust に加えて **bun と node** も使う（`docs-contract-test` / `integration-assets-test`）。
- ビルド/テストには `ZIG` の指定が要る（`build.rs` が `zig build` を実行。mise の zig は非対話 PATH に乗らない）。
  さらに **macOS 26 の Command Line Tools SDK（MacOSX26.x.sdk）では `libSystem.tbd` に `arm64-macos` ターゲットが無く、
  zig 0.15.2 は libSystem を一切リンクできない**（`undefined symbol: _exit` 等が大量に出る）。zig は PATH 上の
  `xcrun --show-sdk-path` で SDK を探すので、旧 15.4 SDK を返す `xcrun` シムを zig だけに見せるラッパを使う
  （`SDKROOT` や `zig build --sysroot` はビルドランナー自身のリンクには効かない）:
  ```bash
  mkdir -p ~/.local/share/herdr-fork/bin
  cat > ~/.local/share/herdr-fork/bin/xcrun <<'EOF'
  #!/bin/sh
  case " $* " in
    *" --show-sdk-path "*) echo /Library/Developer/CommandLineTools/SDKs/MacOSX15.4.sdk ;;
    *" --show-sdk-version "*) echo 15.4 ;;
    *) exec /usr/bin/xcrun "$@" ;;
  esac
  EOF
  cat > ~/.local/share/herdr-fork/zig <<'EOF'
  #!/bin/sh
  export PATH="$HOME/.local/share/herdr-fork/bin:$PATH"
  exec "$HOME/.local/share/mise/installs/zig/0.15.2/zig" "$@"
  EOF
  chmod +x ~/.local/share/herdr-fork/bin/xcrun ~/.local/share/herdr-fork/zig
  export ZIG=~/.local/share/herdr-fork/zig
  mise exec -- just check
  ```
  15.4 SDK が CLT から消えたら、`arm64-macos` を含む別 SDK を指す。
- herdr セッションの中でテストを走らせるときは `HERDR_*` 環境変数を全部外す（PTY 系テストが誤動作する）:
  `env -u HERDR_ENV -u HERDR_PANE_ID -u HERDR_SOCKET_PATH -u HERDR_CLIENT_SOCKET_PATH -u HERDR_TAB_ID -u HERDR_WORKSPACE_ID ZIG="$HOME/.local/share/herdr-fork/zig" mise exec -- just check`
  `env` を前置する形では `ZIG=~/...` のチルダが展開されない（`env` の引数になるため）ので `$HOME` で書く。

### 既知の環境依存テスト失敗（macOS 26.6）

**この Mac では `just check` は素の `main` でも緑にならない。** 以下 7 件は PTY / SSH / マルチクライアントの
統合テストで、フォークの変更とは無関係に失敗する（pristine な `main` で同一に再現することを 2026-09-11 に確認）。

| バイナリ | テスト |
|---|---|
| `live_handoff` | `live_handoff_preserves_pane_process_io`（`os error 22`） |
| `live_handoff` | `live_handoff_keeps_unmanaged_agent_name_bound_to_saved_session`（`/bin/sleep` の環境変数が他プロセスから見えず `HERDR_AGENT` ヒントを検出できない） |
| `live_handoff` | `live_server_holds_one_pty_master_fd_per_pane` |
| `api_ping` | `events_subscribe_streams_output_and_agent_status_events` |
| `multi_client` | `same_tab_geometry_follows_meaningful_client_activity` |
| `multi_client` | `api_pane_output_is_fanned_out_as_pane_surface_updates` |
| `client_mode` | `federated_*`（実行ごとに `federated_client_starts_without_local_and_survives_its_restart` / `federated_launch_opens_local_directly_while_saved_ssh_is_unavailable` のどちらかが落ちる） |

`just check` は fail-fast で最初の数件で止まるため、判定には必ず `cargo nextest run --no-fail-fast` を使い、
上の一覧と照合する。**失敗件数だけを見て自分の変更のせいだと判断しない。**
同じ系統は CI の macOS ジョブでもたまに落ちる（実例: [#46](https://github.com/peinan/herdr/pull/46) の
`federated_launch_opens_local_directly_while_saved_ssh_is_unavailable`）。その場合は失敗ジョブを再実行して切り分ける。

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

- 複数リリース分遅れた場合は **リリースタグ単位で段階マージ**する（`git merge v0.8.0` → `just check` → commit → 次のタグ）。
  各段の解決が `rerere` に記録され、最後の段の衝突が「フォーク独自変更 vs upstream の書き直し」に絞れる。
  2026-09-10 の 0.9.0 sync（367 commits、upstream #3487 の shell クライアント化を含む）は `sync/upstream-0.9.0` worktree で
  この手順を踏み、`main` へ `--ff-only` で取り込んだ（経緯は [`FORK_FEATURES.md`](FORK_FEATURES.md) の変更履歴）。
- upstream 側のパス変更: `website/latest.json` / `website/preview.json` は `distribution/` 配下へ移動（0.9.0）。

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
ZIG=~/.local/share/herdr-fork/zig mise exec -- just build   # → target/release/herdr（§0 の zig ラッパ前提）
cp ~/.local/bin/herdr ~/.local/bin/herdr.prev                  # 現行を退避
install -m 0755 target/release/herdr ~/.local/bin/herdr        # 入れ替え（`herdr server stop` → 再起動で反映）
```

`make build` / `make install` は `mise exec zig@0.15.2 -- just build` を呼ぶため、§0 の xcrun シムが PATH に無いと
macOS 26 の SDK 問題でリンクに失敗する。当面は上記のように `ZIG` にラッパを渡す。

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
| **A. fork release** | ①`git push origin main:master` で `master` をリリース点へ進める → ②`preview.yml`（or `release.yml`）を手動 dispatch（**master 参照**でビルド）→ ③`peinan/herdr` の Release にバイナリが添付される。相手は自分の OS/arch を DL → `chmod +x` → PATH。| 非開発者向けに楽。全プラットフォーム添付の実績あり。⚠️ macOS 未署名 → Gatekeeper（`xattr -d com.apple.quarantine ./herdr`）。ワークフローの upstream 前提の副作用（`website/preview.json` 等）は手で触らない。|
| **B. ソースビルド** | `cargo install --git https://github.com/peinan/herdr --branch main herdr`（Rust + Zig 必要。`zig` が PATH に無ければ `ZIG=<path>` 前置）| 開発者向け・こちら側のインフラ不要。|
| **C. バイナリ直渡し** | `target/release/herdr` を渡す | 1 人・同一 OS/arch・即席。macOS は同様に Gatekeeper 対応。|

**配る相手にも必ず伝える**: `version_check = false` を設定する / `herdr update` を実行しない（さもないと upstream バイナリで上書きされる）。ライセンスは root の `LICENSE` の条件下で再配布可（配布物に同梱）。

## やってはいけないこと

- `master` にリリース反映（`main:master` push）以外の個人開発を直接積む／混同する（`master` はリリース源専用）。
- `main` の rebase（公開済み・worktree が積まれている）。
- `Cargo.toml` の `version` を手で書き換え。
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

継承 CI / `preview.yml` / `release.yml` は `master` を参照する。フォークのバイナリを配るときだけ `main` を `master` へ反映してワークフローを回す（§6 A）:

```bash
git push origin main:master        # master をリリース点へ（diverge 時は --force-with-lease）
# GitHub Actions で preview.yml（or release.yml）を dispatch → Release にバイナリ
```
