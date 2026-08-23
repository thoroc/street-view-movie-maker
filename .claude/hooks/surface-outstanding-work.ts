#!/usr/bin/env bun
// SessionStart hook: surface active follow-ups and plans from
// .context/index.yaml (the context-index format). Pure TypeScript — no
// python or yaml dependency; parses the small, predictable subset of YAML the
// context-index generator emits.

import { readFile } from 'node:fs/promises'
import { join } from 'node:path'

export type IndexEntry = { title?: string; path?: string; status?: string }
export type ContextIndex = {
  followUps: IndexEntry[]
  plans: IndexEntry[]
  unparseable?: string
}

const stripQuotes = (value: string): string => {
  const v = value.trim()
  if (v.length >= 2 && v[0] === '"' && v[v.length - 1] === '"')
    return v.slice(1, -1)
  if (v.length >= 2 && v[0] === "'" && v[v.length - 1] === "'")
    return v.slice(1, -1)
  return v
}

export const parseContextIndex = (content: string): ContextIndex => {
  const followUps: IndexEntry[] = []
  const plans: IndexEntry[] = []
  let section: 'plans' | 'follow-ups' | null = null
  let current: Record<string, string> | null = null

  for (const rawLine of content.split('\n')) {
    const line = rawLine.trimEnd()
    const trimmed = line.trim()
    if (!trimmed || trimmed.startsWith('#')) continue

    const kv = line.trimStart().match(/^([A-Za-z][A-Za-z0-9-]*):\s*(.*)$/)
    if (kv) {
      const key = kv[1]
      const value = stripQuotes(kv[2])
      const indent = line.length - line.trimStart().length

      if (indent === 0) {
        section =
          key === 'plans' ? 'plans' : key === 'follow-ups' ? 'follow-ups' : null
        current = null
      } else if (current) {
        current[key] = value
      }
      continue
    }

    const listMatch = line.trimStart().match(/^-\s+(.*)$/)
    if (listMatch) {
      current = section === 'plans' || section === 'follow-ups' ? {} : null
      if (current) {
        if (section === 'plans') plans.push(current)
        else if (section === 'follow-ups') followUps.push(current)
        const inline = listMatch[1].match(/^([A-Za-z][A-Za-z0-9-]*):\s*(.*)$/)
        if (inline) current[inline[1]] = stripQuotes(inline[2])
      }
      continue
    }

    return { followUps, plans, unparseable: line }
  }

  return { followUps, plans }
}

const UNPARSEABLE_MESSAGE = (line: string): string =>
  [
    '## Outstanding work: UNAVAILABLE',
    '',
    '`.context/index.yaml` did not parse as the context-index format, so no follow-ups or plans could be read.',
    '',
    `\`\`\`\n${line}\n\`\`\``,
    '',
    "Regenerate it with your project's context-index tooling (see AGENTS.md).",
  ].join('\n')

export const buildOutstandingMessage = (
  index: ContextIndex,
): string | undefined => {
  if (index.unparseable) return UNPARSEABLE_MESSAGE(index.unparseable)

  const lines: string[] = []
  const groups: Array<[string, IndexEntry[]]> = [
    ['Follow-ups', index.followUps],
    ['Plans', index.plans],
  ]

  for (const [label, entries] of groups) {
    const active = entries.filter((entry) => entry.status === 'active')
    if (active.length === 0) continue
    lines.push(`### Active ${label}`)
    for (const entry of active) {
      lines.push(`- ${entry.title ?? entry.path} (${entry.path ?? 'unknown'})`)
    }
    lines.push('')
  }

  if (lines.length === 0) return undefined
  return `## Outstanding work (.context/index.yaml)\n\n${lines.join('\n').trimEnd()}`
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
  const indexFile =
    process.env.AGENT_HOOKS_INDEX_FILE ??
    join(projectDir, '.context', 'index.yaml')
  const raw = await readFile(indexFile, 'utf8').catch(() => null)
  if (raw === null) return

  const message = buildOutstandingMessage(parseContextIndex(raw))
  if (message) console.log(report(message))
}

if (import.meta.main) {
  await main()
}
