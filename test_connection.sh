#!/bin/bash
set -e

echo "フロントエンドからrecord-serviceへの接続テストを開始します..."

# フロントエンドコンテナ内でcurlをインストール
echo "フロントエンドコンテナにcurlをインストール中..."
docker compose exec frontend apk add --no-cache curl

# record-serviceのヘルスチェック
echo "record-serviceのヘルスチェックを実行中..."
for i in {1..10}; do
  if docker compose exec frontend curl -s http://record-service:3000/api/v1/streams/status > /dev/null; then
    echo "✅ record-serviceへの接続成功"
    break
  fi
  if [ $i -eq 10 ]; then
    echo "❌ record-serviceへの接続に失敗しました"
    exit 1
  fi
  echo "待機中... ($i/10)"
  sleep 2
done

# フロントエンドの環境変数チェック
echo "フロントエンドの環境変数をチェック中..."
API_URL=$(docker compose exec frontend env | grep VITE_API_BASE_URL | cut -d '=' -f2)
if [ "$API_URL" = "http://record-service:3000" ]; then
  echo "✅ 環境変数 VITE_API_BASE_URL が正しく設定されています"
else
  echo "❌ 環境変数 VITE_API_BASE_URL が不正です: $API_URL"
  exit 1
fi

echo "✅ すべてのテストが成功しました" 