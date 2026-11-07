use super::*;

fn props(list: &[(&str, zbus::zvariant::Value<'_>)]) -> Props {
    list.iter().map(|(k, v)| (k.to_string(), OwnedValue::try_from(v.clone()).unwrap())).collect()
}

#[test]
fn a_device_is_read_from_its_properties() {
    use zbus::zvariant::Value;
    let p = props(&[
        ("Address", Value::from("AA:BB:CC:DD:EE:FF")),
        ("Name", Value::from("WH-1000")),
        ("Alias", Value::from("My headphones")),
        ("Icon", Value::from("audio-headphones")),
        ("Paired", Value::from(true)),
        ("Connected", Value::from(true)),
        ("RSSI", Value::from(-61i16)),
    ]);
    let d = device_of(&p).unwrap();
    assert_eq!(
        (d.name.as_str(), d.kind, d.paired, d.trusted, d.connected, d.rssi),
        ("My headphones", Kind::Audio, true, false, true, Some(-61))
    );
    // No alias: the device's own name; neither: the address. No address: not a device.
    let d = device_of(&props(&[
        ("Address", Value::from("11:22")),
        ("Name", Value::from("Mouse")),
        ("Alias", Value::from("")),
    ]))
    .unwrap();
    assert_eq!(d.name, "Mouse");
    assert_eq!(device_of(&props(&[("Address", Value::from("11:22"))])).unwrap().name, "11:22");
    assert!(device_of(&props(&[("Name", Value::from("x"))])).is_none());
}
