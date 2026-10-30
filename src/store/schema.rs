//! Schema and migrations. `PRAGMA user_version` is the number of applied migrations.

/// Migrations in order; index + 1 is the schema version they produce.
pub const MIGRATIONS: &[&str] = &[V1, V2];

pub const SCHEMA_VERSION: i64 = MIGRATIONS.len() as i64;

const V1: &str = r#"
CREATE TABLE projects (
  id          INTEGER PRIMARY KEY,
  key         TEXT NOT NULL UNIQUE,
  alias       TEXT,
  created_at  TEXT NOT NULL
);

CREATE TABLE tickets (
  id          INTEGER PRIMARY KEY,
  system      TEXT NOT NULL,
  key         TEXT NOT NULL,
  title       TEXT,
  url         TEXT,
  created_at  TEXT NOT NULL,
  UNIQUE (system, key)
);

CREATE TABLE actors (
  id              INTEGER PRIMARY KEY,
  kind            TEXT NOT NULL CHECK (kind IN ('human','agent')),
  provider        TEXT,
  external_id     TEXT NOT NULL,
  label           TEXT,
  trusted         INTEGER NOT NULL DEFAULT 0,
  first_seen      TEXT NOT NULL,
  last_seen       TEXT NOT NULL,
  UNIQUE (kind, provider, external_id)
);

CREATE TABLE tasks (
  id                    INTEGER PRIMARY KEY,
  project_id            INTEGER REFERENCES projects(id),
  ticket_id             INTEGER REFERENCES tickets(id),
  title                 TEXT NOT NULL,
  goal                  TEXT,
  acceptance            TEXT,
  plan                  TEXT,
  result                TEXT,
  kind                  TEXT NOT NULL DEFAULT 'code' CHECK (kind IN ('code','note')),
  status                TEXT NOT NULL CHECK (status IN
                          ('draft','todo','in_progress','review','done','dropped')),
  priority              INTEGER NOT NULL DEFAULT 2 CHECK (priority BETWEEN 0 AND 3),
  needs_plan_approval   INTEGER NOT NULL DEFAULT 0,
  plan_approved_at      TEXT,
  created_by            INTEGER NOT NULL REFERENCES actors(id),
  created_at            TEXT NOT NULL,
  updated_at            TEXT NOT NULL
);

CREATE TABLE deps (
  task_id     INTEGER NOT NULL REFERENCES tasks(id),
  blocker_id  INTEGER NOT NULL REFERENCES tasks(id),
  PRIMARY KEY (task_id, blocker_id),
  CHECK (task_id <> blocker_id)
);

CREATE TABLE claims (
  id              INTEGER PRIMARY KEY,
  task_id         INTEGER NOT NULL REFERENCES tasks(id),
  actor_id        INTEGER NOT NULL REFERENCES actors(id),
  host            TEXT NOT NULL,
  cwd             TEXT NOT NULL,
  branch          TEXT,
  base_commit     TEXT,
  workspace_id    TEXT,
  claimed_at      TEXT NOT NULL,
  last_activity   TEXT NOT NULL,
  released_at     TEXT,
  release_reason  TEXT
);
CREATE UNIQUE INDEX one_active_claim ON claims(task_id) WHERE released_at IS NULL;

CREATE TABLE notes (
  id          INTEGER PRIMARY KEY,
  task_id     INTEGER NOT NULL REFERENCES tasks(id),
  actor_id    INTEGER NOT NULL REFERENCES actors(id),
  kind        TEXT NOT NULL CHECK (kind IN ('handoff','comment','finding')),
  body        TEXT NOT NULL,
  created_at  TEXT NOT NULL
);

CREATE TABLE evidence_snapshots (
  claim_id     INTEGER PRIMARY KEY REFERENCES claims(id),
  data         TEXT NOT NULL,
  computed_at  TEXT NOT NULL
);

CREATE TABLE events (
  id          INTEGER PRIMARY KEY,
  task_id     INTEGER REFERENCES tasks(id),
  actor_id    INTEGER NOT NULL REFERENCES actors(id),
  type        TEXT NOT NULL,
  data        TEXT,
  pid         INTEGER NOT NULL,
  ppid        INTEGER NOT NULL,
  proc_chain  TEXT,
  created_at  TEXT NOT NULL
);

INSERT INTO actors (kind, provider, external_id, label, trusted, first_seen, last_seen)
VALUES ('human', NULL, 'human', 'human', 0,
        strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), strftime('%Y-%m-%dT%H:%M:%SZ', 'now'));
"#;

const V2: &str = r#"
CREATE TABLE delegations (
  id          INTEGER PRIMARY KEY,
  actor_id    INTEGER NOT NULL REFERENCES actors(id),
  action      TEXT NOT NULL CHECK (action IN ('promote','approve','verify','reject','release','steal','drop','reopen')),
  created_at  TEXT NOT NULL,
  UNIQUE (actor_id, action)
);
"#;
