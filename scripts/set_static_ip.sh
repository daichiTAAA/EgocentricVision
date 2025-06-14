#!/bin/zsh
# set_static_ip.sh
# Usage: sudo ./set_static_ip.sh <interface> <ip_address> <subnet_mask> <router>
# Example: sudo ./set_static_ip.sh "Wi-Fi" 192.168.1.100 255.255.255.0 192.168.1.1

if [ "$#" -ne 4 ]; then
  echo "Usage: sudo $0 <interface> <ip_address> <subnet_mask> <router>"
  exit 1
fi

INTERFACE="$1"
IP="$2"
MASK="$3"
ROUTER="$4"

echo "Setting static IP for $INTERFACE..."
networksetup -setmanual "$INTERFACE" "$IP" "$MASK" "$ROUTER"
if [ $? -eq 0 ]; then
  echo "Successfully set static IP for $INTERFACE."
else
  echo "Failed to set static IP."
  exit 2
fi
