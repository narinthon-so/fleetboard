# fleetboard — HTTP API (M1–M2)

Status: draft (2026-10-30). Implemented by `fb serve`.

The HTTP server binds strictly to the loopback interface (`127.0.0.1`, default port 7878) and serves the REST and SSE API for the fleetboard web UI.

---

## Architecture & Security Model

### 1. Loopback Binding Only
`fb serve` binds only to `127.0.0.1:<port>` (IPv4 loopback), never `0.0.0.0` or public network interfaces.

### 2. One-Time Login Token
When `fb serve` starts, it generates a high-entropy 32-byte token using the OS CSPRNG (`getrandom`), encoded as a 64-character lowercase hex string in memory:
- The token is printed directly to stdout in the human operator's terminal.
- The token is **never** written to disk or persisted in logs or database files.
- The token can be used **only once**. As soon as `POST /api/login` exchanges it, the token is consumed and invalidated.
- Token validation during login is performed in constant time (`subtle::ConstantTimeEq`) to prevent timing attacks.

### 3. Session Cookie
Exchanging the one-time token sets an `HttpOnly`, `SameSite=Strict` cookie named `fb_session`.
- The cookie carries a `Max-Age` of 24 hours and the server rejects it once expired; after that a fresh one-time token (new `fb serve`) is required.
- All requests to `/api/*` (except `POST /api/login`) require a valid session cookie.
- Requests without a valid cookie receive `401 Unauthorized`.
- `fb serve` itself requires `--as-human` and refuses to start from a shell with agent markers, so an agent cannot stand up a server that grants human powers.

### 4. Write Protection (`X-Requested-With`)
To prevent eager agents or ambient requests from triggering state mutations through loopback:
- All state-modifying requests (`POST`, `PUT`, `DELETE`, `PATCH`) under `/api/*` require the header:
  ```http
  X-Requested-With: fleetboard
  ```
- Missing or invalid `X-Requested-With` headers receive `403 Forbidden`.

---

## Response Envelope

All API endpoints follow the standard fleetboard JSON envelope:

### Success
```json
{
  "v": 1,
  "ok": true,
  "data": { ... }
}
```

### Error
```json
{
  "v": 1,
  "ok": false,
  "error": {
    "code": "ERROR_CODE",
    "message": "Human-readable description"
  }
}
```

### HTTP Status Codes & Error Codes
| HTTP Status | Error Code | Meaning |
|---|---|---|
| `200 OK` | — | Request succeeded |
| `400 Bad Request` | `USAGE` | Malformed request or invalid payload |
| `401 Unauthorized` | `UNAUTHORIZED` | Missing or invalid session cookie, or invalid login token |
| `403 Forbidden` | `FORBIDDEN` | Missing or invalid `X-Requested-With: fleetboard` on writes |
| `404 Not Found` | `NOT_FOUND` | Task or resource does not exist |
| `409 Conflict` | `CLAIM_HELD`, `NOT_READY`, `BAD_TRANSITION`, `DEP_CYCLE` | Invalid state transition or constraint conflict |
| `500 Internal Error` | `INTERNAL` | Unexpected internal server error |

---

## Authentication Endpoints

### `POST /api/login`
Exchanges the one-time token printed in the terminal for a session cookie.

**Request:**
```http
POST /api/login HTTP/1.1
Content-Type: application/json

{
  "token": "a1b2c3d4e5f6..."
}
```

**Response (`200 OK`):**
```http
HTTP/1.1 200 OK
Set-Cookie: fb_session=s3ss10n_1d...; HttpOnly; SameSite=Strict; Path=/
Content-Type: application/json

{
  "v": 1,
  "ok": true,
  "data": {
    "message": "logged in"
  }
}
```

**Errors:**
- `401 Unauthorized`: Token is incorrect, empty, or has already been used.
  ```json
  {
    "v": 1,
    "ok": false,
    "error": {
      "code": "UNAUTHORIZED",
      "message": "invalid or expired token"
    }
  }
  ```

---

### `POST /api/logout`
Invalidates the current session and clears the cookie.

**Request:**
```http
POST /api/logout HTTP/1.1
Cookie: fb_session=...
X-Requested-With: fleetboard
```

**Response (`200 OK`):**
```http
HTTP/1.1 200 OK
Set-Cookie: fb_session=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0
Content-Type: application/json

{
  "v": 1,
  "ok": true,
  "data": {
    "message": "logged out"
  }
}
```

---

## Read Endpoints

### `GET /api/board`
Returns all board data needed to render columns, filters, and cards.

**Request:**
```http
GET /api/board HTTP/1.1
Cookie: fb_session=...
```

**Response (`200 OK`):**
```json
{
  "v": 1,
  "ok": true,
  "data": {
    "tasks": [
      {
        "id": "fb-12",
        "title": "Rate-limit public endpoints",
        "kind": "code",
        "status": "in_progress",
        "priority": 1,
        "project": {
          "key": "github.com/org/repo",
          "alias": null,
          "name": "repo"
        },
        "ticket": {
          "system": "jira",
          "key": "ABC-36",
          "title": "Hardening",
          "url": null
        },
        "goal": "Rate limit endpoints to protect from abuse",
        "acceptance": "- [ ] 429 after 100 req/min",
        "plan": "Add token bucket filter",
        "result": null,
        "needs_plan_approval": false,
        "plan_approved_at": null,
        "blocked_by": [],
        "claim": {
          "id": 1,
          "actor": {
            "id": 2,
            "kind": "agent",
            "provider": "codex",
            "external_id": "403254b",
            "label": "codex 403254b",
            "trusted": false
          },
          "host": "RPI",
          "cwd": "/home/user/code/repo",
          "branch": "task/fb-12",
          "base_commit": "abc1234",
          "workspace_id": null,
          "claimed_at": "2026-10-30T10:00:00Z",
          "last_activity": "2026-10-30T10:30:00Z",
          "released_at": null,
          "release_reason": null
        },
        "derived": {
          "ready": false,
          "blocked": false,
          "orphaned": false,
          "plan_pending": false,
          "stale": false,
          "no_evidence": false,
          "needs_me": false
        },
        "runtime": {
          "status": "running",
          "pending_permissions": 0
        },
        "runtime_status": "running",
        "pending_permissions": 0,
        "evidence": {
          "commits": 3,
          "computed_at": "2026-10-30T10:35:00Z",
          "diffstat": "1 file changed, 10 insertions(+)",
          "dirty": false,
          "pr": null,
          "reachable": true,
          "rebased": false,
          "source": "live"
        },
        "created_by": {
          "id": 1,
          "kind": "human",
          "provider": null,
          "external_id": "human",
          "label": "human",
          "trusted": false
        },
        "created_at": "2026-10-30T09:00:00Z",
        "updated_at": "2026-10-30T10:30:00Z"
      }
    ],
    "projects": [
      {
        "id": 1,
        "key": "github.com/org/repo",
        "alias": null,
        "name": "repo"
      }
    ],
    "instances": [
      {
        "host": "RPI",
        "cwd": "/home/user/code/repo",
        "branch": "task/fb-12",
        "workspace_id": null,
        "actor": {
          "id": 2,
          "kind": "agent",
          "provider": "codex",
          "external_id": "403254b",
          "label": "codex 403254b",
          "trusted": false
        },
        "project": {
          "id": 1,
          "key": "github.com/org/repo",
          "alias": null,
          "name": "repo"
        }
      }
    ],
    "actors": [
      {
        "id": 1,
        "kind": "human",
        "provider": null,
        "external_id": "human",
        "label": "human",
        "trusted": false
      },
      {
        "id": 2,
        "kind": "agent",
        "provider": "codex",
        "external_id": "403254b",
        "label": "codex 403254b",
        "trusted": false
      }
    ]
  }
}
```

---

### `GET /api/tasks/:id`
Returns full detail for a single task, including claims history, notes, live evidence, and the event audit log.

**Path Parameters:**
- `:id`: Task identifier. Accepts either format `fb-12` or numeric `12`.

**Request:**
```http
GET /api/tasks/fb-12 HTTP/1.1
Cookie: fb_session=...
```

**Response (`200 OK`):**
```json
{
  "v": 1,
  "ok": true,
  "data": {
    "task": {
      "id": "fb-12",
      "title": "Rate-limit public endpoints",
      "kind": "code",
      "status": "in_progress",
      "priority": 1,
      "project": {
        "key": "github.com/org/repo",
        "alias": null,
        "name": "repo"
      },
      "ticket": null,
      "goal": "Protect endpoints",
      "acceptance": "- [ ] 429 after 100 req/min",
      "plan": "Token bucket filter",
      "result": null,
      "needs_plan_approval": false,
      "plan_approved_at": null,
      "blocked_by": [],
      "claim": { ... },
      "derived": { ... },
      "evidence": {
        "commits": 3,
        "computed_at": "2026-10-30T10:35:00Z",
        "diffstat": "1 file changed, 10 insertions(+)",
        "dirty": false,
        "pr": null,
        "reachable": true,
        "rebased": false,
        "source": "live"
      },
      "claims": [
        {
          "id": 1,
          "actor": { ... },
          "host": "RPI",
          "cwd": "/home/user/code/repo",
          "branch": "task/fb-12",
          "base_commit": "abc1234",
          "workspace_id": null,
          "claimed_at": "2026-10-30T10:00:00Z",
          "last_activity": "2026-10-30T10:30:00Z",
          "released_at": null,
          "release_reason": null
        }
      ],
      "notes": [
        {
          "id": 1,
          "kind": "handoff",
          "actor": { ... },
          "body": "Token bucket implemented, writing integration tests.",
          "created_at": "2026-10-30T10:25:00Z"
        }
      ],
      "events": [
        {
          "id": 1,
          "task_id": "fb-12",
          "actor": { ... },
          "type": "created",
          "data": null,
          "pid": 1234,
          "ppid": 1230,
          "proc_chain": "bash < codex",
          "created_at": "2026-10-30T09:00:00Z",
          "on_behalf_of": null
        },
        {
          "id": 2,
          "task_id": "fb-12",
          "actor": { ... },
          "type": "claimed",
          "data": { "base_commit": "abc1234" },
          "pid": 1240,
          "ppid": 1230,
          "proc_chain": "bash < codex",
          "created_at": "2026-10-30T10:00:00Z",
          "on_behalf_of": null
        }
      ],
      "created_by": { ... },
      "created_at": "2026-10-30T09:00:00Z",
      "updated_at": "2026-10-30T10:30:00Z"
    }
  }
}
```

**Errors:**
- `404 Not Found`: Task does not exist.
  ```json
  {
    "v": 1,
    "ok": false,
    "error": {
      "code": "NOT_FOUND",
      "message": "task fb-999 not found"
    }
  }
  ```

---

### `GET /api/events`
Server-Sent Events (SSE) stream for live board updates.

**Request:**
```http
GET /api/events HTTP/1.1
Cookie: fb_session=...
Accept: text/event-stream
```

**Response (`200 OK`):**
```http
HTTP/1.1 200 OK
Content-Type: text/event-stream
Cache-Control: no-cache

data: {"type":"changed"}

: ping

data: {"type":"changed"}
```

**Mechanism:**
- A background worker polls SQLite `PRAGMA data_version` every ~500 ms on a persistent connection.
- When any process commits a write transaction to `board.db`, `PRAGMA data_version` increments and the server emits `data: {"type":"changed"}\n\n`.
- The frontend TanStack Query cache invalidates its board queries upon receiving this event.
- An automatic `: ping\n\n` keepalive is emitted every 15 seconds to keep the connection healthy.

---

## Human Action Endpoints (Writes)

All human-action endpoints act on behalf of the human operator, enforce the exact same store and transition rules as the CLI, and log audit events with `source="web"` and `proc_chain="web"`.

Every write endpoint requires:
1. `Cookie: fb_session=<session_id>`
2. `X-Requested-With: fleetboard` (returns `403 Forbidden` if missing or invalid)

Path parameter `:id` accepts either format `fb-12` or numeric `12`. Non-existent tasks return `404 Not Found`.

---

### `POST /api/tasks/:id/promote`
Promotes a draft task to `todo`. Equivalent to `fb promote <id>`.

**Request:**
```http
POST /api/tasks/fb-12/promote HTTP/1.1
Cookie: fb_session=...
X-Requested-With: fleetboard
```

**Response (`200 OK`):**
```json
{
  "v": 1,
  "ok": true,
  "data": {
    "task": {
      "id": "fb-12",
      "status": "todo",
      ...
    }
  }
}
```

**Errors:**
- `409 Conflict` (`BAD_TRANSITION`): Task is not in `draft` status.

---

### `POST /api/tasks/:id/approve`
Approves the plan submitted for a task with `needs_plan_approval = true`. Equivalent to `fb approve <id>`.

**Request:**
```http
POST /api/tasks/fb-12/approve HTTP/1.1
Cookie: fb_session=...
X-Requested-With: fleetboard
```

**Response (`200 OK`):**
```json
{
  "v": 1,
  "ok": true,
  "data": {
    "task": {
      "id": "fb-12",
      "plan_approved_at": "2026-10-30T10:30:00Z",
      ...
    }
  }
}
```

**Errors:**
- `409 Conflict` (`BAD_TRANSITION`): Task is not in `in_progress` or has no plan awaiting approval.

---

### `POST /api/tasks/:id/verify`
Verifies completion of a task in `review`, transitioning it to `done` and releasing any active claim. Equivalent to `fb verify <id> [--force]`.

**Request:**
```http
POST /api/tasks/fb-12/verify HTTP/1.1
Cookie: fb_session=...
X-Requested-With: fleetboard
Content-Type: application/json

{
  "force": false
}
```

**Payload:**
- `force` (boolean, optional, default `false`): If `true`, allows verifying code tasks that lack commit evidence.

**Response (`200 OK`):**
```json
{
  "v": 1,
  "ok": true,
  "data": {
    "task": {
      "id": "fb-12",
      "status": "done",
      "claim": null,
      ...
    }
  }
}
```

**Errors:**
- `409 Conflict` (`BAD_TRANSITION`): Task is not in `review`, or is a `code` task without commit evidence and `force` is not set.

---

### `POST /api/tasks/:id/reject`
Rejects a task in `review` back to `in_progress` with a required comment note. The active claim remains with the same agent. Equivalent to `fb reject <id> <comment>`.

**Request:**
```http
POST /api/tasks/fb-12/reject HTTP/1.1
Cookie: fb_session=...
X-Requested-With: fleetboard
Content-Type: application/json

{
  "comment": "Edge cases around token expiration are unhandled"
}
```

**Payload:**
- `comment` (string, required): Explanation for rejection. (Also accepts `body` or `text`).

**Response (`200 OK`):**
```json
{
  "v": 1,
  "ok": true,
  "data": {
    "task": {
      "id": "fb-12",
      "status": "in_progress",
      ...
    }
  }
}
```

**Errors:**
- `400 Bad Request` (`USAGE`): `comment` is empty or missing.
- `409 Conflict` (`BAD_TRANSITION`): Task is not in `review`.

---

### `POST /api/tasks/:id/release`
Releases the active claim on an `in_progress` task, transitioning it back to `todo`. Equivalent to `fb release <id>`.

**Request:**
```http
POST /api/tasks/fb-12/release HTTP/1.1
Cookie: fb_session=...
X-Requested-With: fleetboard
```

**Response (`200 OK`):**
```json
{
  "v": 1,
  "ok": true,
  "data": {
    "task": {
      "id": "fb-12",
      "status": "todo",
      "claim": null,
      ...
    }
  }
}
```

**Errors:**
- `409 Conflict` (`BAD_TRANSITION`): Task is not in `in_progress` or has no active claim.

---

### `POST /api/tasks/:id/steal`
Takes over an active claim or reassigns it to another actor. Equivalent to `fb claim <id> --steal [--for <actor>]`.

**Request:**
```http
POST /api/tasks/fb-12/steal HTTP/1.1
Cookie: fb_session=...
X-Requested-With: fleetboard
Content-Type: application/json

{
  "for": "claude 60a1168"
}
```

**Payload:**
- `for` (string, optional): Target actor spec (numeric id, external id, or label). If omitted, the claim is assigned to the human operator. (Also accepts `for_actor` or `actor`).

**Response (`200 OK`):**
```json
{
  "v": 1,
  "ok": true,
  "data": {
    "task": {
      "id": "fb-12",
      "claim": {
        "actor": {
          "label": "claude 60a1168",
          ...
        },
        ...
      },
      ...
    }
  }
}
```

**Errors:**
- `404 Not Found` (`NOT_FOUND`): Target actor specified by `for` does not exist.
- `409 Conflict` (`BAD_TRANSITION`): Task has no active claim to steal.

---

### `POST /api/tasks/:id/drop`
Drops a non-terminal task. Equivalent to `fb drop <id> [--reason …]`.

**Request:**
```http
POST /api/tasks/fb-12/drop HTTP/1.1
Cookie: fb_session=...
X-Requested-With: fleetboard
Content-Type: application/json

{
  "reason": "Superseded by architectural change"
}
```

**Payload:**
- `reason` (string, optional): Reason for dropping the task.

**Response (`200 OK`):**
```json
{
  "v": 1,
  "ok": true,
  "data": {
    "task": {
      "id": "fb-12",
      "status": "dropped",
      ...
    }
  }
}
```

**Errors:**
- `409 Conflict` (`BAD_TRANSITION`): Task is already terminal (`done` or `dropped`).

---

### `POST /api/tasks/:id/reopen`
Reopens a terminal (`done` or `dropped`) task, transitioning it back to `todo`. Equivalent to `fb reopen <id>`.

**Request:**
```http
POST /api/tasks/fb-12/reopen HTTP/1.1
Cookie: fb_session=...
X-Requested-With: fleetboard
```

**Response (`200 OK`):**
```json
{
  "v": 1,
  "ok": true,
  "data": {
    "task": {
      "id": "fb-12",
      "status": "todo",
      ...
    }
  }
}
```

**Errors:**
- `409 Conflict` (`BAD_TRANSITION`): Task is not terminal (`done` or `dropped`).

---

### `POST /api/tasks/:id/notes` & `POST /api/tasks/:id/comment`
Appends a note or comment to a task. Equivalent to `fb note <id> "…" [--kind …]`.

**Request:**
```http
POST /api/tasks/fb-12/comment HTTP/1.1
Cookie: fb_session=...
X-Requested-With: fleetboard
Content-Type: application/json

{
  "comment": "Checked with product team, priority confirmed."
}
```

**Payload:**
- `body` or `comment` or `text` (string, required): Body of the note/comment.
- `kind` (string, optional, default `"comment"`): Note kind (`"comment"`, `"handoff"`, or `"finding"`).

**Response (`200 OK`):**
```json
{
  "v": 1,
  "ok": true,
  "data": {
    "task": {
      "id": "fb-12",
      ...
    }
  }
}
```

**Errors:**
- `400 Bad Request` (`USAGE`): Empty body or invalid `kind`.
- `404 Not Found` (`NOT_FOUND`): Task does not exist.

---

## Runtime Adapters (M2)

`fb serve` integrates with live agent orchestrators via the `RuntimeAdapter` trait. The adapter is configured in `$FB_HOME/config.toml`:

```toml
adapter = "paseo"   # "none" (default) | "paseo"
```

### Supported Adapters

| Adapter | Description |
|---|---|
| `none` | Default self-report mode. No background orchestrator queries. `runtime` is `null` and staleness is based purely on elapsed time (`stale_after`). |
| `paseo` | Paseo orchestrator integration. Shells out to `paseo workspace ls --json`, `paseo ls --json`, and `paseo inspect <id> --json`. Caches responses in memory for ~5 s. Joins agents to workspaces by longest-prefix match on working directory. |

### Runtime Statuses

Tasks with an active claim expose live orchestrator state under `runtime` (as well as `runtime_status` and `pending_permissions`):

| Status | Meaning |
|---|---|
| `running` | Agent is actively executing. |
| `idle` | Agent session is active but waiting for prompt or user input. |
| `permission` | Agent has one or more pending tool or execution permissions requiring operator approval. |
| `gone` | Agent is archived, closed, or missing from the orchestrator. |

### Derived State & Needs-Me Lane Integration

- **Pending Permissions:** When an adapter reports `pending_permissions > 0` (or status `permission`), `derived.needs_me` is automatically set to `true`, bringing the task into the Needs-me lane.
- **Runtime-Aware Stale (D5):** When an adapter reports `gone` for an agent on an `in_progress` task, `derived.stale` and `derived.needs_me` are immediately set to `true` (within one poll).

### Graceful Degradation

If the orchestrator binary (e.g. `paseo`) is missing from `PATH` or encounters an execution or JSON parse error:
- A warning is emitted to `stderr` once: `warning: paseo binary not found; degrading to none adapter`
- `fb serve` and CLI commands gracefully degrade to `none` adapter behavior
- Requests and commands **never fail** due to adapter issues

