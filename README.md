# EgocentricVision

# IPアドレスを固定する
## Raspberry Pi OSでIPアドレスを固定する方法

Raspberry Pi OSでは、`/etc/dhcpcd.conf`ファイルを編集してIPアドレスを固定します。

### 設定手順

1. ターミナルで以下のコマンドを実行して設定ファイルを開きます：

   ```sh
   sudo nano /etc/dhcpcd.conf
   ```

2. ファイルの末尾に以下のように追記します（例: 有線LANの場合は`eth0`、無線LANの場合は`wlan0`）：

```
interface wlan0
static ip_address=192.168.1.100/24
static routers=192.168.0.1
static domain_name_servers=8.8.8.8 8.8.4.4
```
   - `interface`：設定したいインターフェース名（`ip a`コマンドで確認可能）
   - `static ip_address`：割り当てたい固定IPアドレス（例: 192.168.1.100/24）
   - `static routers`：ルーター（ゲートウェイ）アドレス
   - `static domain_name_servers`：DNSサーバーアドレス

3. 保存してエディタを終了し、Raspberry Piを再起動します：

   ```sh
   sudo reboot
   ```

#### ルーター（ゲートウェイ）アドレスの確認方法（Raspberry Pi OS）

ターミナルで以下のコマンドを実行してください：

```sh
ip route | grep default
```

`default via`の後ろに表示されるアドレスがルーター（ゲートウェイ）です。

# Mediamtxをインストールする

Raspberry Pi Zero 2 W の Raspberry Pi OS で Mediamtx を最新版にインストールし、systemd で自動起動する手順を記載します。

1. スクリプトに実行権限を付与します。

   ```sh
   chmod +x ./scripts/install_mediamtx.sh
   chmod +x ./scripts/setup_mediamtx_service.sh
   ```

2. Mediamtx をインストールします。

   ```sh
   ./scripts/install_mediamtx.sh
   ```

3. systemdサービスを設定・有効化します。

   ```sh
   ./scripts/setup_mediamtx_service.sh
   ```

4. Mediamtx のステータスを確認します。

   ```sh
   sudo systemctl status mediamtx
   ```

5. Mediamtx のログを確認します。

   ```sh
   sudo journalctl -u mediamtx -f
   ```

6. Mediamtx の設定ファイルを編集します（例: /home/pi/mediamtx.yml など）。

   ```sh
   nano /home/$USER/EgocentricVision/mediamtx.yml
   ```

   ※ systemdサービスのWorkingDirectoryや-cオプションで指定したパスに合わせて編集してください。

7. 設定ファイルを編集したら、Mediamtx を再起動します。

   ```sh
   sudo systemctl restart mediamtx
   ```

8. Mediamtx の自動起動を有効/無効にする場合は、以下のコマンドを実行します。

   ```sh
   sudo systemctl enable mediamtx
   sudo systemctl disable mediamtx
   ```