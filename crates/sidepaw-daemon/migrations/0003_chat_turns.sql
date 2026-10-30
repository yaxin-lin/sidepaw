-- Version 3: the chat remembers what was said (ADR 11).

-- Turns of the chat window and the lines the pet said unprompted in
-- its bubble, so a conversation continues across windows and a bubble
-- line can be answered in chat. Bodies are redacted. Kept a week.
CREATE TABLE chat_turns (
    id          INTEGER PRIMARY KEY,
    ts          TEXT NOT NULL,
    role        TEXT NOT NULL,              -- user|pet
    channel     TEXT NOT NULL DEFAULT 'chat', -- chat|bubble
    body        TEXT NOT NULL
);
CREATE INDEX chat_turns_ts ON chat_turns(ts);
