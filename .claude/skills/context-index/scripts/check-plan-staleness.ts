#!/usr/bin/env bun
// Advisory check: flags active plans/follow-ups older than a threshold.
// Usage: check-plan-staleness.ts <indexPath> <thresholdDays>
import { existsSync, readFileSync } from 'node:fs'

type IndexEntry = {
  path?: string
  title?: string
  status?: string
  date?: string
}

const parseIndex = (text: string): Array<[string, IndexEntry]> => {
  const entries: Array<[string, IndexEntry]> = []
  let currentSection: string | null = null
  let current: IndexEntry = {}

  const flush = () => {
    if (Object.keys(current).length > 0 && currentSection) {
      entries.push([currentSection, current])
    }
    current = {}
  }

  for (const line of text.split('\n')) {
    if (/^(plans|follow-ups):\s*$/.test(line)) {
      flush()
      currentSection = line.match(/^(plans|follow-ups):/)?.[1] ?? null
      continue
    }
    if (/^[a-z-]+:\s*$/.test(line) && !line.startsWith(' ')) {
      flush()
      currentSection = null
      continue
    }
    if (currentSection === null) continue

    const pathMatch = line.match(/^\s*- path: "(.+)"/)
    if (pathMatch) {
      flush()
      current = { path: pathMatch[1] }
      continue
    }
    const titleMatch = line.match(/^\s*title: "(.+)"/)
    if (titleMatch) {
      current.title = titleMatch[1]
      continue
    }
    const statusMatch = line.match(/^\s*status: (\S+)/)
    if (statusMatch) {
      current.status = statusMatch[1]
      continue
    }
    const dateMatch = line.match(/^\s*date: (\S+)/)
    if (dateMatch) {
      current.date = dateMatch[1]
    }
  }
  flush()
  return entries
}

const isValidIsoDate = (value: string): boolean =>
  /^\d{4}-\d{2}-\d{2}$/.test(value) && !Number.isNaN(Date.parse(value))

const main = (): void => {
  const [indexPath, thresholdArg] = process.argv.slice(2)
  const threshold = Number.parseInt(thresholdArg, 10)

  if (!existsSync(indexPath)) {
    console.error('context index not found, skipping plan-staleness check')
    return
  }

  const entries = parseIndex(readFileSync(indexPath, 'utf8'))
  const today = new Date()
  const stale: Array<[string, string, number]> = []

  for (const [, e] of entries) {
    if (e.status !== 'active') continue
    if (!e.date || !isValidIsoDate(e.date)) continue
    const ageMs = today.getTime() - new Date(e.date).getTime()
    const age = Math.floor(ageMs / (1000 * 60 * 60 * 24))
    if (age > threshold) stale.push([e.path ?? '', e.title ?? '', age])
  }

  if (stale.length > 0) {
    console.log(
      `NOTICE: ${stale.length} active plan(s)/follow-up(s) older than ${threshold} days (advisory, non-blocking):`,
    )
    for (const [path, title, age] of stale.sort((a, b) => b[2] - a[2])) {
      console.log(`  ${path} (${age}d old): "${title}"`)
    }
    console.log()
    console.log(
      'Age alone does not mean a plan is wrong, but check whether recent commits already satisfy its scope',
    )
    console.log(
      "under a different name before leaving it active. See ways-of-working.md, 'Keeping plans in sync'.",
    )
  }

  // Advisory only, matching the journal-tag-lint precedent: always exit 0.
}

main()
