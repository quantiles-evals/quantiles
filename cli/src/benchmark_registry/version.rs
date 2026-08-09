use anyhow::{Result, bail};

#[derive(Clone, Debug)]
pub struct Version(String);

impl Version {
    pub fn new(ver: &str) -> Result<Self> {
        if ver.is_empty() {
            bail!("version must not be empty");
        }
        Ok(Self(ver.to_string()));
    }

    pub(crate) fn to_string(self) -> String {
        self.0.clone()
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_an_empty_version() {
        let error = Version::new("").unwrap_err();

        assert!(error.to_string().contains("version must not be empty"));
    }

    #[test]
    fn preserves_and_displays_a_nonempty_version() {
        let version = Version::new("v1.2.3").unwrap();

        assert_eq!(format!("{version}"), "v1.2.3");
        assert_eq!(version.to_string(), "v1.2.3");
    }

    #[test]
    fn clone_preserves_the_version() {
        let version = Version::new("release-42").unwrap();

        assert_eq!(version.clone().to_string(), version.to_string());
    }
}
