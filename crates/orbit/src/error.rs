use std::error::Error;
use std::fmt::{Debug, Display};

pub struct MigrateError {
    kind: MigrateErrorKind,
    source: Option<anyhow::Error>,
    message: String,
}

impl MigrateError {
    pub fn internal<T>(message: T) -> Self
    where
        T: Into<String>,
    {
        Self {
            kind: MigrateErrorKind::Internal,
            source: None,
            message: message.into(),
        }
    }

    pub fn unimplemented<T>(message: T) -> Self
    where
        T: Into<String>,
    {
        Self {
            kind: MigrateErrorKind::Unimplemented,
            source: None,
            message: message.into(),
        }
    }

    pub fn missing_handler() -> Self {
        Self {
            kind: MigrateErrorKind::MissingHandler,
            source: None,
            message: "handler is required".to_owned(),
        }
    }

    pub fn missing_migrations() -> Self {
        Self {
            kind: MigrateErrorKind::MissingMigrations,
            source: None,
            message: "migrations are required".to_owned(),
        }
    }

    pub fn kind(&self) -> &MigrateErrorKind {
        &self.kind
    }
}

impl MigrateError {
    pub fn with_source<E>(mut self, source: E) -> Self
    where
        E: Into<anyhow::Error>,
    {
        self.source = Some(source.into());
        self
    }
}

impl Display for MigrateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({:?}) {}", self.kind, self.message)
    }
}

impl Debug for MigrateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.source {
            Some(ref source) => write!(f, "{}: {source:?}", self),
            None => Display::fmt(self, f),
        }
    }
}

impl Error for MigrateError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source.as_ref().map(|source| source.as_ref())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MigrateErrorKind {
    Internal,
    Unimplemented,
    MissingHandler,
    MissingMigrations,
}

pub type MigrateResult<T> = Result<T, MigrateError>;
