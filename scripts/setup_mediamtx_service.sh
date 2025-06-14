#!/bin/bash
# Mediamtx systemdサービスの設定・有効化スクリプト

set -e

USER=$(whoami)
GROUP=$(id -gn)
HOME_DIR=$HOME

# 1. systemdサービスファイル作成
echo "[Unit]
Description=Mediamtx Service
After=network.target

[Service]
ExecStart=/usr/local/bin/mediamtx /home/${USER}/EgocentricVision/config/mediamtx.yml
Restart=always
User=${USER}
Group=${GROUP}
Environment=HOME=${HOME_DIR}
WorkingDirectory=${HOME_DIR}

[Install]
WantedBy=multi-user.target
" | sudo tee /etc/systemd/system/mediamtx.service > /dev/null

# 2. systemdへ登録・自動起動
sudo systemctl daemon-reload
sudo systemctl enable mediamtx
sudo systemctl start mediamtx

echo "Mediamtx systemdサービスの設定と自動起動が完了しました。"
