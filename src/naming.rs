pub fn append_missing_keys(src: &str, key1: &str, key2: &str) -> String {
    let mut parts: Vec<String> = src.split(':').map(ToOwned::to_owned).collect();

    if let Some(first) = parts.get_mut(0)
        && !first.is_empty()
        && !first.contains('=')
    {
        *first = format!("{key1}{first}");
    }

    if let Some(second) = parts.get_mut(1)
        && !second.is_empty()
        && !second.contains('=')
    {
        *second = format!("{key2}{second}");
    }

    parts.join(":")
}

pub fn is_integer(src: &str) -> bool {
    !src.is_empty() && src.bytes().all(|b| b.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepends_missing_keys() {
        assert_eq!(
            append_missing_keys("Foo:Bar", "album=", "album_artist="),
            "album=Foo:album_artist=Bar"
        );
    }

    #[test]
    fn keeps_existing_keys() {
        assert_eq!(
            append_missing_keys("album=Foo:artist=Bar", "album=", "album_artist="),
            "album=Foo:artist=Bar"
        );
    }

    #[test]
    fn integer_check() {
        assert!(is_integer("123"));
        assert!(!is_integer("12a"));
        assert!(!is_integer(""));
    }
}
