//! Error type shared by the store and the CLI. Maps to the DESIGN.md exit codes.

use crate::model::ModelError;

#[derive(Debug, thiserror::Error)]
pub enum FbError {
    #[error("{0}")]
    Usage(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    ClaimHeld(String),
    #[error("{0}")]
    NotReady(String),
    #[error("{0}")]
    BadTransition(String),
    #[error("{0}")]
    DepCycle(String),
    #[error("{0}")]
    HumanOnly(String),
    #[error("{0}")]
    NotClaimant(String),
    #[error("database schema v{found} is newer than this fb supports (v{supported}); upgrade fb")]
    SchemaAhead { found: i64, supported: i64 },
    #[error("database is locked; retry in a moment")]
    DbLocked,
    #[error("{0}")]
    Store(String),
}

pub type Result<T> = std::result::Result<T, FbError>;

impl FbError {
    pub fn code(&self) -> &'static str {
        match self {
            FbError::Usage(_) => "USAGE",
            FbError::NotFound(_) => "NOT_FOUND",
            FbError::ClaimHeld(_) => "CLAIM_HELD",
            FbError::NotReady(_) => "NOT_READY",
            FbError::BadTransition(_) => "BAD_TRANSITION",
            FbError::DepCycle(_) => "DEP_CYCLE",
            FbError::HumanOnly(_) => "HUMAN_ONLY",
            FbError::NotClaimant(_) => "NOT_CLAIMANT",
            FbError::SchemaAhead { .. } => "SCHEMA_AHEAD",
            FbError::DbLocked => "DB_LOCKED",
            FbError::Store(_) => "STORE_ERROR",
        }
    }

    pub fn exit_code(&self) -> i32 {
        match self {
            FbError::Usage(_) => 1,
            FbError::NotFound(_) => 2,
            FbError::ClaimHeld(_)
            | FbError::NotReady(_)
            | FbError::BadTransition(_)
            | FbError::DepCycle(_) => 3,
            FbError::HumanOnly(_) | FbError::NotClaimant(_) => 4,
            FbError::SchemaAhead { .. } | FbError::DbLocked | FbError::Store(_) => 5,
        }
    }
}

impl From<ModelError> for FbError {
    fn from(e: ModelError) -> Self {
        match e {
            ModelError::HumanOnly(_) => FbError::HumanOnly(e.to_string()),
            _ => FbError::BadTransition(e.to_string()),
        }
    }
}

impl From<rusqlite::Error> for FbError {
    fn from(e: rusqlite::Error) -> Self {
        use rusqlite::ErrorCode::*;
        if let rusqlite::Error::SqliteFailure(f, msg) = &e {
            match f.code {
                DatabaseBusy | DatabaseLocked => return FbError::DbLocked,
                ConstraintViolation
                    if msg.as_deref().is_some_and(|m| m.contains("claims.task_id")) =>
                {
                    return FbError::ClaimHeld("task already has an active claim".into());
                }
                _ => {}
            }
        }
        FbError::Store(e.to_string())
    }
}

impl From<std::io::Error> for FbError {
    fn from(e: std::io::Error) -> Self {
        FbError::Store(e.to_string())
    }
}
