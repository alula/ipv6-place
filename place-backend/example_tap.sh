#!/bin/bash

TAP_NAME=tap0

sudo ip tuntap add name $TAP_NAME mode tap user $USER
sudo ip link set $TAP_NAME up
sudo ip -6 route add fdaa:0:0:1000::/52 dev $TAP_NAME
sudo ip -6 route add fdaa:0:0:2000::/52 dev $TAP_NAME