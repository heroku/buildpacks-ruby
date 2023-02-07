/// Allows
#[derive(Debug)]
pub(crate) struct IoErrorAnnotation {
    source: std::io::Error,
    annotation: String,
}

impl IoErrorAnnotation {
    pub(crate) fn new(source: std::io::Error, annotation: String) -> Self {
        Self { source, annotation }
    }

    pub(crate) fn into_io_error(self) -> std::io::Error {
        std::io::Error::new(self.source.kind(), self)
    }
}

impl std::fmt::Display for IoErrorAnnotation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{}", self.source)?;
        f.write_str(&self.annotation)?;
        Ok(())
    }
}

impl std::error::Error for IoErrorAnnotation {
    fn cause(&self) -> Option<&dyn std::error::Error> {
        self.source()
    }

    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        let io_error = std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Zoinks, I couldn't find that ",
        );
        let wrapped: std::io::Error = IoErrorAnnotation::new(
            io_error,
            String::from("Debug details: it's just a villan in a mask"),
        )
        .into_io_error();

        assert_eq!(
            "Zoinks, I couldn't find that \nDebug details: it's just a villan in a mask",
            &format!("{wrapped}")
        );
    }
}
