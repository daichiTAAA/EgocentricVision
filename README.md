# EgocentricVision

# IPアドレスを固定する

## IPアドレスを固定するスクリプト

macOSでネットワークインターフェースのIPアドレスを固定するには、`set_static_ip.sh`スクリプトを利用できます。

### 使い方

1. ターミナルで本リポジトリのディレクトリに移動します。
2. スクリプトに実行権限を付与します：

   ```zsh
   chmod +x ./scripts/set_static_ip.sh
   ```

3. 以下のコマンドでIPアドレスを固定します（管理者権限が必要です）：

   ```zsh
   sudo ./scripts/set_static_ip.sh "<インターフェース名>" <IPアドレス> <サブネットマスク> <ルーター>
   ```
   例：
   ```zsh
   sudo ./scripts/set_static_ip.sh "Wi-Fi" 192.168.1.100 255.255.255.0 192.168.1.1
   ```

- `<インターフェース名>` には `networksetup -listallnetworkservices` で確認できる名称を指定してください。
- 変更後、ネットワーク接続が一時的に切断される場合があります。

#### ルーター（ゲートウェイ）アドレスの確認方法

ターミナルで以下のコマンドを実行してください：

```zsh
route -n get default | grep 'gateway'
```

または、ネットワーク設定画面でも確認できます。

# Mediamtxをインストールする

Raspberry Pi Zero 2 W の Raspberry Pi OS で Mediamtx を最新版にインストールし、systemd で自動起動する手順を記載します。

1. スクリプトを実行して、Mediamtx をインストールします。

   ```sh
   ./scripts/install_mediamtx.sh
   ```
2. インストールが完了したら、Mediamtx を起動します。

   ```sh
   sudo systemctl start mediamtx
   ```
3. Mediamtx のステータスを確認します。

   ```sh
   sudo systemctl status mediamtx
   ```
4. Mediamtx のログを確認します。

   ```sh
   sudo journalctl -u mediamtx -f
   ```
5. Mediamtx の設定ファイルを編集します。

   ```sh
   sudo nano /etc/mediamtx/mediamtx.conf
   ```
6. 設定ファイルを編集したら、Mediamtx を再起動します。

   ```sh
   sudo systemctl restart mediamtx
   ```
# 7. Mediamtx の自動起動を有効にします。

   ```sh
   sudo systemctl enable mediamtx
   ```
# 8. Mediamtx の自動起動を無効にする場合は、以下のコマンドを実行します。

   ```sh
   sudo systemctl disable mediamtx
   ```