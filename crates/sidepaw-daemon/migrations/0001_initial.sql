-- Version 1: sessions, events, state changes, memories, trigger stats
-- and the audit of model calls. Databases created before migrations
-- existed already hold all of this and start at version 2.

CREATE TABLE schema_version (
    version INTEGER NOT NULL
);

CREATE TABLE sessions (
    id          TEXT PRIMARY KEY,           -- "<host>:<session>"
    host        TEXT NOT NULL,
    cwd         TEXT,                       -- null below level 2
    started_at  TEXT NOT NULL,              -- RFC 3339 UTC
    ended_at    TEXT,
    last_state  TEXT NOT NULL DEFAULT 'idle',
    turns       INTEGER NOT NULL DEFAULT 0,
    tool_calls  INTEGER NOT NULL DEFAULT 0,
    errors      INTEGER NOT NULL DEFAULT 0,
    tokens_in   INTEGER NOT NULL DEFAULT 0,
    tokens_out  INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE events (
    id          INTEGER PRIMARY KEY,
    session_id  TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    ts          TEXT NOT NULL,
    kind        TEXT NOT NULL,
    tool_name   TEXT,
    tool_cat    TEXT,
    text        TEXT,                       -- redacted; null below level 2
    error_type  TEXT,
    error_msg   TEXT,                       -- redacted; null below level 2
    tokens_in   INTEGER,
    tokens_out  INTEGER,
    raw         TEXT                        -- redacted JSON; null below level 2
);
CREATE INDEX events_session_ts ON events(session_id, ts);
CREATE INDEX events_ts ON events(ts);

-- Pet state transitions, one row per change. Lets the daily report and
-- the proactive scheduler answer "how long was it waiting" cheaply.
CREATE TABLE state_changes (
    id          INTEGER PRIMARY KEY,
    session_id  TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    ts          TEXT NOT NULL,
    state       TEXT NOT NULL,              -- idle|thinking|tool|waiting|done|error|sleeping
    detail      TEXT                        -- tool category for 'tool'
);
CREATE INDEX state_changes_session_ts ON state_changes(session_id, ts);

-- Things the pet remembers. Fully visible and editable by the user.
CREATE TABLE memories (
    id          INTEGER PRIMARY KEY,
    created_at  TEXT NOT NULL,
    source      TEXT NOT NULL,              -- user|observed
    body        TEXT NOT NULL,
    retention   TEXT NOT NULL DEFAULT '30d',-- forever|30d|7d
    pinned      INTEGER NOT NULL DEFAULT 0
);

-- Proactive scheduler feedback. One row per trigger type.
CREATE TABLE trigger_stats (
    trigger     TEXT PRIMARY KEY,
    fired       INTEGER NOT NULL DEFAULT 0,
    dismissed   INTEGER NOT NULL DEFAULT 0,
    answered    INTEGER NOT NULL DEFAULT 0,
    last_fired  TEXT,
    cooldown_s  INTEGER NOT NULL DEFAULT 0
);

-- Every call to a remote model. The payload itself is not stored.
CREATE TABLE audit (
    id          INTEGER PRIMARY KEY,
    ts          TEXT NOT NULL,
    provider    TEXT NOT NULL,
    purpose     TEXT NOT NULL,              -- chat|lines|persona|nudge
    bytes       INTEGER NOT NULL,
    sha256      TEXT NOT NULL,
    level       INTEGER NOT NULL
);
