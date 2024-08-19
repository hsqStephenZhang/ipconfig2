use std::{
    cell::RefCell,
    collections::HashMap,
    ffi::CString,
    io::{self, ErrorKind},
    mem,
    os::windows::io::AsRawSocket,
    time::{Duration, Instant},
};

use windows_sys::Win32::{
    NetworkManagement::IpHelper::if_nametoindex,
    Networking::WinSock::{
        htonl, setsockopt, WSAGetLastError, IPPROTO_IP, IPPROTO_IPV6, IPV6_UNICAST_IF,
        IP_UNICAST_IF, SOCKET, SOCKET_ERROR,
    },
};

type PCSTR = *const u8;

pub fn find_adapter_interface_index(is_ipv6: bool, iface: &str) -> io::Result<Option<u32>> {
    let adapaters =
        crate::get_adapters().map_err(|e| io::Error::new(io::ErrorKind::NotFound, e))?;
    let if_index = adapaters
        .iter()
        .filter(|a| {
            if is_ipv6 {
                a.ipv6_if_index != 0
            } else {
                a.ipv4_if_index != 0
            }
        })
        .find(|a| a.friendly_name == iface || a.adapter_name == iface)
        .map(|adapter| adapter.ipv4_if_index);
    Ok(if_index)
}

fn find_interface_index_cached(is_ipv6: bool, iface: &str) -> io::Result<u32> {
    const INDEX_EXPIRE_DURATION: Duration = Duration::from_secs(5);

    thread_local! {
        static INTERFACE_INDEX_CACHE: RefCell<HashMap<String, (u32, Instant)>> =
            RefCell::new(HashMap::new());
    }

    let cache_index = INTERFACE_INDEX_CACHE.with(|cache| cache.borrow().get(iface).cloned());
    if let Some((idx, insert_time)) = cache_index {
        // short-path, cache hit for most cases
        let now = Instant::now();
        if now - insert_time < INDEX_EXPIRE_DURATION {
            return Ok(idx);
        }
    }

    // Get from API GetAdaptersAddresses
    let idx = match find_adapter_interface_index(is_ipv6, iface)? {
        Some(idx) => idx,
        None => unsafe {
            //  Windows if_nametoindex requires a C-string for interface name
            let ifname = CString::new(iface).expect("iface");

            //  https:docs.microsoft.com/en-us/previous-versions/windows/hardware/drivers/ff553788(v=vs.85)
            let if_index = if_nametoindex(ifname.as_ptr() as PCSTR);
            if if_index == 0 {
                // If the if_nametoindex function fails and returns zero, it is not possible to determine an error code.
                tracing::error!("if_nametoindex {} fails", iface);
                return Err(io::Error::new(
                    ErrorKind::InvalidInput,
                    "invalid interface name",
                ));
            }

            if_index
        },
    };

    INTERFACE_INDEX_CACHE.with(|cache| {
        cache
            .borrow_mut()
            .insert(iface.to_owned(), (idx, Instant::now()));
    });

    Ok(idx)
}

// the addr doesn't matter, it's just a mark of ip version
#[allow(unused)]
pub fn set_ip_unicast_if<S: AsRawSocket>(socket: &S, is_ipv6: bool, iface: &str) -> io::Result<()> {
    let handle = socket.as_raw_socket() as SOCKET;

    let if_index = find_interface_index_cached(is_ipv6, iface)?;

    unsafe {
        //  https:docs.microsoft.com/en-us/windows/win32/winsock/ipproto-ip-socket-options
        let ret = if !is_ipv6 {
            // Interface index is in network byte order for IPPROTO_IP.
            let if_index = htonl(if_index);
            setsockopt(
                handle,
                IPPROTO_IP as i32,
                IP_UNICAST_IF as i32,
                &if_index as *const _ as PCSTR,
                mem::size_of_val(&if_index) as i32,
            )
        } else {
            // Interface index is in host byte order for IPPROTO_IPV6.
            setsockopt(
                handle,
                IPPROTO_IPV6 as i32,
                IPV6_UNICAST_IF as i32,
                &if_index as *const _ as PCSTR,
                mem::size_of_val(&if_index) as i32,
            )
        };

        if ret == SOCKET_ERROR {
            let err = io::Error::from_raw_os_error(WSAGetLastError());
            tracing::error!(
                "set IP_UNICAST_IF / IPV6_UNICAST_IF interface: {}, index: {}, error: {}",
                iface, if_index, err
            );
            return Err(err);
        }
    }

    Ok(())
}
