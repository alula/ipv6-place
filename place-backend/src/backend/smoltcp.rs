use super::NetworkBackend;
use crate::{backend::PixelRequest, place::SharedImageHandle, settings::Settings, PResult};
use smoltcp::{
    iface::{Config, Interface, SocketSet},
    phy::{self, ChecksumCapabilities, Medium, TunTapInterface},
    socket::raw,
    wire::{
        Icmpv6Packet, Icmpv6Repr, IpAddress, IpCidr, IpProtocol, IpVersion, Ipv6Address,
        Ipv6Packet, Ipv6Repr, UdpPacket, UdpRepr,
    },
};
use std::os::fd::AsRawFd;
use tokio::task::JoinHandle;

pub struct SmoltcpNetworkBackend {
    image: SharedImageHandle,
    device: TunTapInterface,
    interface: Interface,
    recv_buffer_size: usize,
}

fn or_addr(addr: Ipv6Address, mask: Ipv6Address) -> Ipv6Address {
    let mut bytes = addr.0;
    let mask_bytes = mask.0;

    for i in 0..16 {
        bytes[i] = bytes[i] | mask_bytes[i];
    }

    Ipv6Address::from_bytes(&bytes)
}

impl SmoltcpNetworkBackend {
    pub fn new(settings: &Settings, image: SharedImageHandle) -> PResult<Box<dyn NetworkBackend>> {
        let mut config = Config::new(smoltcp::wire::HardwareAddress::Ip);
        config.random_seed = rand::random();
        // config.hardware_addr = Some(EthernetAddress([0x02, 0x00, 0x00, 0x00, 0x00, 0x01]).into());

        let mut device = TunTapInterface::new(&settings.backend.smoltcp.tun_iface, Medium::Ip)?;

        let prefix: Ipv6Address = settings.backend.prefix48.into();

        let prefix_s1 = or_addr(prefix, Ipv6Address::new(0, 0, 0, 0x1000, 0, 0, 0, 0));
        let prefix_s2 = or_addr(prefix, Ipv6Address::new(0, 0, 0, 0x2000, 0, 0, 0, 0));

        let mut interface = Interface::new(config, &mut device);
        interface.update_ip_addrs(|addrs| {
            // Actually we register two /52 prefixes, for 1 and 2 pixel sizes.
            let _ = addrs.push(IpCidr::new(IpAddress::Ipv6(prefix_s1), 52));
            let _ = addrs.push(IpCidr::new(IpAddress::Ipv6(prefix_s2), 52));
        });

        Ok(Box::new(Self {
            image,
            device,
            interface,
            recv_buffer_size: settings.backend.smoltcp.recv_buffer_size,
        }))
    }
}

// SAFETY: We only ever access inner fields from a single thread.
unsafe impl Send for SmoltcpNetworkBackend {}
unsafe impl Sync for SmoltcpNetworkBackend {}

impl NetworkBackend for SmoltcpNetworkBackend {
    fn start(mut self: Box<Self>) -> JoinHandle<PResult<()>> {
        tokio::task::spawn_blocking(move || {
            let dimensions = self.image.get_dimensions_blocking();

            let mut sockets = SocketSet::new(vec![]);

            let icmp_rx_buffer = raw::PacketBuffer::new(
                vec![raw::PacketMetadata::EMPTY; self.recv_buffer_size],
                vec![0; self.recv_buffer_size * 512],
            );
            let icmp_tx_buffer =
                raw::PacketBuffer::new(vec![raw::PacketMetadata::EMPTY], vec![0; 256]);
            let icmp_socket = raw::Socket::new(
                IpVersion::Ipv6,
                IpProtocol::Icmpv6,
                icmp_rx_buffer,
                icmp_tx_buffer,
            );

            let udp_rx_buffer = raw::PacketBuffer::new(
                vec![raw::PacketMetadata::EMPTY; self.recv_buffer_size],
                vec![0; self.recv_buffer_size * 512],
            );
            let udp_tx_buffer =
                raw::PacketBuffer::new(vec![raw::PacketMetadata::EMPTY], vec![0; 256]);
            let udp_socket = raw::Socket::new(
                IpVersion::Ipv6,
                IpProtocol::Udp,
                udp_rx_buffer,
                udp_tx_buffer,
            );

            let icmp_handle = sockets.add(icmp_socket);
            let udp_handle = sockets.add(udp_socket);
            let fd = self.device.as_raw_fd();
            let ignored_caps = ChecksumCapabilities::ignored();

            loop {
                let timestamp = smoltcp::time::Instant::now();
                self.interface
                    .poll(timestamp, &mut self.device, &mut sockets);
                {
                    let icmp_socket = sockets.get_mut::<raw::Socket>(icmp_handle);

                    while icmp_socket.can_recv() {
                        let buffer = match icmp_socket.recv() {
                            Ok(buffer) => buffer,
                            Err(_) => continue,
                        };
                        let packet = match Ipv6Packet::new_checked(buffer) {
                            Ok(packet) => packet,
                            Err(_) => continue,
                        };
                        let ipv6_parsed = match Ipv6Repr::parse(&packet) {
                            Ok(repr) => repr,
                            Err(_) => continue,
                        };

                        log::trace!("Received packet {:?}", ipv6_parsed);

                        let icmp_packet = match Icmpv6Packet::new_checked(packet.payload()) {
                            Ok(packet) => packet,
                            Err(_) => continue,
                        };

                        let icmp_parsed = match Icmpv6Repr::parse(
                            &ipv6_parsed.src_addr.into_address(),
                            &ipv6_parsed.dst_addr.into_address(),
                            &icmp_packet,
                            &ignored_caps,
                        ) {
                            Ok(repr) => repr,
                            Err(_) => continue,
                        };

                        match icmp_parsed {
                            Icmpv6Repr::EchoRequest { .. } => {
                                let req = PixelRequest::from_ipv6(&ipv6_parsed.dst_addr.into());
                                let (x, y) = req.pos;
                                self.image
                                    .put_blocking(x as _, y as _, req.color, req.size == 2);
                            }
                            _ => {}
                        }
                    }
                }

                {
                    let udp_socket = sockets.get_mut::<raw::Socket>(udp_handle);

                    while udp_socket.can_recv() {
                        let buffer = match udp_socket.recv() {
                            Ok(buffer) => buffer,
                            Err(_) => continue,
                        };
                        let packet = match Ipv6Packet::new_checked(buffer) {
                            Ok(packet) => packet,
                            Err(_) => continue,
                        };
                        let ipv6_parsed = match Ipv6Repr::parse(&packet) {
                            Ok(repr) => repr,
                            Err(_) => continue,
                        };

                        log::trace!("Received packet {:?}", ipv6_parsed);

                        let udp_packet = match UdpPacket::new_checked(packet.payload()) {
                            Ok(packet) => packet,
                            Err(_) => continue,
                        };

                        let udp_parsed = match UdpRepr::parse(
                            &udp_packet,
                            &ipv6_parsed.src_addr.into_address(),
                            &ipv6_parsed.dst_addr.into_address(),
                            &ignored_caps,
                        ) {
                            Ok(repr) => repr,
                            Err(_) => continue,
                        };

                        if udp_parsed.dst_port == 7 {
                            let req = PixelRequest::from_ipv6(&ipv6_parsed.dst_addr.into());
                            let (x, y) = req.pos;
                            self.image
                                .put_blocking(x as _, y as _, req.color, req.size == 2);
                        }
                    }
                }

                phy::wait(fd, self.interface.poll_delay(timestamp, &sockets))?;
            }
        })
    }
}
