# Agent Rules

Project-level agent behavioural rules. This is the authoritative source all agents read before acting in this
repository. See `.claude/skills/rules-management/` for how entries are added and validated.

### Rule: Use a git worktree for every new branch

**Directive:** ALWAYS create a new git worktree under .claude/worktrees/ before creating and checking out a new branch, rather than switching branches in the primary working directory. Remove the worktree (git worktree remove) once its branch is merged.

**Rationale:** An agent session can be interrupted or resumed while another branch is being worked on elsewhere; switching branches in-place in the primary checkout risks one session's working tree being changed out from under another mid-task. A dedicated worktree per branch isolates each session's working tree so branch switches and commits in one session cannot affect another.

### Rule: Check whether a skill was imported before deciding where a fix lives

**Directive:** Before amending or extending the behaviour of a `.claude/skills/<name>` skill, check whether it was imported wholesale from another project's skill set (as this repo's were, via commit `4e642ce`, "chore: import and adapt Claude Code skills/instructions for this repo") versus authored or substantially rewritten natively in this repo. There is currently no active upstream this repo re-syncs from, so a direct edit to the skill file is not at risk of being silently overwritten -- amend the skill's own SKILL.md/scripts directly. Only fall back to a project-specific override in `.claude/RULES.md` if a future workflow reintroduces re-vendoring from an external source.

**Rationale:** The imported skill set required real content fixes (see the rule below) rather than a thin adaptation layer; treating every imported skill as permanently untouchable would leave known-wrong content in place indefinitely.

### Rule: Treat incomplete or mismatching documentation as urgent tech debt

**Directive:** ALWAYS log incomplete or mismatching documentation (a doc that no longer matches the code, or is missing content a change should have added) as a docs/TECH_DEBT.md row under Area "Code quality", but treat it as urgent: fix it in the same session it is discovered rather than leaving Status: Open for later spare-time cleanup like a routine tech-debt row. Do not route it to docs/RISK_REGISTER.md merely to avoid this urgency -- it still belongs in TECH_DEBT.md, just fixed immediately instead of deferred.

**Rationale:** Stale or mismatched documentation actively misleads the next reader -- human or agent -- into following wrong guidance, and the cost compounds with every reader who trusts it before it's caught. This is not hypothetical: on 2026-08-23 the imported `.claude/instructions/` and `.claude/RULES.md` were found to still contain another project's regulatory language, GitLab-only workflow assumptions, AWS delivery-pipeline theme vocabulary, and references to skills (`mr-review`, `pr-author`) that don't exist in this repo -- all left over from the import despite an earlier pass claiming to have adapted them. That makes stale docs categorically different from ordinary code-quality cleanup, which is genuinely fine to defer.

#### Rule: Create a handover file on session handover

**Directive:** ALWAYS create a handover file under .context/handover/<date>-<slug>.md when handing over a session to another agent or human, before concluding the work.

**Rationale:** Handovers need a durable, discoverable resume point; a dated handover file under .context/handover/ gives the next session the current state, what was tried, and what remains without re-discovery.
