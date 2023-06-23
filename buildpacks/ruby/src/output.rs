use indoc::formatdoc;

const RESET: &'static str = r#"\033[0m"#;
const RED: &'static str = r#"\e[31m"#;
const YELLOW: &'static str = r#"\e[11m"#;
const BLUE: &'static str = r#"\e[34m"#;
const BOLD_PURPLE: &'static str = r#"\e[1;35m"#; // magenta
const NOCOLOR: &'static str = r#"\033[0m\033[0m"#; //differentiate between color clear and explicit no color
const NOCOLOR_TMP: &'static str = r#"🙈🙈🙈"#;

pub fn value(contents: impl AsRef<str>) -> String {
    let contents = colorize(BLUE, contents.as_ref());
    format!("`{contents}`")
}

pub fn details(contents: impl AsRef<str>) -> String {
    let contents = contents.as_ref();
    format!("({contents})")
}

pub fn header(contents: impl AsRef<str>) -> String {
    let contents = contents.as_ref();
    colorize(BOLD_PURPLE, format!("# {contents}"))
}

pub struct ErrorContents {
    header: String,
    body: String,
    url: Option<String>,
}

pub fn lookatme(
    color: &str,
    noun: impl AsRef<str>,
    header: impl AsRef<str>,
    body: impl AsRef<str>,
    url: Option<String>,
) -> String {
    let noun = noun.as_ref();
    let header = header.as_ref();
    let body = help_url(body, url);
    bangify(formatdoc! {"
        {noun} {header}

        {body}
    "})
}

pub fn error(header: impl AsRef<str>, body: impl AsRef<str>, url: Option<String>) -> String {
    let header = header.as_ref();
    let body = body.as_ref();

    lookatme(RED, "ERROR:", header, body, url)
}

pub fn warning(header: impl AsRef<str>, body: impl AsRef<str>, url: Option<String>) -> String {
    let header = header.as_ref();
    let body = body.as_ref();

    lookatme(YELLOW, "WARNING:", header, body, url)
}

pub fn important(header: impl AsRef<str>, body: impl AsRef<str>, url: Option<String>) -> String {
    let header = header.as_ref();
    let body = body.as_ref();

    lookatme(BLUE, "", header, body, url)
}

fn help_url(body: impl AsRef<str>, url: Option<String>) -> String {
    let body = body.as_ref();

    if let Some(url) = url {
        let url = colorize(NOCOLOR, url);
        formatdoc! {"
            {body}

            For more information, refer to the following documentation:
            {url}
        "}
    } else {
        format!("{body}")
    }
}

fn bangify(body: impl AsRef<str>) -> String {
    body.as_ref()
        .split("\n")
        .map(|section| format!("! {section}"))
        .collect::<Vec<String>>()
        .join("\n")
}

/// Colorizes a body while preserving existing color/reset combinations and clearing before newlines
///
/// Colors with newlines are a problem since the contents stream to git which prepends `remote:` before the libcnb_test
/// if we don't clear, then we will colorize output that isn't ours
///
/// Explicitly uncolored output is handled by a hacky process of treating two color clears as a special cases
fn colorize(color: &str, body: impl AsRef<str>) -> String {
    body.as_ref()
        .split("\n")
        .map(|section| section.replace(NOCOLOR, NOCOLOR_TMP)) // Explicit no-color hack so it's not cleaned up by accident
        .map(|section| section.replace(RESET, &format!("{RESET}{color}"))) // Handles nested color
        .map(|section| format!("{color}{section}{RESET}")) // Clear after every newline
        .map(|section| section.replace(&format!("{RESET}{color}{RESET}"), RESET)) // Reduce useless color
        .map(|section| section.replace(&format!("{color}{color}"), color)) // Reduce useless color
        .map(|section| section.replace(NOCOLOR_TMP, NOCOLOR)) // Explicit no-color repair
        .collect::<Vec<String>>()
        .join("\n")
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn lol() {
        println!("{}", error("ohno", "nope", None));
    }

    #[test]
    fn handles_explicitly_removed_colors() {
        let nested = colorize(NOCOLOR, "nested");

        let out = colorize(RED, format!("hello {nested} color"));
        let expected = format!("{RED}hello {NOCOLOR}nested{RESET}{RED} color{RESET}");

        assert_eq!(expected, out);
    }

    #[test]
    fn handles_nested_colors() {
        let nested = colorize(BLUE, "nested");

        let out = colorize(RED, format!("hello {nested} color"));
        let expected = format!("{RED}hello {BLUE}nested{RESET}{RED} color{RESET}");

        assert_eq!(expected, out);
    }

    #[test]
    fn splits_newlines() {
        let out = colorize(RED, "hello\nworld");
        let expected = r#"\e[31mhello\033[0m
\e[31mworld\033[0m"#;

        assert_eq!(expected, &out);
    }

    #[test]
    fn simple_case() {
        let out = colorize(RED, "hello world");
        assert_eq!(r#"\e[31mhello world\033[0m"#, &out);
    }
}
