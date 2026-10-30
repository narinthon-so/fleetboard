# fleetboard — HTTP API (M1)

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
- All requests to `/api/*` (except `POST /api/login`) require a valid session cookie.
- Requests without a valid cookie receive `401 Unauthorized`.

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
