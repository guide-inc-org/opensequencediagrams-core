# OpenSequenceDiagrams Core リネーム指示書

## 背景

WebSequenceDiagrams (WSD) の代替サービスとして「OpenSequenceDiagrams (OSD)」を立ち上げることになりました。

- ドメイン: `opensequencediagrams.com` （取得済み）
- 運営: Guide Inc.

現在の `guideline` リポジトリを `opensequencediagrams-core` にリネームし、関連するソースコード内の記述も更新します。

## リポジトリ構成（変更後）

```
guide-inc-org/
├── opensequencediagrams-core  (OSS、パーサー/レンダラー) ← 今回の対象
├── opensequencediagrams-web   (Private、Webサービス本体) ← 今後作成
```

## 変更内容

### 1. GitHub リポジトリ名の変更（手動）

- 変更前: `guide-inc-org/guideline`
- 変更後: `guide-inc-org/opensequencediagrams-core`

※ これは GitHub の Settings から手動で行う

### 2. ソースコード内の変更

以下の項目を検索・置換してください：

| 変更前 | 変更後 |
|--------|--------|
| `guideline` (パッケージ名) | `osd-core` |
| `guide-inc-org/guideline` | `guide-inc-org/opensequencediagrams-core` |
| `@guide-inc-org/guideline` | `@opensequencediagrams/core` |

### 3. 具体的なファイル更新

#### Cargo.toml
- `name = "guideline"` → `name = "osd-core"`
- リポジトリURLの更新

#### package.json（存在する場合）
- `name` を `@opensequencediagrams/core` に変更
- `repository` URLの更新

#### README.md
- プロジェクト名を「OpenSequenceDiagrams Core」に変更
- 説明文を更新
- バッジURLの更新（もしあれば）
- インストール手順の更新

#### その他
- ソースコード内のコメントで `guideline` に言及している箇所
- ドキュメント内の参照
- CI/CD設定ファイル（GitHub Actions等）

## 注意事項

- 旧URL `guide-inc-org/guideline` は自動でリダイレクトされる
- crates.io に公開済みの場合は、新しいクレート名で再公開が必要
- npm に公開済みの場合は、新しいパッケージ名で再公開が必要

## 確認コマンド

リネーム漏れがないか確認：

```bash
# guideline という文字列を検索
grep -r "guideline" --include="*.rs" --include="*.toml" --include="*.json" --include="*.md" .
```

## 完了条件

- [ ] GitHub リポジトリ名が `opensequencediagrams-core` に変更されている
- [ ] Cargo.toml のパッケージ名が `osd-core` になっている
- [ ] README.md が更新されている
- [ ] `grep -r "guideline"` で不要な参照が残っていない
