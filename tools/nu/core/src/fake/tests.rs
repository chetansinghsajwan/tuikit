use super::*;

#[test]
fn connect_needs_a_password_for_new_protected_networks_and_saves_it() {
    let f = Fake::new();
    assert_eq!(f.connect("neighbor", None), Err(Error("a password is needed".into())));
    assert!(f.connect("neighbor", Some("short")).is_err());
    f.connect("neighbor", Some("longenough")).unwrap();
    assert_eq!(f.status().unwrap().active.as_deref(), Some("neighbor"));
    assert!(f.networks().unwrap()[0].known);
    // A saved or open network connects without one, and the previous one is dropped.
    f.connect("cafe", None).unwrap();
    assert_eq!(f.networks().unwrap().iter().filter(|n| n.active).count(), 1);
    assert!(f.connect("office", None).is_err());
    assert!(f.connect("nowhere", None).is_err());
}

#[test]
fn disconnect_forget_and_power() {
    let f = Fake::new();
    f.connect("home", None).unwrap();
    f.disconnect().unwrap();
    assert_eq!(f.status().unwrap().active, None);
    f.forget("home").unwrap();
    assert!(f.forget("home").is_err());
    f.set_enabled(false).unwrap();
    assert!(f.networks().unwrap().is_empty() && !f.status().unwrap().enabled);
    assert_eq!(f.log().last().map(String::as_str), Some("enabled false"));
}
