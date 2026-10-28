-- Version 2: answers the pet gave for the user (level 4).

-- Permission prompts the pet answered on the user's behalf (level 4).
-- One row per click; `decision` is allow or deny.
CREATE TABLE approvals (
    id          INTEGER PRIMARY KEY,
    ts          TEXT NOT NULL,
    session_id  TEXT NOT NULL,
    tool_name   TEXT,
    summary     TEXT,                       -- redacted one-line description
    decision    TEXT NOT NULL
);
