#!/usr/bin/env bun
// SessionStart hook: surface the knowledge-base index summary so the model
// searches existing patterns before concluding none exist. Pure TypeScript —
// reads the NDJSON index directly, no jq or python.

import { readFile } from 'node:fs/promises'
import { join } from 'node:path'

const KB_REL = 'docs/knowledge-base-index.ndjson'
const DEFAULT_QMD_COLLECTION = 'project-kb'

export const buildKnowledgeMessage = (
  lines: string[],
  qmdCollection: string,
): string | undefined => {
  const entries = lines
    .map((line): { domain?: unknown } | undefined => {
      try {
        return JSON.parse(line) as { domain?: unknown }
      } catch {
        return undefined
      }
    })
    .filter((entry): entry is { domain?: unknown } => entry !== undefined)

  if (entries.length === 0) return undefined

  const counts = new Map<string, number>()
  for (const entry of entries) {
    const domain = typeof entry.domain === 'string' ? entry.domain : 'unknown'
    counts.set(domain, (counts.get(domain) ?? 0) + 1)
  }
  const domains = [...counts.entries()]
    .sort((a, b) => b[1] - a[1])
    .map(([domain, count]) => `${domain} (${count})`)
    .join(', ')

  return [
    `## Knowledge base (${KB_REL})`,
    '',
    `${entries.length} articles across domains: ${domains}.`,
    '',
    `Before concluding no prior pattern, decision, or automation exists for a task, search it: \`qmd query\` (collection \`${qmdCollection}\`) or check the index file directly — a negative result in an external repo is not evidence there is nothing here.`,
  ].join('\n')
}

const report = (message: string): string =>
  JSON.stringify({
    systemMessage: message,
    hookSpecificOutput: {
      hookEventName: 'SessionStart',
      additionalContext: message,
    },
  })

const main = async (): Promise<void> => {
  const projectDir = process.env.CLAUDE_PROJECT_DIR ?? '.'
  const indexFile = process.env.AGENT_HOOKS_KB_INDEX ?? join(projectDir, KB_REL)
  const qmdCollection =
    process.env.AGENT_HOOKS_QMD_COLLECTION ?? DEFAULT_QMD_COLLECTION

  const raw = await readFile(indexFile, 'utf8').catch(() => null)
  if (raw === null) return

  const lines = raw.split('\n').filter((line) => line.trim().length > 0)
  const message = buildKnowledgeMessage(lines, qmdCollection)
  if (message) console.log(report(message))
}

if (import.meta.main) {
  await main()
}
