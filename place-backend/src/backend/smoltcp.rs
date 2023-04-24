use super::NetworkBackend;
use crate::{place::SharedImageHandle, settings::Settings, PResult};
use smoltcp::{
    iface::{Config, Interface, SocketSet, Route},
    phy::{self, ChecksumCapabilities, Medium, TunTapInterface},
    socket::raw,
    wire::{
        EthernetAddress, Icmpv6Packet, Icmpv6Repr, IpAddress, IpCidr, IpProtocol, IpVersion,
        Ipv6Address, Ipv6Packet, Ipv6Repr, NdiscRepr,
    },
};
use std::os::fd::AsRawFd;
use tokio::task::JoinHandle;

pub struct SmoltcpNetworkBackend {
    image: SharedImageHandle,
    device: TunTapInterface,
    interface: Interface,
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
        let mut config = Config::new();
        config.random_seed = rand::random();
        config.hardware_addr = Some(EthernetAddress([0x02, 0x00, 0x00, 0x00, 0x00, 0x01]).into());

        let mut device =
            TunTapInterface::new(&settings.backend.smoltcp.tap_iface, Medium::Ethernet)?;

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

            let raw_rx_buffer =
                raw::PacketBuffer::new(vec![raw::PacketMetadata::EMPTY], vec![0; 256]);
            let raw_tx_buffer =
                raw::PacketBuffer::new(vec![raw::PacketMetadata::EMPTY], vec![0; 256]);
            let raw_socket = raw::Socket::new(
                IpVersion::Ipv6,
                IpProtocol::Icmpv6,
                raw_rx_buffer,
                raw_tx_buffer,
            );

            let raw_handle = sockets.add(raw_socket);
            let fd = self.device.as_raw_fd();

            loop {
                let timestamp = smoltcp::time::Instant::now();
                self.interface
                    .poll(timestamp, &mut self.device, &mut sockets);

                let raw_socket = sockets.get_mut::<raw::Socket>(raw_handle);

                while raw_socket.can_recv() {
                    let buffer = match raw_socket.recv() {
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

                    log::debug!("Received packet {:?}", ipv6_parsed);

                    let icmp_packet = match Icmpv6Packet::new_checked(packet.payload()) {
                        Ok(packet) => packet,
                        Err(_) => continue,
                    };

                    let icmp_parsed = match Icmpv6Repr::parse(
                        &ipv6_parsed.src_addr.into_address(),
                        &ipv6_parsed.dst_addr.into_address(),
                        &icmp_packet,
                        &ChecksumCapabilities::default(),
                    ) {
                        Ok(repr) => repr,
                        Err(_) => continue,
                    };

                    log::debug!("Received ICMP packet {:?}", icmp_parsed);

                    match icmp_parsed {
                        Icmpv6Repr::EchoRequest { ident, seq_no, data } => {
                            
                        }
                        Icmpv6Repr::Ndisc(NdiscRepr::RouterSolicit{lladdr}) => {
                            
                            
                        }
                        _ => {}
                    }
                }

                phy::wait(fd, self.interface.poll_delay(timestamp, &sockets))?;
            }
        })
    }
}
