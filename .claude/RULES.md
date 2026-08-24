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

### Rule: Create a handover file on session handover

**Directive:** ALWAYS create a handover file under .context/handover/<date>-<slug>.md when handing over a session to another agent or human, before concluding the work.

**Rationale:** Handovers need a durable, discoverable resume point; a dated handover file under .context/handover/ gives the next session the current state, what was tried, and what remains without re-discovery.

### Rule: Dig into git history when inspecting the project, and save findings

**Directive:** When investigating this project's structure, past decisions, or how a feature came to be (e.g. inferring ADR candidates, auditing whether documentation matches reality), ALWAYS consult `git log`/`git show` on the relevant history, not just the current working tree and `.context/` files -- commit messages and diffs carry decisions and rationale that never made it into a plan or finding. Save what the investigation turns up as a `.context/findings/<date>-<slug>.md` file via the `context-file` skill, even when the immediate task was something else (e.g. an ADR-inference request), so the git-history research is discoverable later instead of living only in a chat transcript.

**Rationale:** This repo's own commit history is denser with real decisions than its `.context/` plans alone -- e.g. the `fnox`/`age` secrets migration and the Python-to-Rust port's rejected alternatives are only fully visible in commit messages, not restated anywhere else. Treating git history as a first-class source, and persisting what it reveals, is what let five real ADR candidates get identified in one pass instead of being rediscovered piecemeal in future sessions.

### Rule: Save temporary artifacts to .tmp, not external scratchpad paths

**Directive:** ALWAYS write temporary artifacts produced while working in this repo (test output, generated screenshots, throwaway run output, scratch scripts, etc.) under this repo's .tmp/ directory (already gitignored) rather than an external session scratchpad path. Create .tmp/ if it does not exist.

**Rationale:** The session scratchpad lives outside the repo and outside this worktree, so files written there are easy to lose track of, get silently deleted during unrelated cleanup, and are not visible to anyone inspecting the repo/worktree directly. .tmp/ keeps temporary output colocated with the project it belongs to, still out of version control, and easy to point the user at.
