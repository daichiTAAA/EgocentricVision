#!/bin/bash
# MediamtxをRaspberry Pi Zero 2 W (Raspberry Pi OS 32bit)にインストールするスクリプト

set -e

# 1. ダウンロード
wget -O mediamtx_v1.12.3_linux_armv6.tar.gz "https://github.com/bluenviron/mediamtx/releases/download/v1.12.3/mediamtx_v1.12.3_linux_armv6.tar.gz"

tar -xzf mediamtx_v1.12.3_linux_armv6.tar.gz

# 2. バイナリの移動と権限付与
sudo mv mediamtx /usr/local/bin/
sudo chmod +x /usr/local/bin/mediamtx

echo "Mediamtxのインストールが完了しました。"
