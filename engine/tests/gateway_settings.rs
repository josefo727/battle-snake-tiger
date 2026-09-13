use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use proptest::prelude::*;
use tiger_engine::gateway::settings::{Settings, SettingsError};

fn lookup<'a>(vars: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
    let map: HashMap<&str, &str> = vars.iter().copied().collect();
    move |name| map.get(name).map(|value| (*value).to_owned())
}

#[test]
fn defaults_to_every_interface_on_port_8080() {
    let settings = Settings::from_lookup(lookup(&[])).expect("the defaults are valid");

    assert_eq!(settings.bind_addr, IpAddr::V4(Ipv4Addr::UNSPECIFIED));
    assert_eq!(settings.port, 8080);
}

#[test]
fn reads_an_explicit_address_and_port() {
    let settings =
        Settings::from_lookup(lookup(&[("BIND_ADDR", "127.0.0.1"), ("PORT", "3000")])).unwrap();

    assert_eq!(settings.bind_addr, IpAddr::V4(Ipv4Addr::LOCALHOST));
    assert_eq!(settings.port, 3000);
}

#[test]
fn accepts_an_ipv6_address_and_port_zero_for_an_ephemeral_port() {
    let settings = Settings::from_lookup(lookup(&[("BIND_ADDR", "::1"), ("PORT", "0")])).unwrap();

    assert_eq!(settings.bind_addr, IpAddr::V6(Ipv6Addr::LOCALHOST));
    assert_eq!(settings.port, 0);
}

#[test]
fn each_variable_falls_back_independently() {
    let only_port = Settings::from_lookup(lookup(&[("PORT", "9000")])).unwrap();
    let only_addr = Settings::from_lookup(lookup(&[("BIND_ADDR", "192.0.2.5")])).unwrap();

    assert_eq!(only_port.bind_addr, IpAddr::V4(Ipv4Addr::UNSPECIFIED));
    assert_eq!(only_port.port, 9000);
    assert_eq!(only_addr.bind_addr, IpAddr::V4(Ipv4Addr::new(192, 0, 2, 5)));
    assert_eq!(only_addr.port, 8080);
}

#[test]
fn rejects_an_address_that_is_not_an_ip() {
    let error = Settings::from_lookup(lookup(&[("BIND_ADDR", "localhost")])).unwrap_err();

    assert_eq!(
        error,
        SettingsError::InvalidBindAddr("localhost".to_owned())
    );
}

#[test]
fn rejects_ports_that_are_not_a_number_from_0_to_65535() {
    for bad in ["abc", "70000", "-1", "80.5", "", " 8080", "0x50"] {
        let error = Settings::from_lookup(lookup(&[("PORT", bad)])).unwrap_err();

        assert_eq!(error, SettingsError::InvalidPort(bad.to_owned()), "{bad:?}");
    }
}

#[test]
fn an_invalid_address_is_reported_before_an_invalid_port() {
    let error =
        Settings::from_lookup(lookup(&[("BIND_ADDR", "nope"), ("PORT", "nope")])).unwrap_err();

    assert!(matches!(error, SettingsError::InvalidBindAddr(_)));
}

#[test]
fn errors_name_the_variable_the_offending_value_and_what_would_work() {
    let address = SettingsError::InvalidBindAddr("localhost".to_owned()).to_string();
    let port = SettingsError::InvalidPort("70000".to_owned()).to_string();

    assert!(
        address.contains("BIND_ADDR") && address.contains("\"localhost\""),
        "{address}"
    );
    assert!(
        address.contains("0.0.0.0") || address.contains("127.0.0.1"),
        "{address}"
    );
    assert!(
        port.contains("PORT") && port.contains("\"70000\""),
        "{port}"
    );
    assert!(port.contains("65535"), "{port}");
}

#[test]
fn the_socket_address_combines_both_settings() {
    let settings =
        Settings::from_lookup(lookup(&[("BIND_ADDR", "127.0.0.1"), ("PORT", "4321")])).unwrap();

    assert_eq!(
        settings.socket_addr(),
        SocketAddr::from(([127, 0, 0, 1], 4321))
    );
}

proptest! {
    #[test]
    fn every_port_in_range_round_trips(port in any::<u16>(), octets in any::<[u8; 4]>()) {
        let address = Ipv4Addr::from(octets).to_string();
        let port_text = port.to_string();

        let settings =
            Settings::from_lookup(lookup(&[("BIND_ADDR", &address), ("PORT", &port_text)])).unwrap();

        prop_assert_eq!(settings.port, port);
        prop_assert_eq!(settings.socket_addr(), SocketAddr::from((octets, port)));
    }
}
