fn main() {
    let adapters = ipconfig2::get_adapters().unwrap();

    for adapter in adapters {
        let guid = adapter.guid.replace("{", "").replace("}", "");
        let uuid = uuid::Uuid::parse_str(&guid).unwrap();
        println!("{:?}: {}", adapter.ip_addresses, uuid.to_string());
    }
}
