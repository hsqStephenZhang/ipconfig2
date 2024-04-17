use windows_sys::Win32::NetworkManagement::IpHelper::ConvertInterfaceLuidToGuid;

pub fn guid_to_bytes(guid: &windows_sys::core::GUID) -> [u8; 16] {
    let data1_bytes = guid.data1.to_ne_bytes();
    let data2_bytes = guid.data2.to_ne_bytes();
    let data3_bytes = guid.data3.to_ne_bytes();
    [
        data1_bytes[0],
        data1_bytes[1],
        data1_bytes[2],
        data1_bytes[3],
        data2_bytes[0],
        data2_bytes[1],
        data3_bytes[0],
        data3_bytes[1],
        guid.data4[0],
        guid.data4[1],
        guid.data4[2],
        guid.data4[3],
        guid.data4[4],
        guid.data4[5],
        guid.data4[6],
        guid.data4[7],
    ]
}

pub fn bytes_to_guid(bytes: [u8; 16]) -> windows_sys::core::GUID {
    windows_sys::core::GUID {
        data1: u32::from_ne_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
        data2: u16::from_ne_bytes([bytes[4], bytes[5]]),
        data3: u16::from_ne_bytes([bytes[6], bytes[7]]),
        data4: [
            bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15],
        ],
    }
}

pub fn luid_to_guid(luid: u64) -> windows_sys::core::GUID {
    let mut guid = windows_sys::core::GUID {
        data1: 0,
        data2: 0,
        data3: 0,
        data4: [0; 8],
    };
    unsafe {
        let luid = windows_sys::Win32::NetworkManagement::IpHelper::NET_LUID_LH { Value: luid };
        ConvertInterfaceLuidToGuid(&luid as *const _, &mut guid as *mut _);
    }
    guid
}
