#!/bin/bash

TUN_NAME=tun0

# Clean up any previous state
sudo ip link set $TUN_NAME down
sudo ip tuntap del dev $TUN_NAME mode tun

sudo ip tuntap add name $TUN_NAME mode tun user $USER
sudo ip link set $TUN_NAME up
sudo ip -6 route add fdaa:0:0:1000::/52 dev $TUN_NAME
sudo ip -6 route add fdaa:0:0:2000::/52 dev $TUN_NAME