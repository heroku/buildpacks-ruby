use libcnb::Env;
use std::{ffi::OsString, fmt::Display};

/// Takes a list and turns it into a sentence structure
///
/// ```rust
/// use commons::display::SentenceList;
///
/// let actual =  SentenceList {
///     list: &["raindrops", "roses", "whiskers", "kittens"],
///     ..SentenceList::default()
/// }.to_string();
/// let expected = String::from("raindrops, roses, whiskers, and kittens");
/// assert_eq!(expected, actual);
/// ```
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SentenceList<'a, L: AsRef<str>> {
    pub list: &'a [L],
    pub on_empty: String,
    pub join_with: String,
}

impl<'a, L: AsRef<str>> SentenceList<'a, L> {
    pub fn new(list: &'a [L]) -> Self {
        Self {
            list,
            ..SentenceList::default()
        }
    }

    #[must_use]
    pub fn on_empty(mut self, string: String) -> Self {
        self.on_empty = string;
        self
    }

    #[must_use]
    pub fn join_with(mut self, string: String) -> Self {
        self.join_with = string;
        self
    }

    #[must_use]
    pub fn empty_str(mut self, str: &str) -> Self {
        self.on_empty = String::from(str);
        self
    }

    #[must_use]
    pub fn join_str(mut self, str: &str) -> Self {
        self.join_with = String::from(str);
        self
    }
}

impl<L: AsRef<str>> Default for SentenceList<'_, L> {
    fn default() -> Self {
        Self {
            list: Default::default(),
            on_empty: String::from("empty"),
            join_with: String::from("and"),
        }
    }
}

impl<L: AsRef<str>> Display for SentenceList<'_, L> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let SentenceList {
            list,
            on_empty,
            join_with: join_word,
        } = self;

        let total = list.len();

        if total == 0 {
            f.write_str(on_empty)?;
        } else {
            let mut count = 0;
            for item in self.list {
                count += 1;
                let item = item.as_ref();
                match sentence_list_item(total, count) {
                    List::First => f.write_str(item)?,
                    List::Item => {
                        f.write_fmt(format_args!(", {item}"))?;
                    }
                    List::LastItemAnd => {
                        f.write_fmt(format_args!(" {join_word} {item}"))?;
                    }
                    List::LastItemAndComma => {
                        f.write_fmt(format_args!(", {join_word} {item}"))?;
                    }
                }
            }
        }
        Ok(())
    }
}

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

/// Takes a list and turns it into a sentence structure
///
/// ```rust
/// use commons::display::list_to_sentence;
///
/// let actual = list_to_sentence(&["raindrops", "roses", "whiskers", "kittens"]);
/// let expected = String::from("raindrops, roses, whiskers, and kittens");
/// assert_eq!(expected, actual);
/// ```
///
/// When an empty list is used it will emit: "empty" by default. To configure
/// use the `SentenceList` struct directly
#[must_use]
pub fn list_to_sentence(list: &[impl AsRef<str>]) -> String {
    SentenceList {
        list,
        ..SentenceList::default()
    }
    .to_string()
}

#[derive(Debug)]
enum List {
    First,
    Item,
    LastItemAnd,
    LastItemAndComma,
}

fn sentence_list_item(total: usize, count: usize) -> List {
    match (count == 1, count == total, count == 2) {
        (true, _, _) => List::First,
        (_, false, _) => List::Item,
        (_, true, true) => List::LastItemAnd,
        (_, true, false) => List::LastItemAndComma,
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_sentence_list() {
        let actual = list_to_sentence(&Vec::<String>::new());
        let expected = String::from("empty");
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
