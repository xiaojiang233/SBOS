//! Kernel network interface and protocol stack.
//!
//! smoltcp handles Ethernet, ARP, IPv4, ICMP, UDP, TCP, DNS, and DHCP protocol
//! state. The SBOS device adapter owns hardware framing and DMA; this module
//! owns the L3/L4 interface configuration and periodic protocol polling.

use crate::drivers::e1000;
use crate::sync::SpinLock;
use alloc::vec;
use alloc::vec::Vec;
use smoltcp::iface::{Config, Interface, SocketHandle, SocketSet};
use smoltcp::phy::{Device, DeviceCapabilities, Medium, RxToken, TxToken};
use smoltcp::socket::udp::{PacketBuffer, PacketMetadata, Socket as UdpSocket};
use smoltcp::time::Instant;
use smoltcp::wire::{EthernetAddress, HardwareAddress, IpAddress, IpCidr, IpEndpoint, IpListenEndpoint, Ipv4Address};
use core::sync::atomic::{AtomicU32, Ordering};

const MAX_FRAME_SIZE: usize = 2048;
const MAX_UDP_SOCKETS: usize = 32;
const UDP_BUFFER_BYTES: usize = 4096;
const UDP_BUFFER_PACKETS: usize = 8;
const EPHEMERAL_FIRST: u16 = 49152;
const EPHEMERAL_LAST: u16 = 65535;

struct E1000Device;
struct ReceivedFrame {
    bytes: [u8; MAX_FRAME_SIZE],
    length: usize,
}
struct TransmitToken;

impl RxToken for ReceivedFrame {
    fn consume<R, F>(self, f: F) -> R
    where
        F: FnOnce(&[u8]) -> R,
    {
        f(&self.bytes[..self.length])
    }
}

impl TxToken for TransmitToken {
    fn consume<R, F>(self, length: usize, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        assert!(length <= MAX_FRAME_SIZE, "network stack exceeded E1000 frame size");
        let mut bytes = [0u8; MAX_FRAME_SIZE];
        let result = f(&mut bytes[..length]);
        if e1000::transmit(&bytes[..length]).is_err() {
            TX_ERRORS.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
        }
        result
    }
}

impl Device for E1000Device {
    type RxToken<'a> = ReceivedFrame;
    type TxToken<'a> = TransmitToken;

    fn receive(&mut self, _timestamp: Instant) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> {
        let mut frame = ReceivedFrame { bytes: [0; MAX_FRAME_SIZE], length: 0 };
        match e1000::receive(&mut frame.bytes) {
            Ok(Some(length)) => {
                frame.length = length;
                Some((frame, TransmitToken))
            }
            Ok(None) | Err(_) => None,
        }
    }

    fn transmit(&mut self, _timestamp: Instant) -> Option<Self::TxToken<'_>> {
        Some(TransmitToken)
    }

    fn capabilities(&self) -> DeviceCapabilities {
        let mut capabilities = DeviceCapabilities::default();
        capabilities.medium = Medium::Ethernet;
        // Ethernet MTU includes its 14-byte header, but not the FCS.
        capabilities.max_transmission_unit = 1514;
        capabilities.max_burst_size = Some(1);
        capabilities
    }
}

struct NetworkState {
    interface: Interface,
    sockets: SocketSet<'static>,
    dhcp_socket: SocketHandle,
    udp_sockets: Vec<UdpEndpoint>,
    next_ephemeral_port: u16,
    ipv4: [u8; 4],
    gateway: [u8; 4],
    dns: [Option<[u8; 4]>; 2],
    dhcp_configured: bool,
    polls: u64,
}

struct UdpEndpoint {
    id: u32,
    socket: SocketHandle,
    references: u32,
    connected: Option<IpEndpoint>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UdpError {
    NotReady,
    NotFound,
    NoSpace,
    AddressInUse,
    AddressNotAvailable,
    NotConnected,
    MessageTooLarge,
    WouldBlock,
    Invalid,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UdpPeer {
    pub address: [u8; 4],
    pub port: u16,
}

static STATE: SpinLock<Option<NetworkState>> = SpinLock::new(None);
static TX_ERRORS: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
static NEXT_UDP_ID: AtomicU32 = AtomicU32::new(1);
static UDP_WAITERS: crate::task::wait::WaitQueue = crate::task::wait::WaitQueue::new();

/// Configure QEMU's user-mode network defaults and start DHCP. The static
/// address remains available while a lease is being acquired.
pub fn init(mac: [u8; 6]) -> Result<(), &'static str> {
    let mut device = E1000Device;
    let hardware = HardwareAddress::Ethernet(EthernetAddress(mac));
    let mut config = Config::new(hardware);
    config.random_seed = crate::arch::x86_64::port::rdtsc();
    let now = Instant::from_millis(0);
    let mut interface = Interface::new(config, &mut device, now);
    interface.update_ip_addrs(|addresses| {
        addresses
            .push(IpCidr::new(IpAddress::v4(10, 0, 2, 15), 24))
            .expect("one IPv4 address fits in the interface table");
    });
    interface
        .routes_mut()
        .add_default_ipv4_route(Ipv4Address::new(10, 0, 2, 2))
        .map_err(|_| "failed to configure IPv4 default route")?;
    let mut sockets = SocketSet::new(Vec::new());
    let dhcp_socket = sockets.add(smoltcp::socket::dhcpv4::Socket::new());
    *STATE.lock() = Some(NetworkState {
        interface,
        sockets,
        dhcp_socket,
        udp_sockets: Vec::new(),
        next_ephemeral_port: EPHEMERAL_FIRST,
        ipv4: [10, 0, 2, 15],
        gateway: [10, 0, 2, 2],
        dns: [Some([10, 0, 2, 3]), None],
        dhcp_configured: false,
        polls: 0,
    });
    Ok(())
}

/// Run one bounded protocol-stack service pass. Called from the PIT tick until
/// a dedicated network worker and interrupt-driven RX queue are available.
pub fn poll() {
    let mut guard = STATE.lock();
    let Some(state) = guard.as_mut() else { return };
    let now = Instant::from_millis(
        crate::task::scheduler::tick_count().saturating_mul(10) as i64,
    );
    let mut device = E1000Device;
    let _ = state.interface.poll(now, &mut device, &mut state.sockets);
    let dhcp_event = state
        .sockets
        .get_mut::<smoltcp::socket::dhcpv4::Socket>(state.dhcp_socket)
        .poll();
    match dhcp_event {
        Some(smoltcp::socket::dhcpv4::Event::Configured(config)) => {
            state.dhcp_configured = true;
            state.ipv4 = config.address.address().octets();
            state.interface.update_ip_addrs(|addresses| {
                addresses.clear();
                let _ = addresses.push(IpCidr::Ipv4(config.address));
            });
            if let Some(router) = config.router {
                state.gateway = router.octets();
                let _ = state.interface.routes_mut().add_default_ipv4_route(router);
            } else {
                state.gateway = [0; 4];
                state.interface.routes_mut().remove_default_ipv4_route();
            }
            state.dns = [None, None];
            for (slot, server) in state.dns.iter_mut().zip(config.dns_servers.iter()) {
                *slot = Some(server.octets());
            }
            crate::kprintln!(
                "DHCP configured: {}.{}.{}.{}/{}",
                state.ipv4[0], state.ipv4[1], state.ipv4[2], state.ipv4[3], config.address.prefix_len()
            );
        }
        Some(smoltcp::socket::dhcpv4::Event::Deconfigured) => {
            if state.dhcp_configured {
                state.interface.update_ip_addrs(|addresses| addresses.clear());
                state.interface.routes_mut().remove_default_ipv4_route();
                state.ipv4 = [0; 4];
                state.gateway = [0; 4];
                state.dns = [None, None];
                state.dhcp_configured = false;
                crate::kprintln!("DHCP lease lost");
            }
        }
        None => {}
    }
    state.polls = state.polls.wrapping_add(1);
    let udp_ready = state.udp_sockets.iter().any(|endpoint| {
        state.sockets.get::<UdpSocket>(endpoint.socket).can_recv()
    });
    drop(guard);
    if udp_ready {
        UDP_WAITERS.wake_all();
    }
}

pub fn udp_wait_queue() -> &'static crate::task::wait::WaitQueue {
    &UDP_WAITERS
}

pub fn udp_open() -> Result<u32, UdpError> {
    let mut guard = STATE.lock();
    let state = guard.as_mut().ok_or(UdpError::NotReady)?;
    if state.udp_sockets.len() >= MAX_UDP_SOCKETS {
        return Err(UdpError::NoSpace);
    }
    let rx_metadata = vec![PacketMetadata::EMPTY; UDP_BUFFER_PACKETS];
    let tx_metadata = vec![PacketMetadata::EMPTY; UDP_BUFFER_PACKETS];
    let rx_buffer = PacketBuffer::new(rx_metadata, vec![0; UDP_BUFFER_BYTES]);
    let tx_buffer = PacketBuffer::new(tx_metadata, vec![0; UDP_BUFFER_BYTES]);
    let socket = state.sockets.add(UdpSocket::new(rx_buffer, tx_buffer));
    let mut id = NEXT_UDP_ID.fetch_add(1, Ordering::Relaxed);
    if id == 0 {
        id = NEXT_UDP_ID.fetch_add(1, Ordering::Relaxed);
    }
    state.udp_sockets.push(UdpEndpoint {
        id,
        socket,
        references: 1,
        connected: None,
    });
    Ok(id)
}

pub fn udp_retain(id: u32) -> Result<(), UdpError> {
    let mut guard = STATE.lock();
    let state = guard.as_mut().ok_or(UdpError::NotReady)?;
    let endpoint = state.udp_sockets.iter_mut().find(|item| item.id == id).ok_or(UdpError::NotFound)?;
    endpoint.references = endpoint.references.checked_add(1).ok_or(UdpError::NoSpace)?;
    Ok(())
}

pub fn udp_release(id: u32) {
    let mut guard = STATE.lock();
    let Some(state) = guard.as_mut() else { return };
    let Some(index) = state.udp_sockets.iter().position(|item| item.id == id) else { return };
    if state.udp_sockets[index].references > 1 {
        state.udp_sockets[index].references -= 1;
    } else {
        let endpoint = state.udp_sockets.remove(index);
        let _ = state.sockets.remove(endpoint.socket);
    }
}

fn udp_index(state: &NetworkState, id: u32) -> Result<usize, UdpError> {
    state.udp_sockets.iter().position(|item| item.id == id).ok_or(UdpError::NotFound)
}

fn port_is_used(state: &NetworkState, port: u16) -> bool {
    state.udp_sockets.iter().any(|endpoint| {
        state.sockets.get::<UdpSocket>(endpoint.socket).endpoint().port == port
    })
}

fn bind_udp(state: &mut NetworkState, index: usize, address: Option<[u8; 4]>, requested_port: u16) -> Result<u16, UdpError> {
    if let Some(address) = address {
        if address != [0; 4] && address != state.ipv4 {
            return Err(UdpError::AddressNotAvailable);
        }
    }
    let smol_address = address.filter(|address| *address != [0; 4])
        .map(|address| IpAddress::v4(address[0], address[1], address[2], address[3]));
    let socket_handle = state.udp_sockets[index].socket;
    if state.sockets.get::<UdpSocket>(socket_handle).is_open() {
        return Err(UdpError::AddressInUse);
    }
    if requested_port != 0 {
        if port_is_used(state, requested_port) {
            return Err(UdpError::AddressInUse);
        }
        state.sockets.get_mut::<UdpSocket>(socket_handle)
            .bind(IpListenEndpoint { addr: smol_address, port: requested_port })
            .map_err(|_| UdpError::AddressInUse)?;
        return Ok(requested_port);
    }
    let count = (EPHEMERAL_LAST - EPHEMERAL_FIRST + 1) as usize;
    for _ in 0..count {
        let port = state.next_ephemeral_port;
        state.next_ephemeral_port = if port == EPHEMERAL_LAST { EPHEMERAL_FIRST } else { port + 1 };
        if port_is_used(state, port) {
            continue;
        }
        if state.sockets.get_mut::<UdpSocket>(socket_handle)
            .bind(IpListenEndpoint { addr: smol_address, port }).is_ok()
        {
            return Ok(port);
        }
    }
    Err(UdpError::NoSpace)
}

pub fn udp_bind(id: u32, address: [u8; 4], port: u16) -> Result<u16, UdpError> {
    let mut guard = STATE.lock();
    let state = guard.as_mut().ok_or(UdpError::NotReady)?;
    let index = udp_index(state, id)?;
    bind_udp(state, index, Some(address), port)
}

pub fn udp_connect(id: u32, peer: UdpPeer) -> Result<(), UdpError> {
    if peer.address == [0; 4] || peer.port == 0 {
        return Err(UdpError::AddressNotAvailable);
    }
    let mut guard = STATE.lock();
    let state = guard.as_mut().ok_or(UdpError::NotReady)?;
    let index = udp_index(state, id)?;
    if !state.sockets.get::<UdpSocket>(state.udp_sockets[index].socket).is_open() {
        bind_udp(state, index, None, 0)?;
    }
    state.udp_sockets[index].connected = Some(IpEndpoint::new(
        IpAddress::v4(peer.address[0], peer.address[1], peer.address[2], peer.address[3]),
        peer.port,
    ));
    Ok(())
}

pub fn udp_send_to(id: u32, bytes: &[u8], peer: UdpPeer) -> Result<usize, UdpError> {
    if bytes.len() > UDP_BUFFER_BYTES { return Err(UdpError::MessageTooLarge); }
    if peer.address == [0; 4] || peer.port == 0 { return Err(UdpError::AddressNotAvailable); }
    let mut guard = STATE.lock();
    let state = guard.as_mut().ok_or(UdpError::NotReady)?;
    let index = udp_index(state, id)?;
    if !state.sockets.get::<UdpSocket>(state.udp_sockets[index].socket).is_open() {
        bind_udp(state, index, None, 0)?;
    }
    let destination = IpEndpoint::new(
        IpAddress::v4(peer.address[0], peer.address[1], peer.address[2], peer.address[3]),
        peer.port,
    );
    state.sockets.get_mut::<UdpSocket>(state.udp_sockets[index].socket)
        .send_slice(bytes, destination)
        .map_err(|_| UdpError::WouldBlock)?;
    Ok(bytes.len())
}

pub fn udp_send_connected(id: u32, bytes: &[u8]) -> Result<usize, UdpError> {
    let peer = {
        let guard = STATE.lock();
        let state = guard.as_ref().ok_or(UdpError::NotReady)?;
        let index = udp_index(state, id)?;
        state.udp_sockets[index].connected.ok_or(UdpError::NotConnected)?
    };
    let IpAddress::Ipv4(address) = peer.addr;
    udp_send_to(id, bytes, UdpPeer { address: address.octets(), port: peer.port })
}

pub fn udp_receive(id: u32, buffer: &mut [u8]) -> Result<Option<(usize, UdpPeer)>, UdpError> {
    let mut guard = STATE.lock();
    let state = guard.as_mut().ok_or(UdpError::NotReady)?;
    let index = udp_index(state, id)?;
    let socket_handle = state.udp_sockets[index].socket;
    let connected = state.udp_sockets[index].connected;
    for _ in 0..UDP_BUFFER_PACKETS {
        let received = match state.sockets.get_mut::<UdpSocket>(socket_handle).recv_slice(buffer) {
            Ok((length, metadata)) => Some((length, metadata.endpoint)),
            Err(smoltcp::socket::udp::RecvError::Exhausted) => None,
            Err(smoltcp::socket::udp::RecvError::Truncated) => return Err(UdpError::MessageTooLarge),
        };
        let Some((length, endpoint)) = received else { return Ok(None) };
        if connected.is_some_and(|peer| peer != endpoint) {
            continue;
        }
        let IpAddress::Ipv4(address) = endpoint.addr;
        return Ok(Some((length, UdpPeer { address: address.octets(), port: endpoint.port })));
    }
    Ok(None)
}

#[derive(Clone, Copy, Debug)]
pub struct NetworkInfo {
    pub mac: [u8; 6],
    pub ipv4: [u8; 4],
    pub gateway: [u8; 4],
    pub poll_count: u64,
    pub transmit_errors: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct NetworkConfig {
    pub version: u32,
    pub ipv4: [u8; 4],
    pub gateway: [u8; 4],
    pub dns_count: u32,
    pub dns: [[u8; 4]; 2],
}

pub fn config() -> Option<NetworkConfig> {
    let guard = STATE.lock();
    let state = guard.as_ref()?;
    let mut dns = [[0u8; 4]; 2];
    let mut count = 0;
    for server in state.dns.iter().flatten() {
        if count == dns.len() { break; }
        dns[count] = *server;
        count += 1;
    }
    Some(NetworkConfig {
        version: 1,
        ipv4: state.ipv4,
        gateway: state.gateway,
        dns_count: count as u32,
        dns,
    })
}

pub fn info() -> Option<NetworkInfo> {
    let state = STATE.lock();
    let state = state.as_ref()?;
    Some(NetworkInfo {
        mac: e1000::mac_address()?,
        ipv4: state.ipv4,
        gateway: state.gateway,
        poll_count: state.polls,
        transmit_errors: TX_ERRORS.load(core::sync::atomic::Ordering::Relaxed),
    })
}
