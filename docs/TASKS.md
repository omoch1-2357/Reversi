# 実装タスク一覧

本ドキュメントは [REQUIREMENTS.md](./REQUIREMENTS.md) および [DESIGN.md](./DESIGN.md) に基づく実装タスク一覧である。
進捗管理用に `[ ]`（未着手）/ `[/]`（作業中）/ `[x]`（完了）で状態を記録する。

---

## Phase 1: Rust ゲームエンジンコア

### 1-1. プロジェクト初期設定

- [x] `rust/` に Cargo ライブラリプロジェクトを作成
- [x] `Cargo.toml` に依存関係を追加（`wasm-bindgen`, `serde`, `serde-wasm-bindgen`, `once_cell`）
- [x] `wasm-pack build` でビルドが通ることを確認

### 1-2. `board.rs` — ビットボード盤面ロジック

- [x] `Board` 構造体（`black: u64`, `white: u64`）
- [x] 初期盤面生成 `new()`
- [x] 合法手ビットマスク計算 `legal_moves(is_black)`
- [x] 8方向反転＋着手 `place(pos, is_black)` → 反転ビットマスク返却
- [x] 石数カウント `count()`, 空きマス数 `empty_count()`
- [x] 1次元配列変換 `to_array()` — `[u8; 64]`
- [x] 単体テスト: T-01（初期合法手4マス）、反転処理の正当性、石数カウント

### 1-3. `types.rs` — 公開型定義

- [ ] `Position`, `GameState`, `GameResult` を `#[derive(Serialize)]` で定義
- [ ] `GameState.flipped` / `is_pass` の契約コメント付記

### 1-4. `game.rs` — ゲーム進行管理

- [ ] `GameInstance` 構造体（`current_player: u8`, `is_pass`, `flipped`, `is_game_over` 等）
- [ ] `new(level, evaluator)` — 初期化
- [ ] `place(row, col)` — プレイヤー着手（手番検証含む）
- [ ] `has_legal_moves_for_current()`, `pass()`, `end_game()`
- [ ] `do_ai_move()` — 探索＋着手のみ（F-05自動パスは `lib.rs::ai_move()` 側の責務）
- [ ] `to_game_state()`, `to_game_result()` 変換メソッド
- [ ] 単体テスト: 初期配置、T-02（非合法手エラー）、T-03（パス発生）、T-04（両者パス終局）、T-05（満盤終局）

---

## Phase 2: Python 学習パイプライン

### 2-1. プロジェクト初期設定

- [ ] `python/` ディレクトリ作成
- [ ] `requirements.txt`（`numpy`, `pytest`）

### 2-2. `board.py` — 盤面ロジック

- [ ] Rust版と同一仕様のビットボード（`legal_moves`, `place`, `count`, `empty_count`）
- [ ] `to_array(is_black)`, `copy()`
- [ ] pytest 単体テスト

### 2-3. `ntuple.py` — N-Tuple Network

- [ ] タプルパターン定義（`TUPLE_PATTERNS`）
- [ ] 8対称変換 `_symmetries()`
- [ ] 評価 `evaluate(board, is_black)` — 手番視点スコア
- [ ] 更新 `update(board, is_black, delta)` — 学習率適用済み更新量
- [ ] パターンインデックス計算 `_pattern_index()`
- [ ] pytest 単体テスト

### 2-4. `td_lambda.py` — TD-Lambda 学習

- [ ] `TDLambdaTrainer`（α, λ, ε パラメータ）
- [ ] ε-greedy 自己対戦ループ `_play_one_game()`
- [ ] TD-Lambda 遡及更新 `_update_weights()`
- [ ] pytest 単体テスト: 更新方向の正当性、終局報酬反映、固定seed再現性

### 2-5. `export_model.py` — モデルエクスポート

- [ ] `weights.bin` フォーマット（20bytes ヘッダ + タプル定義 + 重み配列）
- [ ] CRC32 チェックサム計算＋ヘッダ付与
- [ ] pytest 単体テスト: magic/version/num_tuples/CRC32/データ長整合の検証

### 2-6. `train.py` — メインスクリプト

- [ ] argparse CLI（`--games`, `--alpha`, `--lambda`, `--epsilon`, `--output`）
- [ ] パイプライン統合（学習→エクスポート）
- [ ] 動作確認（固定seed・1000局学習で `weights.bin` 生成成功、フォーマット・CRC・ロード可能性を検証）

---

## Phase 3: Rust AI推論 ＆ WASM統合

### 3-1. `ai/ntuple.rs` — N-Tuple 評価関数（推論用）

- [ ] 仮 `weights.bin` を Python で生成し `rust/src/ai/` に配置
- [ ] `NTupleEvaluator` 構造体
- [ ] `from_bytes()` — デシリアライズ（マジック・バージョン・CRC32 検証）
- [ ] `evaluate(board, is_black)` — 手番視点、8対称変換
- [ ] 単体テスト: デシリアライズ成功/失敗、評価値計算、T-09（不正 weights.bin）

### 3-2. `ai/search.rs` — 探索アルゴリズム

- [ ] `Searcher` 構造体（タイムアウト・開始時刻管理）
- [ ] 反復深化フロー `search()` — 合法手1つなら即返し
- [ ] `negaalpha()` — `root_depth` 引数付き（深度1タイムアウト抑制）
- [ ] 合法手の評価値順ソート＋安定タイブレーク（インデックス昇順）— 枝刈り効率向上（DESIGN.md §2.5, REQUIREMENTS.md §5.3）
- [ ] タイブレーク: `score > best_score || (score == best_score && mv < best_move)`
- [ ] 終盤完全読み `should_exact_solve()` / `exact_solve()` — deadline 管理付き
- [ ] 単体テスト: 探索結果の正当性、タイムアウト動作

### 3-3. `lib.rs` — WASM 公開 API

- [ ] `GAME` シングルトン（`Lazy<Mutex<Option<GameInstance>>>`）
- [ ] `init_game(level)` — `include_bytes!`, CRC32検証, `serde_wasm_bindgen::to_value`
- [ ] `get_legal_moves()` — `Result<JsValue, JsValue>`
- [ ] `place_stone(row, col)` — 手番検証（`current_player != 1` → Err）
- [ ] `ai_move()` — 手番検証 + 1手AI着手 + プレイヤー自動パス（F-05）
- [ ] `get_result()` — 終了検証付き
- [ ] `wasm-pack build` でビルド通過を確認

### 3-4. Rust 統合テスト

- [ ] `tests/` にプレイスルーテスト（init → place → ai_move → get_result）
- [ ] `search.rs` 単体/結合での決定性検証（Rust内部層。WASM境界の決定性は 3-5 の T-08/T-13 で担保）

### 3-5. WASM API 統合テスト実装

- [ ] T-06: 不正レベル値テスト — `init_game(0)`, `init_game(7)` でエラーを返すことを検証
- [ ] T-07: 全レベルAI着手テスト — Level 1〜6 で正常に着手が返ることを検証
- [ ] T-08: AI決定性テスト — 同一局面・同一レベルで同じ手が返ることを検証
- [ ] T-10: `init_game` 再呼び出しテスト — 2回目の `init_game()` で状態がリセットされることを検証
- [ ] T-11: AIパス局面テスト — AI合法手0の局面で自動パスしプレイヤー手番に戻ることを検証
- [ ] T-12: 完全読みタイムアウトテスト — 5秒超過時にフォールバック手を返すことを検証
- [ ] T-13: AI決定性（100回）テスト — 同一局面・同一レベルで100回連続同じ手を返すことを検証
- [ ] 未終局 `get_result()` エラーテスト — ゲーム未終了時に `get_result()` がエラーを返すことを検証

---

## Phase 4: Web フロントエンド

### 4-1. プロジェクト初期設定

- [ ] `web/` に Vite + React + TypeScript プロジェクトを生成
- [ ] `vite-plugin-wasm` 導入、`vite.config.ts` 設定（`target: ['chrome80', ...]`, `worker.format: 'es'`, `base: '/Reversi/'`）
- [ ] WASM生成物を `web/src/wasm` からインポートできることを確認

### 4-2. WASM TypeScript ラッパー

- [ ] `wasm/index.ts` — 型アサーション付きラッパー関数

### 4-3. Web Worker

- [ ] `wasm.worker.ts` — WASM 初期化 + メッセージハンドリング
- [ ] `place_stone` 後の AI 手番ループ（`while current_player===2`）+ `ai_step` 通知

### 4-4. React コンポーネント

- [ ] `LevelSelect.tsx` — レベル(1〜6)選択画面
- [ ] `Board.tsx` + `Cell.tsx` — 盤面描画、着手可能マスハイライト、反転アニメーション
- [ ] `GameInfo.tsx` — 石数・手番・思考中インジケータ
- [ ] `ResultModal.tsx` — 勝敗表示 + リスタートボタン

### 4-5. 状態管理フック

- [ ] `useGame.ts` — Worker ライフサイクル管理 + React State 連携

### 4-6. レスポンシブ対応

- [ ] CSS: ブレークポイント 768px、盤面最小幅 352px（44px×8マス）
- [ ] モバイル: 縦積みレイアウト、タップ領域 44px×44px 保証

### 4-7. 画面統合

- [ ] `App.tsx` — 画面遷移（タイトル ↔ ゲーム ↔ 結果）
- [ ] `init_game` エラー時のUI対応（モデル読み込み失敗メッセージ表示・開始ボタン無効化・再試行導線）
- [ ] `npm run dev` でローカル動作確認

---

## Phase 5: CI/CD ＆ パフォーマンス検証

### 5-1. CI ワークフロー（`.github/workflows/test.yml`）

- [ ] rust-test: `cargo test`（T-01〜T-05, T-09）
- [ ] python-test: `pytest`
- [ ] wasm-build: `wasm-pack build`
- [ ] web-build: `npm ci && npm run build`
- [ ] performance-test: 要件3.1準拠（seed=42, 100局面, 手数20〜40, ウォームアップ, 各レベル p95 < 3秒）
- [ ] wasm-integration-test: T-06, T-07, T-08, T-10, T-11, T-12, T-13 + 未終局 `get_result()` エラー検証
- [ ] wasm-size-check: gzip 後 ≤ 10MB
- [ ] e2e-test: Playwright（PC 1280×720 + モバイル 375×667）
- [ ] ブラウザ互換テスト（NF-02）: Playwright ブラウザマトリクス（`chromium` / `firefox` / `webkit`）で主要フローを検証（起動、着手、AI応答、終局表示）

### 5-2. CD ワークフロー（`.github/workflows/deploy.yml`）

- [ ] ビルド → size-gate → GitHub Pages デプロイ

### 5-3. 本番リリース

- [ ] Python フル学習実行 → 本番用 `weights.bin` 生成
- [ ] `rust/src/ai/weights.bin` を差し替えてコミット
- [ ] `main` へマージ → 自動デプロイ → 動作確認
