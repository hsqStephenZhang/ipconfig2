use crate::{error::*, utils::luid_to_guid};
use widestring::WideCString;
use windows_sys::Win32::{Foundation::GetLastError, NetworkManagement::IpHelper};

// U can get the dns servers in `Adapter`'s field, here is the detailed version
pub fn get_dns_setting(luid: u64) -> Result<IpHelper::DNS_INTERFACE_SETTINGS> {
    let dns_setting = [0u8; std::mem::size_of::<IpHelper::DNS_INTERFACE_SETTINGS>()];
    let mut dns_setting: IpHelper::DNS_INTERFACE_SETTINGS =
        unsafe { std::mem::transmute(dns_setting) };
    dns_setting.Version = IpHelper::DNS_INTERFACE_SETTINGS_VERSION3;
    let guid = luid_to_guid(luid);
    let code = unsafe { IpHelper::GetInterfaceDnsSettings(guid, &mut dns_setting as *mut _) };
    if code != 0 {
        return Err(Error {
            kind: ErrorKind::Os(unsafe { GetLastError() }),
        });
    }
    Ok(dns_setting)
}

pub fn set_dns_setting_v4(
    luid: u64,
    is_ipv6: bool,
    servers: &[&str],
    search_list: &[&str],
) -> Result<()> {
    let dns_setting = [0u8; std::mem::size_of::<IpHelper::DNS_INTERFACE_SETTINGS>()];
    // safety: dns_setting is valid
    let mut dns_setting: IpHelper::DNS_INTERFACE_SETTINGS =
        unsafe { std::mem::transmute(dns_setting) };
    dns_setting.Version = IpHelper::DNS_INTERFACE_SETTINGS_VERSION3;
    let servers = WideCString::from_str(servers.join(",")).unwrap();
    let search_list = WideCString::from_str(search_list.join(",")).unwrap();
    dns_setting.NameServer = servers.as_ptr();
    dns_setting.SearchList = search_list.as_ptr();
    dns_setting.Flags = (IpHelper::DNS_SETTING_NAMESERVER | IpHelper::DNS_SETTING_SEARCHLIST) as _;
    if is_ipv6 {
        dns_setting.Flags |= IpHelper::DNS_SETTING_IPV6 as u64;
    }
    unsafe { set_dns_setting(luid, &dns_setting) }
}

/// safety: the dns_setting must be valid
unsafe fn set_dns_setting(
    luid: u64,
    dns_setting: &IpHelper::DNS_INTERFACE_SETTINGS,
) -> Result<()> {
    let guid = luid_to_guid(luid);
    let code = IpHelper::SetInterfaceDnsSettings(guid, dns_setting);
    if code != 0 {
        return Err(Error {
            kind: ErrorKind::Os(GetLastError()),
        });
    }
    Ok(())
}

#[test]
fn test_get_dns() {
    use widestring::WideCString;
    let adapters = crate::get_adapters().unwrap();

    unsafe {
        for adapter in adapters {
            let luid = adapter.luid;
            let setting = get_dns_setting(luid).unwrap();
            let nameserver = if setting.NameServer.is_null() {
                None
            } else {
                Some(WideCString::from_ptr_str(setting.NameServer).to_string_lossy())
            };
            let search_list = if setting.SearchList.is_null() {
                None
            } else {
                Some(WideCString::from_ptr_str(setting.SearchList).to_string_lossy())
            };
            println!(
                "name:{}, LUID: {}, nameserver: {:?}, search_list: {:?}",
                adapter.friendly_name, luid, nameserver, search_list
            );
        }
    }
}

#[test]
fn test_set_dns() {
    let adapter = crate::get_adapters()
        .unwrap()
        .into_iter()
        .find(|a| a.friendly_name == "utun64")
        .unwrap();
    let luid = adapter.luid;
    let servers = &["1.0.0.1"];
    let search_list = &[];
    set_dns_setting_v4(luid, false, servers, search_list).unwrap();
    let new_settings = get_dns_setting(luid).unwrap();
    assert!(new_settings.NameServer != std::ptr::null());
    assert_eq!(
        unsafe {
            WideCString::from_ptr_str(new_settings.NameServer)
                .to_string()
                .unwrap()
        },
        servers.join(",")
    );
}
