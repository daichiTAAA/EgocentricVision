# EgocentricVision

An Egocentric Vision system that provides video stream recording and management capabilities using Raspberry Pi and modern web technologies.

## Architecture

- **Record Service**: Rust-based backend service for RTSP/WebRTC stream recording
- **Frontend**: Modern web interface for stream management
- **Mediamtx**: RTSP/WebRTC media server

## Quick Start

### Using Docker Compose

1. Clone the repository:
```bash
git clone https://github.com/daichiTAAA/EgocentricVision.git
cd EgocentricVision
```

2. Start the services:
```bash
docker compose up --build -d
```
終了
```bash
# ボリュームも削除する場合
docker compose down -v
```

```bash
# 停止して再起動する場合
docker compose down -v && docker compose up --build -d
```

This will start:
- PostgreSQL database on port 5432
- Record service on port 3000

3. Test the setup:
```bash
./test-docker-setup.sh
```

For detailed instructions, see [Record Service README](src/record/README.md).

---

# Raspberry Pi Zero 2 WでのEgocentric Visionシステムのセットアップ手順
## Raspberry Pi OSのインストール
Raspberry Pi Zero 2 WにRaspberry Pi OS(32ビット版)をインストールします。

## IPアドレスを固定
Raspberry Pi Zero 2 WのIPアドレスを固定することで、ネットワーク上でのアクセスを安定させます。以下は、Raspberry Pi OSでのIPアドレス固定の手順です。

Raspberry Pi OSでは、`/etc/dhcpcd.conf`ファイルを編集してIPアドレスを固定します。

### 設定手順

1. ターミナルで以下のコマンドを実行して設定ファイルを開きます：

   ```sh
   sudo nano /etc/dhcpcd.conf
   ```

2. ファイルの末尾に以下のように追記します（例: 有線LANの場合は`eth0`、無線LANの場合は`wlan0`）：

```
interface wlan0
static ip_address=192.168.0.100/24
static routers=192.168.0.1
static domain_name_servers=8.8.8.8 8.8.4.4
```
   - `interface`：設定したいインターフェース名（`ip a`コマンドで確認可能）
   - `static ip_address`：割り当てたい固定IPアドレス（例: 192.168.0.100/24）
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

## Mediamtxをインストールする

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

---

## ストリームの受信方法

### RTSPストリームの受信

Mediamtxのデフォルト設定では、Raspberry Pi Cameraのストリームは以下のURLで受信できます。

- RTSP URL例:
  
  ```
  rtsp://<RaspberryPiのIPアドレス>:8554/cam/
  ```
  
  - VLCやffmpegなどのRTSPクライアントで再生可能です。

### WebRTCストリームの受信（ブラウザ）

MediamtxはWebRTCの簡易プレイヤーを内蔵しています。

- WebRTC視聴用URL:
  
  ```
  http://<RaspberryPiのIPアドレス>:8889/cam/
  ```
  
  - ブラウザで上記URLにアクセスし、ストリーム名（例: `/`）を入力して再生できます。
  - 例: `http://<RaspberryPiのIPアドレス>:8889/` → ページ上部のフォームに `/` を入力し「play」ボタンを押す

#### 注意
- `<RaspberryPiのIPアドレス>`は実際のIPアドレスに置き換えてください。
- ファイアウォールやネットワーク設定により外部からアクセスできない場合があります。