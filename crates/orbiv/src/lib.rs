#![doc = include_str!("../README.md")]

extern crate self as orbiv;

#[doc(hidden)]
pub mod __private {
    pub use async_trait::async_trait;
}

pub use orbiv_macros::migration;

pub use error::{OrbivError, OrbivErrorKind, OrbivResult};
pub use migration::Migration;
pub use migrator::{Migrator, MigratorBuilder, MigratorSteps};
pub use migrator_source::{MigrationRecord, MigratorSource};

mod error;
mod migration;
mod migrator;
mod migrator_source;
