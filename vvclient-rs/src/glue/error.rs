
// https://github.com/mozilla/uniffi-rs/blob/main/docs/manual/src/types/errors.md


#[derive(uniffi::Object)]
#[derive(Debug, thiserror::Error)]
#[error("{e:?}")] // default message is from anyhow.
pub struct Error {
    e: anyhow::Error,
}

impl Error {
    pub fn inner(&self) -> &anyhow::Error {
        &self.e
    }
}

#[uniffi::export]
impl Error {
    fn message(&self) -> String {
        format!("{}", self.e)
    }

    fn debug(&self) -> String {
        format!("{:?}", self.e)
    }
}

impl From<anyhow::Error> for Error {
    fn from(e: anyhow::Error) -> Self {
        Self { e }
    }
}

impl From<ForeignError> for Error {
    fn from(value: ForeignError) -> Self {
        Self {
            e: anyhow::Error::msg(value.to_string()) 
        }
    }
}

#[derive(uniffi::Error)]
#[derive(Debug, thiserror::Error)]
pub enum ForeignError {
    #[error("Busy")]
    Busy,

    #[error("Unexpected error [{0}]")]
    Unexpected(String),
}

impl From<uniffi::UnexpectedUniFFICallbackError> for ForeignError {
    fn from(value: uniffi::UnexpectedUniFFICallbackError) -> Self {
        Self::Unexpected(value.reason)
    }
}
