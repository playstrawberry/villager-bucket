pub const FILES: [(&str, &str); 4] = [
    ("en", include_str!("../lang/en.lang")),
    ("fr", include_str!("../lang/fr.lang")),
    ("es", include_str!("../lang/es.lang")),
    ("tr", include_str!("../lang/tr.lang")),
];

pub fn text(locale: &str, key: &str) -> String {
    let language = locale.get(..2).unwrap_or("").to_ascii_lowercase();
    let lookup = |contents: &str| {
        contents
            .lines()
            .filter_map(|line| line.split_once('='))
            .find_map(|(candidate, value)| (candidate == key).then(|| value.to_owned()))
    };
    FILES
        .iter()
        .find(|(code, _)| *code == language)
        .and_then(|(_, file)| lookup(file))
        .or_else(|| lookup(FILES[0].1))
        .unwrap_or_else(|| key.to_owned())
}
