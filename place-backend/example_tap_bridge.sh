#!/bin/bash

TAP_NAME=tap0
ETH_NAME=ens18
BR_NAME=br0

# Clean up any previous state
sudo ip link set $BR_NAME down
sudo ip link set $TAP_NAME down

sudo brctl delbr $BR_NAME
sudo ip tuntap del dev $TAP_NAME mode tap

# Set things up
sudo modprobe bridge
sudo modprobe br_netfilter

sudo sysctl -w net.ipv6.conf.all.forwarding=1
sudo sysctl -w net.bridge.bridge-nf-call-arptables=0
sudo sysctl -w net.bridge.bridge-nf-call-ip6tables=0
sudo sysctl -w net.bridge.bridge-nf-call-iptables=0

sudo ip tuntap add name $TAP_NAME mode tap user $USER
sudo brctl addbr $BR_NAME
sudo brctl addif $BR_NAME $TAP_NAME
sudo brctl addif $BR_NAME $ETH_NAME
sudo ip link set $TAP_NAME up
sudo ip link set $BR_NAME up
sudo ip -6 route add 2602:fa9b:202:1000::/52 dev $TAP_NAME
sudo ip -6 route add 2602:fa9b:202:2000::/52 dev $TAP_NAME