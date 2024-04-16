# Ipconfig, with cleaner code

The original [repo](https://github.com/liranringel/ipconfig) has done an great job in providing a user-friendly api to get all the adapters on windows, but it uses `build.rs` and `bindgen`, which can be simply avoided by adding some features in `windows-sys`. So i just refactor the original code.

## Examples

```rust
// Print the ip addresses and dns servers of all adapters:
for adapter in ipconfig::get_adapters()? {
    println!("Ip addresses: {:#?}", adapter.ip_addresses());
    println!("Dns servers: {:#?}", adapter.dns_servers());
}
```

## TODOs

- add some apis to manage fwpm

## License

Licensed under either of
 * Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)
at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you shall be dual licensed as above, without any
additional terms or conditions.
