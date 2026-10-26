# 5. One model provider at a time

Status: accepted, 2026-10-25

## Context

Chat apps often expose a provider matrix (one model for this, another
for that). It is powerful and confusing. The pet has two uses for a
model: generating local lines once per persona change, and streaming
chat.

## Decision

Exactly one configured provider: an official API (DeepSeek, OpenAI,
Anthropic), a local Ollama, or any OpenAI-compatible base URL. The same
model is used for both jobs. Lines are generated in a batch and cached,
so day-to-day use makes no model calls unless the user opens the chat
panel. With no provider configured, everything except chat works.

## Consequences

Setup is one dropdown and one key. Advanced routing can be added later
behind the same config key if someone needs it.
