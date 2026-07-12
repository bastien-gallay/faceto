<!-- markdownlint-disable MD013 -->

# CLAUDE.md

@AGENTS.md

The project's full standing guidance lives in [`AGENTS.md`](AGENTS.md), imported above so it loads
every session (Claude Code reads `CLAUDE.md`, not `AGENTS.md` — the `@AGENTS.md` line pulls it in).
Keep all substance in `AGENTS.md` so Claude Code and other tools can't drift; add Claude-Code-only
notes below this line if any are ever needed.

Path-scoped rules in [`.claude/rules/`](.claude/rules/) add UI guidance (when you edit
`src/template.html` or `src/render/`) and event-spine invariants (when you edit `src/events/` or
`src/serve/`). These are symlinked to `.agents/rules/` so that Google Antigravity (Gemini) loads
the identical path-scoped rules and stays fully aligned.
