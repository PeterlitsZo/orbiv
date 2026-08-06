use std::error::Error;
use std::fmt::{Debug, Display};

pub struct OrbitError {
    /// The kind of error that occurred.
    kind: OrbitErrorKind,
    /// The context of the error, if any.
    ///
    /// It should be like a stack trace. Like "do something: do something else".
    context: Option<anyhow::Error>,
    /// The message of the error.
    message: String,
    /// The source of the error, if any.
    source: Option<anyhow::Error>,
}

impl OrbitError {
    fn new<T>(kind: OrbitErrorKind, message: T) -> Self
    where
        T: Into<String>,
    {
        Self {
            kind,
            context: None,
            message: message.into(),
            source: None,
        }
    }

    pub fn internal<T>(message: T) -> Self
    where
        T: Into<String>,
    {
        Self::new(OrbitErrorKind::Internal, message)
    }

    pub fn unimplemented<T>(message: T) -> Self
    where
        T: Into<String>,
    {
        Self::new(OrbitErrorKind::Unimplemented, message)
    }

    pub fn bad_argument<T>(message: T) -> Self
    where
        T: Into<String>,
    {
        Self::new(OrbitErrorKind::BadArgument, message)
    }

    pub fn has_failed_migration<T>(message: T) -> Self
    where
        T: Into<String>,
    {
        Self::new(OrbitErrorKind::HasFailedMigration, message)
    }

    pub fn invalid_migration<T>(message: T) -> Self
    where
        T: Into<String>,
    {
        Self::new(OrbitErrorKind::InvalidMigration, message)
    }
}

impl OrbitError {
    /// Get the kind of error that occurred.
    pub fn kind(&self) -> &OrbitErrorKind {
        &self.kind
    }
}

impl OrbitError {
    /// Set the source of the error.
    ///
    /// If the source is already set, it will be replaced.
    pub fn source<E>(mut self, source: E) -> Self
    where
        E: Into<anyhow::Error>,
    {
        self.source = Some(source.into());
        self
    }

    /// Set the context of the error.
    ///
    /// If the context is already set, it will be prepended with the new
    /// message.
    pub fn context<T>(self, message: T) -> Self
    where
        T: Into<String>,
    {
        let source = match self.context {
            Some(source) => source.context(message.into()),
            None => anyhow::anyhow!(message.into()),
        };
        Self {
            context: Some(source),
            ..self
        }
    }
}

impl Display for OrbitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.context {
            Some(ref context) => write!(f, "({:?}) {:?}: {}", self.kind, context, self.message),
            None => write!(f, "({:?}) {}", self.kind, self.message),
        }?;
        match self.source {
            Some(ref source) => write!(f, " Source: {:?}", source),
            None => Ok(()),
        }
    }
}

impl Debug for OrbitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}

impl Error for OrbitError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.context.as_ref().map(|source| source.as_ref())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrbitErrorKind {
    /// An internal error occurred.
    Internal,
    /// Not implemented.
    Unimplemented,
    /// The argument was invalid.
    BadArgument,
    /// Has failed migration.
    HasFailedMigration,
    /// Invalid migration.
    InvalidMigration,
}

pub type OrbitResult<T> = Result<T, OrbitError>;
