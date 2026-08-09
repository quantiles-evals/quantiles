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
