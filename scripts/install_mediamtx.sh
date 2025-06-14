#!/bin/bash
# Mediamtx 最新版をRaspberry Pi Zero 2 W (Raspberry Pi OS)にインストールし、systemdサービスとして登録するスクリプト

set -e

# 1. ダウンロード
wget -O mediamtx_v1.12.3_linux_armv6.tar.gz "https://github.com/bluenviron/mediamtx/releases/download/v1.12.3/mediamtx_v1.12.3_linux_armv6.tar.gz"

tar -xzf mediamtx_v1.12.3_linux_armv6.tar.gz

# 2. バイナリの移動と権限付与
sudo mv mediamtx /usr/local/bin/
sudo chmod +x /usr/local/bin/mediamtx

# 3. systemdサービスファイル作成
echo "[Unit]
Description=Mediamtx Service
After=network.target

[Service]
ExecStart=/usr/local/bin/mediamtx
Restart=always
User=pi
Group=pi
Environment=HOME=/home/pi
WorkingDirectory=/home/pi

[Install]
WantedBy=multi-user.target
" | sudo tee /etc/systemd/system/mediamtx.service > /dev/null

# 4. systemdへ登録・自動起動
sudo systemctl daemon-reload
sudo systemctl enable mediamtx
sudo systemctl start mediamtx

echo "Mediamtxのインストールと自動起動設定が完了しました。"
