use libcnb::Env;
use std::ffi::OsString;

/// Transforms an env into a human readable string
///
/// Keys are sorted for consistent comparison
#[must_use]
pub fn env_to_sorted_string(env: &Env) -> String {
    let mut env = env
        .into_iter()
        .map(|(a, b)| (a.clone(), b.clone()))
        .collect::<Vec<(OsString, OsString)>>();

    env.sort_by(|(a, _), (b, _)| a.cmp(b));

    env.iter()
        .map(|(key, value)| {
            let mut out = OsString::new();
            out.push(key);
            out.push(OsString::from("="));
            out.push(value);
            out.to_string_lossy() // UTF-8 values see no degradation, otherwise we should be comparing equivalent strings.
                .to_string()
        })
        .collect::<Vec<String>>()
        .join("\n")
}

/// When input is empty call function, otherwise return `list_to_sentence`
///
/// ```rust
/// use commons::display::list_to_sentence_or_else;
///
/// let mut input = vec![String::from("hello"), String::from("there")];
/// let actual = list_to_sentence_or_else(&input, || String::from("<empty>"));
///
/// assert_eq!(String::from("hello and there"), actual);
///
/// input.retain(|_| false);
/// let actual = list_to_sentence_or_else(&input, || String::from("<empty>"));
///
/// assert_eq!(String::from("<empty>"), actual);
/// ```
pub fn list_to_sentence_or_else(list: &[impl AsRef<str>], f: impl Fn() -> String) -> String {
    if list.is_empty() {
        f()
    } else {
        list_to_sentence(list)
    }
}

/// Takes a list and turns it into a sentence structure
///
/// ```rust
/// use commons::display::list_to_sentence;
///
/// let actual = list_to_sentence(&["raindrops", "roses", "whiskers", "kittens"]);
/// let expected = String::from("raindrops, roses, whiskers, and kittens");
/// assert_eq!(expected, actual);
/// ```
#[must_use]
pub fn list_to_sentence(list: &[impl AsRef<str>]) -> String {
    let total = list.len();
    let mut count = 0;
    let mut string = String::new();
    for out in list {
        count += 1;
        let out = out.as_ref();
        match sentence_list_item(total, count) {
            SentenceList::First => string.push_str(out),
            SentenceList::Item => {
                string.push_str(&format!(", {out}"));
            }

            SentenceList::LastItemAnd => {
                string.push_str(&format!(" and {out}"));
            }

            SentenceList::LastItemAndComma => {
                string.push_str(&format!(", and {out}"));
            }
        }
    }
    string
}

#[derive(Debug)]
enum SentenceList {
    First,
    Item,
    LastItemAnd,
    LastItemAndComma,
}

fn sentence_list_item(total: usize, count: usize) -> SentenceList {
    match (count == 1, count == total, count == 2) {
        (true, _, _) => SentenceList::First,
        (_, false, _) => SentenceList::Item,
        (_, true, true) => SentenceList::LastItemAnd,
        (_, true, false) => SentenceList::LastItemAndComma,
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_sentence_list() {
        let actual = list_to_sentence(&Vec::<String>::new());
        let expected = String::new();
        assert_eq!(expected, actual);

        let actual = list_to_sentence(&["me"]);
        let expected = String::from("me");
        assert_eq!(expected, actual);

        let actual = list_to_sentence(&["me", "myself"]);
        let expected = String::from("me and myself");
        assert_eq!(expected, actual);

        let actual = list_to_sentence(&["me", "myself", "I"]);
        let expected = String::from("me, myself, and I");
        assert_eq!(expected, actual);

        let actual = list_to_sentence(&["raindrops", "roses", "whiskers", "kittens"]);
        let expected = String::from("raindrops, roses, whiskers, and kittens");
        assert_eq!(expected, actual);
    }
}
