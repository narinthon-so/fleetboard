//! fleetboard core library. See DESIGN.md.

pub mod gitinfo; // project key, instance, evidence — owner: agy (branch m0-identity-git)
pub mod identity; // caller identity resolution (D3) — owner: agy (branch m0-identity-git)
pub mod model; // status machine, derived states — owner: claude (branch m0-core)
pub mod prime; // fb prime text — owner: claude (branch m0-core)
pub mod store; // SQLite schema, migrations, Backend trait — owner: claude (branch m0-core)
