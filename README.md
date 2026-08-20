# gh-pages — フォーク紹介ページの配信ブランチ

https://peinan.github.io/herdr/ の配信元。main/master とは履歴を共有しない orphan ブランチ。

- `index.html` — ランディングページ本体(自己完結の静的 HTML、ビルド不要)
- 内容は `main` の [`docs/fork/FORK_FEATURES.md`](https://github.com/peinan/herdr/blob/main/docs/fork/FORK_FEATURES.md) を基に作成。フォークに機能を積んだら両方更新する。

## 更新のしかた

worktree `../herdr-worktrees/gh-pages` で `index.html` を編集して push するだけ(GitHub Pages が自動再デプロイ)。

```bash
git -C ../herdr-worktrees/gh-pages add -A
git -C ../herdr-worktrees/gh-pages commit -m "docs: update landing page"
git -C ../herdr-worktrees/gh-pages push
```

動作確認はブラウザで `index.html` を直接開く。
