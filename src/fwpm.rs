/// doc: https://learn.microsoft.com/en-us/windows/win32/fwp/viewing-current-state
use crate::error::*;
use widestring::WideCString;
use windows_sys::Win32::NetworkManagement::WindowsFilteringPlatform::{self as fwpm};

pub use filters::get_filters;
pub use sub_layer::{add_sub_layer, get_sub_layers};

#[derive(Clone, Debug)]
pub struct DisplayData {
    pub name: WideCString,
    pub desc: Option<WideCString>,
}

impl From<fwpm::FWPM_DISPLAY_DATA0> for DisplayData {
    fn from(value: fwpm::FWPM_DISPLAY_DATA0) -> Self {
        let name = unsafe { WideCString::from_ptr_str(value.name) };
        let desc = if !value.description.is_null() {
            unsafe { Some(WideCString::from_ptr_str(value.description)) }
        } else {
            None
        };
        Self { name, desc }
    }
}

impl DisplayData {
    pub fn new(name: String, desc: Option<String>) -> Self {
        Self {
            name: WideCString::from_str(name).unwrap(),
            desc: desc.map(|d| WideCString::from_str(d).unwrap()),
        }
    }

    pub fn as_raw(&self) -> fwpm::FWPM_DISPLAY_DATA0 {
        fwpm::FWPM_DISPLAY_DATA0 {
            name: self.name.as_ptr(),
            description: if self.desc.is_some() {
                self.desc.as_ref().unwrap().as_ptr()
            } else {
                std::ptr::null_mut()
            },
        }
    }
}

#[inline(always)]
fn empty_provider_data() -> fwpm::FWP_BYTE_BLOB {
    fwpm::FWP_BYTE_BLOB {
        data: std::ptr::null_mut(),
        size: 0,
    }
}

fn get_engine_handle() -> Result<isize> {
    let mut session: fwpm::FWPM_SESSION0;
    unsafe {
        session = core::mem::zeroed();
    }
    session.flags = fwpm::FWPM_SESSION_FLAG_DYNAMIC;
    let mut engine_handle = 0;
    let code = unsafe {
        fwpm::FwpmEngineOpen0(
            std::ptr::null(),
            windows_sys::Win32::System::Rpc::RPC_C_AUTHN_DEFAULT as _,
            std::ptr::null(),
            &session,
            &mut engine_handle as *mut _,
        )
    };
    if code != 0 {
        return Err(Error {
            kind: ErrorKind::Os(code),
        });
    }
    Ok(engine_handle)
}

mod filters {
    use super::*;

    /// will get all the filters in the raw format
    pub unsafe fn get_filters() -> Result<Vec<fwpm::FWPM_FILTER0>> {
        let engine_handle = get_engine_handle()?;

        let template = [0u8; std::mem::size_of::<fwpm::FWPM_FILTER_ENUM_TEMPLATE0>()];
        let mut template: fwpm::FWPM_FILTER_ENUM_TEMPLATE0 =
            unsafe { std::mem::transmute(template) };

        template.layerKey = fwpm::FWPM_LAYER_ALE_AUTH_CONNECT_V4;
        template.actionMask = 0xFFFFFFFF;

        let mut enum_handle = 0_isize;
        let code = {
            fwpm::FwpmFilterCreateEnumHandle0(
                engine_handle,
                &template as *const _,
                &mut enum_handle as *mut _,
            )
        };

        if code != 0 {
            return Err(Error {
                kind: ErrorKind::Os(code),
            });
        }

        const NUM_REQ: usize = usize::MAX;
        let mut num_filters = 0_u32;
        let mut filters = std::ptr::null_mut() as *mut *mut fwpm::FWPM_FILTER0;
        let code = unsafe {
            fwpm::FwpmFilterEnum0(
                engine_handle,
                enum_handle,
                NUM_REQ as _,
                &mut filters as *mut *mut *mut fwpm::FWPM_FILTER0,
                &mut num_filters as *mut _,
            )
        };

        if code != 0 {
            return Err(Error {
                kind: ErrorKind::Os(code),
            });
        }

        let mut results = Vec::with_capacity(num_filters as usize);

        for i in 0..num_filters {
            let filter_ptr = filters.add(i as usize);
            if filter_ptr.is_null() {
                break;
            }
            let filter = unsafe { *filter_ptr };
            let mut filter_data = [0u8; std::mem::size_of::<fwpm::FWPM_FILTER0>()];
            std::ptr::copy_nonoverlapping(
                filter as *const u8,
                &mut filter_data as *mut _,
                std::mem::size_of::<fwpm::FWPM_FILTER0>(),
            );
            let filter_data: fwpm::FWPM_FILTER0 = unsafe { std::mem::transmute(filter_data) };
            results.push(filter_data);
        }

        Ok(results)
    }

    // pub fn add_filter(filter: &fwpm::FWPM_FILTER0) -> Result<u64> {
    //     let engine_handle = get_engine_handle()?;
    //     let mut filter_id = 0_u64;
    //     let code = unsafe {
    //         fwpm::FwpmFilterAdd0(
    //             engine_handle,
    //             filter,
    //             std::ptr::null(),
    //             &mut filter_id as *mut _,
    //         )
    //     };
    //     if code != 0 {
    //         return Err(Error {
    //             kind: ErrorKind::Os(code),
    //         });
    //     }
    //     Ok(filter_id)
    // }
}

mod sub_layer {
    use crate::utils::{bytes_to_guid, guid_to_bytes};

    use super::*;

    #[derive(Clone, Debug)]
    pub struct SubLayer {
        pub sub_layer_key: [u8; 16],
        pub display_data: DisplayData,
        pub flags: u32,
    }

    impl From<fwpm::FWPM_SUBLAYER0> for SubLayer {
        fn from(value: fwpm::FWPM_SUBLAYER0) -> Self {
            let sub_layer_key = guid_to_bytes(&value.subLayerKey);
            let display_data = value.displayData.into();
            let flags = value.flags;
            Self {
                sub_layer_key,
                display_data,
                flags,
            }
        }
    }

    impl SubLayer {
        pub fn as_raw(&self) -> fwpm::FWPM_SUBLAYER0 {
            fwpm::FWPM_SUBLAYER0 {
                subLayerKey: bytes_to_guid(self.sub_layer_key),
                displayData: self.display_data.as_raw(),
                flags: self.flags,
                providerKey: std::ptr::null_mut(),
                providerData: empty_provider_data(),
                weight: u16::MAX,
            }
        }
    }

    fn get_enum_handle(engine_handle: isize) -> Result<isize> {
        let template = [0u8; std::mem::size_of::<fwpm::FWPM_SUBLAYER_ENUM_TEMPLATE0>()];
        let template: fwpm::FWPM_SUBLAYER_ENUM_TEMPLATE0 = unsafe { std::mem::transmute(template) };

        let mut enum_handle = 0_isize;
        let code = unsafe {
            fwpm::FwpmSubLayerCreateEnumHandle0(
                engine_handle,
                &template as *const _,
                &mut enum_handle as *mut _,
            )
        };

        if code != 0 {
            return Err(Error {
                kind: ErrorKind::Os(code),
            });
        }
        Ok(enum_handle)
    }

    pub fn get_sub_layers() -> Result<Vec<SubLayer>> {
        let engine_handle = get_engine_handle()?;
        let enum_handle = get_enum_handle(engine_handle)?;

        // list all sublayers
        const NUM_REQ: usize = usize::MAX;
        let mut num_layers = 0_u32;
        let mut layers = std::ptr::null_mut() as *mut *mut fwpm::FWPM_SUBLAYER0;
        let code = unsafe {
            fwpm::FwpmSubLayerEnum0(
                engine_handle,
                enum_handle,
                NUM_REQ as _,
                &mut layers as *mut *mut *mut fwpm::FWPM_SUBLAYER0,
                &mut num_layers as *mut _,
            )
        };
        if code != 0 {
            return Err(Error {
                kind: ErrorKind::Os(code),
            });
        }
        if layers.is_null() {
            return Err(Error {
                kind: ErrorKind::Os(0),
            });
        }
        let mut results = Vec::with_capacity(num_layers as usize);

        for i in 0..num_layers {
            let filter_ptr: *mut *mut fwpm::FWPM_SUBLAYER0 = unsafe { layers.add(i as usize) };
            if filter_ptr.is_null() {
                break;
            }
            let filter = unsafe { *filter_ptr };
            let mut filter_data = [0u8; std::mem::size_of::<fwpm::FWPM_SUBLAYER0>()];
            unsafe {
                std::ptr::copy_nonoverlapping(
                    filter as *const u8,
                    &mut filter_data as *mut _,
                    std::mem::size_of::<fwpm::FWPM_SUBLAYER0>(),
                );
            }
            let layer_data: fwpm::FWPM_SUBLAYER0 = unsafe { std::mem::transmute(filter_data) };
            results.push(layer_data.into());
        }
        Ok(results)
    }

    pub fn add_sub_layer(sub_layer: &SubLayer) -> Result<()> {
        let engine_handle = get_engine_handle()?;
        let sub_layer = sub_layer.as_raw();
        let code = unsafe {
            fwpm::FwpmSubLayerAdd0(engine_handle, &sub_layer as *const _, std::ptr::null())
        };
        if code != 0 {
            return Err(Error {
                kind: ErrorKind::Os(code),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use self::sub_layer::{add_sub_layer, SubLayer};
    use super::*;
    use crate::utils::{generate_guid, guid_to_bytes};

    #[test]
    fn test_get_filters() {
        let filters = unsafe { filters::get_filters().unwrap() };
        unsafe {
            for filter in filters {
                let display = WideCString::from_ptr_str(filter.displayData.name)
                    .to_string()
                    .unwrap();
                let desc = if !filter.displayData.description.is_null() {
                    Some(
                        WideCString::from_ptr_str(filter.displayData.description)
                            .to_string()
                            .unwrap(),
                    )
                } else {
                    None
                };
                println!("[FILTER] {:?}, {:?}", display, desc);
                if display == "sing-tun" {
                    // print each field of the filter
                    let data = dump_filter(filter);
                    println!("{:x?}", &data);
                }
            }
        }
    }

    const FILTER_DATA_SIZE: usize = std::mem::size_of::<fwpm::FWPM_FILTER0>();

    unsafe fn dump_filter(filter: fwpm::FWPM_FILTER0) -> [u8; FILTER_DATA_SIZE] {
        let raw_data: [u8; std::mem::size_of::<fwpm::FWPM_FILTER0>()] =
            unsafe { std::mem::transmute(filter) };
        raw_data
    }

    #[test]
    fn test_get_sub_layers() {
        let sub_layers = sub_layer::get_sub_layers().unwrap();
        for sub_layer in sub_layers {
            println!("[SUB LAYER] {:?}", sub_layer.display_data);
        }
    }

    #[test]
    fn test_add_layer() {
        let display_data = DisplayData::new("clashrs".into(), Some("clash".into()));
        let sub_layer = SubLayer {
            sub_layer_key: guid_to_bytes(&generate_guid()),
            display_data,
            flags: 0,
        };
        println!("adding sub layer {:?}", sub_layer);

        add_sub_layer(&sub_layer).unwrap();
        let sub_layers = sub_layer::get_sub_layers().unwrap();
        for sub_layer in &sub_layers {
            println!("[SUB LAYER] {:?}", sub_layer.display_data);
        }
        let find = sub_layers
            .iter()
            .find(|l| l.sub_layer_key == sub_layer.sub_layer_key);
        assert!(find.is_some());
        let find = find.unwrap();
        println!("find the added sub layer {:?}", find.display_data);
    }

    // #[test]
    // #[expect(fail)]
    // fn test_add_filter() {
    //     let mut filter: fwpm::FWPM_FILTER0;
    //     unsafe {
    //         filter = core::mem::zeroed();
    //     }
    //     filter.numFilterConditions = 0;
    //     filter.filterCondition = std::ptr::null_mut();
    //     let f1_display = DisplayData::new("filter1".into(), Some("filter1".into()));
    //     filter.displayData = f1_display.as_raw();
    //     filter.layerKey = fwpm::FWPM_LAYER_ALE_AUTH_CONNECT_V4;
    //     filter.action.r#type = fwpm::FWP_ACTION_FLAG_TERMINATING;
    //     filter.flags = fwpm::FWPM_FILTER_FLAG_CLEAR_ACTION_RIGHT;
    //     filter.weight.r#type = fwpm::FWP_EMPTY;
    //     add_filter(&filter).unwrap();
    //     drop(f1_display);
    // }
}
