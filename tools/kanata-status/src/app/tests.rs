use super::*;

const KBD: &str = "(defsrc\n a s x\n spc)\n(deflayermap main\n a lctl\n s lsft\n x C-x\n ___ XX)\n(deflayermap ins\n ___ _)\n";

fn app() -> App {
    App::new(Config::parse(KBD).unwrap(), "x".into())
}

fn state(layer: &str, keys: &[&str]) -> Event {
    Event::State(State {
        layer: layer.into(),
        keys: keys.iter().map(|k| k.to_string()).collect(),
        kanata: true,
        ..State::default()
    })
}

#[test]
fn the_most_specific_key_is_lit() {
    let mut app = app();
    app.apply(state("main", &["lctl", "x"]));
    assert_eq!(app.lit("main"), vec!["x"]);
    app.apply(state("main", &["lctl"]));
    assert_eq!(app.lit("main"), vec!["a"]);
}

#[test]
fn keys_pass_through_in_the_insert_layer() {
    let mut app = app();
    app.apply(state("ins", &["a", "s"]));
    assert_eq!(app.lit("ins"), vec!["a", "s"]);
}

#[test]
fn history_records_presses_and_layer_changes() {
    let mut app = app();
    app.apply(state("main", &[]));
    app.apply(state("main", &["a"]));
    app.apply(state("main", &[]));
    app.apply(state("ins", &[]));
    let names: Vec<String> = app
        .history
        .iter()
        .map(|i| match i {
            Item::Press(k) => k.clone(),
            Item::Layer(l) => format!("[{l}]"),
        })
        .collect();
    assert_eq!(names, ["[main]", "a", "[ins]"]);
}

#[test]
fn renders_without_panicking_on_small_screens() {
    let mut app = app();
    app.apply(state("main", &["lctl"]));
    for (w, h) in [(120, 40), (60, 20), (10, 5), (1, 1)] {
        let area = Rect::new(0, 0, w, h);
        let mut buf = Buffer::empty(area);
        app.render(area, &mut buf);
    }
}
