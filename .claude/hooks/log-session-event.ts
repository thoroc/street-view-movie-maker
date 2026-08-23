#!/usr/bin/env bun
// Lifecycle hook: append every hook event to a per-session JSONL debug log.
// Wired across the session lifecycle in .claude/settings.json. Must never
// throw — a logging failure must not block the triggering event.

import { appendFile, mkdir } from 'node:fs/promises'
import { join } from 'node:path'

const MAX_FIELD_LEN = 4000

type JsonValue =
  | string
  | number
  | boolean
  | null
  | JsonValue[]
  | { [key: string]: JsonValue }
type HookInput = Record<string, JsonValue> & { session_id?: string }

const truncate = (value: JsonValue): JsonValue => {
  if (typeof value === 'string' && value.length > MAX_FIELD_LEN) {
    const clipped = value.length - MAX_FIELD_LEN
    return `${value.slice(0, MAX_FIELD_LEN)}...<truncated ${clipped} chars>`
  }
  if (Array.isArray(value)) return value.map(truncate)
  if (value !== null && typeof value === 'object') {
    return Object.fromEntries(
      Object.entries(value).map(([key, v]) => [key, truncate(v)]),
    )
  }
  return value
}

const parseInput = (raw: string): HookInput => {
  if (!raw.trim()) return {}
  try {
    return JSON.parse(raw) as HookInput
  } catch {
    return { unparsed_input: raw }
  }
}

const toLocalIso = (date: Date): string => {
  const pad = (n: number) => String(n).padStart(2, '0')
  const offsetMin = -date.getTimezoneOffset()
  const sign = offsetMin >= 0 ? '+' : '-'
  const offH = pad(Math.floor(Math.abs(offsetMin) / 60))
  const offM = pad(Math.abs(offsetMin) % 60)
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}T${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}${sign}${offH}:${offM}`
}

const localDateStamp = (date: Date): string => {
  const pad = (n: number) => String(n).padStart(2, '0')
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`
}

export const logEvent = async (
  projectDir: string,
  data: HookInput,
  now: Date,
): Promise<void> => {
  const sessionId = data.session_id ?? 'unknown-session'
  const logDir = join(projectDir, '.context', 'logs', sessionId)
  await mkdir(logDir, { recursive: true })
  const logFile = join(logDir, `${localDateStamp(now)}.debug.jsonl`)

  const entry = {
    ts: toLocalIso(now),
    ...(truncate(data) as Record<string, JsonValue>),
  }
  await appendFile(logFile, `${JSON.stringify(entry)}\n`)
}

const main = async (): Promise<void> => {
  const raw = await new Response(Bun.stdin).text()
  const data = parseInput(raw)
  const projectDir = process.env.CLAUDE_PROJECT_DIR ?? '.'

  try {
    await logEvent(projectDir, data, new Date())
  } catch (error) {
    console.error(
      `log-session-event: ${error instanceof Error ? error.message : String(error)}`,
    )
  }

  console.log(JSON.stringify({ suppressOutput: true }))
}

if (import.meta.main) {
  await main()
}
