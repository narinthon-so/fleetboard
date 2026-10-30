//! fleetboard core library. See DESIGN.md.

pub mod adapters; // M2 runtime adapters (none, paseo)
pub mod config; // config.toml loading and durations
pub mod gitinfo; // project key, instance, evidence — owner: agy (branch m0-identity-git)
pub mod identity; // caller identity resolution (D3) — owner: agy (branch m0-identity-git)
pub mod model; // status machine, derived states — owner: claude (branch m0-core)
pub mod prime; // fb prime text — owner: claude (branch m0-core)
pub mod server; // M1 axum server, auth, read API, SSE
pub mod store; // SQLite schema, migrations, Backend trait — owner: claude (branch m0-core)
