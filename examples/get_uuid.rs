fn main() {
    let adapters = ipconfig2::get_adapters().unwrap();

    for adapter in adapters {
        let uuid = uuid::Uuid::from_bytes(adapter.network_guid);
        println!("{:?}: {}", adapter.ip_addresses, uuid);
    }
}
