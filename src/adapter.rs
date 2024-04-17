#![allow(clippy::cast_ptr_alignment)]

use std;
use std::convert::TryFrom;
use std::ffi::CStr;
use std::net::IpAddr;

use crate::error::*;
use crate::utils::guid_to_bytes;
use socket2;
use widestring::WideCString;
use windows_sys::Win32::Foundation::ERROR_BUFFER_OVERFLOW;
use windows_sys::Win32::Foundation::ERROR_SUCCESS;

use windows_sys::Win32::NetworkManagement::IpHelper;
use windows_sys::Win32::Networking::WinSock;
// use windows_sys::Win32::System::Com::StringFromGUID2;

/// Represent an operational status of the adapter
/// See IP_ADAPTER_ADDRESSES docs for more details
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OperStatus {
    IfOperStatusUp = 1,
    IfOperStatusDown = 2,
    IfOperStatusTesting = 3,
    IfOperStatusUnknown = 4,
    IfOperStatusDormant = 5,
    IfOperStatusNotPresent = 6,
    IfOperStatusLowerLayerDown = 7,
}

/// Represent an interface type
/// See IANA docs on iftype for more details
/// <https://www.iana.org/assignments/ianaiftype-mib/ianaiftype-mib>
/// Note that we only support a subset of the IANA interface
/// types and in case the adapter has an unsupported type,
/// `IfType::Unsupported` is used. `IfType::Other`
/// is different from `IfType::Unsupported`, as the former
/// one is defined by the IANA itself.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum IfType {
    Other = 1,
    EthernetCsmacd = 6,
    Iso88025Tokenring = 9,
    Ppp = 23,
    SoftwareLoopback = 24,
    Atm = 37,
    Ieee80211 = 71,
    Tunnel = 131,
    Ieee1394 = 144,
    Unsupported,
    /// This enum may grow additional variants, so this makes sure clients
    /// don't count on exhaustive matching. (Otherwise, adding a new variant
    /// could break existing code.)
    #[doc(hidden)]
    __Nonexhaustive,
}

/// Represent an adapter.
#[derive(Debug)]
pub struct Adapter {
    pub adapter_name: String,
    pub network_guid: [u8; 16],
    pub luid: u64,
    pub ipv4_if_index: u32,
    pub ip_addresses: Vec<IpAddr>,
    pub prefixes: Vec<(IpAddr, u32)>,
    pub gateways: Vec<IpAddr>,
    pub dns_servers: Vec<IpAddr>,
    pub description: String,
    pub friendly_name: String,
    pub physical_address: Option<Vec<u8>>,
    pub receive_link_speed: u64,
    pub transmit_link_speed: u64,
    pub oper_status: OperStatus,
    pub if_type: IfType,
    pub ipv6_if_index: u32,
    pub ipv4_metric: u32,
    pub ipv6_metric: u32,
}

/// Get all the network adapters on this machine.
pub fn get_adapters() -> Result<Vec<Adapter>> {
    unsafe {
        // Preallocate 16K per Microsoft recommendation, see Remarks section
        // https://docs.microsoft.com/en-us/windows/desktop/api/iphlpapi/nf-iphlpapi-getadaptersaddresses
        let mut buf_len: u32 = 16384;
        let mut adapters_addresses_buffer = Vec::new();

        let mut result = ERROR_BUFFER_OVERFLOW;
        while result == ERROR_BUFFER_OVERFLOW {
            adapters_addresses_buffer.resize(buf_len as usize, 0);

            result = IpHelper::GetAdaptersAddresses(
                IpHelper::AF_UNSPEC,
                0x0080 | 0x0010, //GAA_FLAG_INCLUDE_GATEWAYS | GAA_FLAG_INCLUDE_PREFIX,
                std::ptr::null_mut(),
                adapters_addresses_buffer.as_mut_ptr() as *mut _,
                &mut buf_len as *mut _,
            );
        }

        if result != ERROR_SUCCESS {
            return Err(Error {
                kind: ErrorKind::Os(result),
            });
        }

        let mut adapters = vec![];
        let mut adapter_addresses_ptr =
            adapters_addresses_buffer.as_mut_ptr() as *const IpHelper::IP_ADAPTER_ADDRESSES_LH;

        while !adapter_addresses_ptr.is_null() {
            adapters.push(get_adapter(adapter_addresses_ptr)?);
            adapter_addresses_ptr = adapter_addresses_ptr.read_unaligned().Next;
        }

        Ok(adapters)
    }
}

// ref: https://learn.microsoft.com/en-us/windows/win32/api/iptypes/ns-iptypes-ip_adapter_addresses_lh
unsafe fn get_adapter(
    adapter_addresses_ptr: *const IpHelper::IP_ADAPTER_ADDRESSES_LH,
) -> Result<Adapter> {
    let adapter_addresses = adapter_addresses_ptr.read_unaligned();
    let guid = guid_to_bytes(&adapter_addresses.NetworkGuid);
    let luid = adapter_addresses.Luid.Value;
    let ipv4_if_index = adapter_addresses.Anonymous1.Anonymous.IfIndex;
    let adapter_name = CStr::from_ptr(adapter_addresses.AdapterName as _)
        .to_str()?
        .to_owned();
    let dns_servers = get_dns_servers(adapter_addresses.FirstDnsServerAddress)?;
    let gateways = get_gateways(adapter_addresses.FirstGatewayAddress)?;
    let prefixes = get_prefixes(adapter_addresses.FirstPrefix)?;
    let unicast_addresses = get_unicast_addresses(adapter_addresses.FirstUnicastAddress)?;
    let receive_link_speed: u64 = adapter_addresses.ReceiveLinkSpeed;
    let transmit_link_speed: u64 = adapter_addresses.TransmitLinkSpeed;
    let ipv4_metric = adapter_addresses.Ipv4Metric;
    let ipv6_metric = adapter_addresses.Ipv6Metric;
    let oper_status = match adapter_addresses.OperStatus {
        1 => OperStatus::IfOperStatusUp,
        2 => OperStatus::IfOperStatusDown,
        3 => OperStatus::IfOperStatusTesting,
        4 => OperStatus::IfOperStatusUnknown,
        5 => OperStatus::IfOperStatusDormant,
        6 => OperStatus::IfOperStatusNotPresent,
        7 => OperStatus::IfOperStatusLowerLayerDown,
        v => {
            panic!("unexpected OperStatus value: {}", v);
        }
    };
    let if_type = match adapter_addresses.IfType {
        1 => IfType::Other,
        6 => IfType::EthernetCsmacd,
        9 => IfType::Iso88025Tokenring,
        23 => IfType::Ppp,
        24 => IfType::SoftwareLoopback,
        37 => IfType::Atm,
        71 => IfType::Ieee80211,
        131 => IfType::Tunnel,
        144 => IfType::Ieee1394,
        _ => IfType::Unsupported,
    };
    let ipv6_if_index = adapter_addresses.Ipv6IfIndex;

    let description = WideCString::from_ptr_str(adapter_addresses.Description).to_string()?;
    let friendly_name = WideCString::from_ptr_str(adapter_addresses.FriendlyName).to_string()?;
    let physical_address = if adapter_addresses.PhysicalAddressLength == 0 {
        None
    } else {
        Some(
            adapter_addresses.PhysicalAddress[..adapter_addresses.PhysicalAddressLength as usize]
                .to_vec(),
        )
    };
    Ok(Adapter {
        adapter_name,
        network_guid: guid,
        luid,
        ipv4_if_index,
        ip_addresses: unicast_addresses,
        prefixes,
        gateways,
        dns_servers,
        description,
        friendly_name,
        physical_address,
        receive_link_speed,
        transmit_link_speed,
        oper_status,
        if_type,
        ipv6_if_index,
        ipv4_metric,
        ipv6_metric,
    })
}

unsafe fn socket_address_to_ipaddr(socket_address: &WinSock::SOCKET_ADDRESS) -> IpAddr {
    let (_, sockaddr) = socket2::SockAddr::try_init(|storage, length| {
        let sockaddr_length = usize::try_from(socket_address.iSockaddrLength).unwrap();
        assert!(sockaddr_length <= std::mem::size_of_val(&storage.read_unaligned()));
        let dst: *mut u8 = storage.cast();
        let src: *const u8 = socket_address.lpSockaddr.cast();
        dst.copy_from_nonoverlapping(src, sockaddr_length);
        std::ptr::write_unaligned(length, socket_address.iSockaddrLength);
        Ok(())
    })
    .unwrap();

    sockaddr.as_socket().map(|s| s.ip()).unwrap()
}

unsafe fn get_dns_servers(
    mut dns_server_ptr: *const IpHelper::IP_ADAPTER_DNS_SERVER_ADDRESS_XP,
) -> Result<Vec<IpAddr>> {
    let mut dns_servers = vec![];

    while !dns_server_ptr.is_null() {
        let dns_server = dns_server_ptr.read_unaligned();
        let ipaddr = socket_address_to_ipaddr(&dns_server.Address);
        dns_servers.push(ipaddr);

        dns_server_ptr = dns_server.Next;
    }

    Ok(dns_servers)
}

unsafe fn get_gateways(
    mut gateway_ptr: *const IpHelper::IP_ADAPTER_GATEWAY_ADDRESS_LH,
) -> Result<Vec<IpAddr>> {
    let mut gateways = vec![];

    while !gateway_ptr.is_null() {
        let gateway = gateway_ptr.read_unaligned();
        let ipaddr = socket_address_to_ipaddr(&gateway.Address);
        gateways.push(ipaddr);

        gateway_ptr = gateway.Next;
    }

    Ok(gateways)
}

unsafe fn get_unicast_addresses(
    mut unicast_addresses_ptr: *const IpHelper::IP_ADAPTER_UNICAST_ADDRESS_LH,
) -> Result<Vec<IpAddr>> {
    let mut unicast_addresses = vec![];

    while !unicast_addresses_ptr.is_null() {
        let unicast_address = unicast_addresses_ptr.read_unaligned();
        let ipaddr = socket_address_to_ipaddr(&unicast_address.Address);
        unicast_addresses.push(ipaddr);

        unicast_addresses_ptr = unicast_address.Next;
    }

    Ok(unicast_addresses)
}

unsafe fn get_prefixes(
    mut prefixes_ptr: *const IpHelper::IP_ADAPTER_PREFIX_XP,
) -> Result<Vec<(IpAddr, u32)>> {
    let mut prefixes = vec![];

    while !prefixes_ptr.is_null() {
        let prefix = prefixes_ptr.read_unaligned();
        let ipaddr = socket_address_to_ipaddr(&prefix.Address);
        prefixes.push((ipaddr, prefix.PrefixLength));

        prefixes_ptr = prefix.Next;
    }

    Ok(prefixes)
}

#[test]
fn test_convert() {
    let adapters = get_adapters().unwrap();
    for a in adapters {
        let mut guid0 = windows_sys::core::GUID {
            data1: 0,
            data2: 0,
            data3: 0,
            data4: [0; 8],
        };
        let luid0 = IpHelper::NET_LUID_LH { Value: a.luid };
        let code = unsafe {
            IpHelper::ConvertInterfaceLuidToGuid(&luid0 as *const _, &mut guid0 as *mut _)
        };
        assert!(code == 0);

        let mut luid1 = IpHelper::NET_LUID_LH { Value: 0 };
        let code = unsafe {
            IpHelper::ConvertInterfaceGuidToLuid(&guid0 as *const _, &mut luid1 as *mut _)
        };
        assert!(code == 0);
        assert_eq!(unsafe { luid1.Value }, a.luid);
    }
}

#[test]
fn test_get_dns() {
    let adapters = get_adapters().unwrap();
    for a in adapters {
        println!("{}: {:?}", a.friendly_name, a.dns_servers);
    }
}
