#!/usr/bin/env bun
// Scans .context/**/*.md, parses frontmatter, and regenerates index.yaml.
// Usage: regenerate-context-index.ts <root> <indexPath> <checkMode: "true"|"false">
import { existsSync, readFileSync, writeFileSync } from 'node:fs'
import { dirname, relative, resolve } from 'node:path'

type Entry = {
  title: string
  type: string
  status: string
  date: string
  path: string
  related?: string[]
}

const TYPE_GROUP_KEY: Record<string, string> = {
  plan: 'plans',
  finding: 'findings',
  analysis: 'analysis',
  instruction: 'instructions',
  'follow-up': 'follow-ups',
  learning: 'learnings',
  handover: 'handovers',
}
const TYPE_ORDER = [
  'plans',
  'findings',
  'analysis',
  'instructions',
  'follow-ups',
  'learnings',
  'handovers',
]
const TYPE_LABEL: Record<string, string> = {
  plans: 'Plans',
  findings: 'Findings',
  analysis: 'Analysis',
  instructions: 'Instructions',
  'follow-ups': 'Follow-ups',
  learnings: 'Learnings',
  handovers: 'Handovers',
  other: 'Other',
}

// A double-quoted YAML scalar built by naive string interpolation breaks on
// any value containing '"'. JSON string syntax is a valid YAML 1.2
// double-quoted scalar and escapes what needs escaping.
const emitScalar = (v: string): string => JSON.stringify(v)

// Frontmatter values may use either YAML quoting style. Stripping only '"'
// left single-quoted values carrying their quotes plus any inner content, so
// both styles are unwrapped explicitly here.
const unquote = (raw: string): string => {
  const v = raw.trim()
  if (
    v.length >= 2 &&
    v[0] === v[v.length - 1] &&
    (v[0] === '"' || v[0] === "'")
  ) {
    const inner = v.slice(1, -1)
    return v[0] === "'" ? inner.replaceAll("''", "'") : inner
  }
  return v
}

const parseFrontmatter = (
  text: string,
): (Record<string, string> & { _raw: string }) | null => {
  if (!text.startsWith('---\n')) return null
  const end = text.indexOf('---\n', 4)
  if (end === -1) return null
  const fmText = text.slice(4, end)
  const fm: Record<string, string> = {}
  for (const line of fmText.split('\n')) {
    if (line.startsWith(' ')) continue
    if (line.includes(': ')) {
      const idx = line.indexOf(': ')
      fm[line.slice(0, idx).trim()] = unquote(line.slice(idx + 2))
    }
  }
  return { ...fm, _raw: fmText }
}

const extractRelated = (raw: string): string[] => {
  const match = raw.match(/related:\n((?: {2}- .+\n?)+)/)
  if (!match) return []
  return match[1]
    .split('\n')
    .filter((line) => line.trim().startsWith('- '))
    .map((line) => line.trim().slice(2))
}

// Findings are investigative input, not a standing task list: once the finding's recommendation
// is *fully* actioned, it should move to done/superseded too. This can't detect "fully actioned"
// on its own, but a finding whose every related plan is already `done` is a mechanical enough
// signal to be worth a nudge — a finding is not fully actioned while even one related plan is
// still open, so this only fires when ALL of them are done, not just one.
const findStaleFindings = (
  root: string,
  entries: Entry[],
): Array<{ path: string; title: string; relatedPaths: string[] }> => {
  const byPath = new Map(entries.map((e) => [e.path, e]))
  const stale: Array<{ path: string; title: string; relatedPaths: string[] }> = []
  for (const e of entries) {
    if (e.type !== 'finding' || e.status !== 'active' || !e.related?.length) continue
    const fileDir = dirname(resolve(root, e.path))
    const related = e.related
      .map((r) => relative(root, resolve(fileDir, r)))
      .map((relatedPath) => byPath.get(relatedPath))
      .filter((target): target is Entry => target !== undefined)
    if (related.length === e.related.length && related.every((t) => t.status === 'done')) {
      stale.push({ path: e.path, title: e.title, relatedPaths: related.map((t) => t.path) })
    }
  }
  return stale
}

const reportStaleFindings = (
  stale: Array<{ path: string; title: string; relatedPaths: string[] }>,
): void => {
  if (stale.length === 0) return
  console.error(
    'NOTICE: active finding(s) whose related plan(s) are all already done (advisory, verify and update status):',
  )
  for (const f of stale) {
    console.error(`  ${f.path} — ${f.relatedPaths.join(', ')} done: "${f.title}"`)
  }
  console.error()
  console.error(
    "If the finding's recommendation has been fully acted on, flip status to done/superseded and",
  )
  console.error(
    "add an Outcome section. See ways-of-working.md, 'Keeping plans and findings in sync'.",
  )
}

const scanContextFiles = (
  root: string,
  contextDir: string,
): { missing: string[]; entries: Entry[] } => {
  const glob = new Bun.Glob('**/*.md')
  const missing: string[] = []
  const entries: Entry[] = []

  for (const relToContext of [...glob.scanSync({ cwd: contextDir })].sort()) {
    const fullPath = `${contextDir}/${relToContext}`
    const rel = relative(root, fullPath)
    const content = readFileSync(fullPath, 'utf8')
    if (rel.startsWith('.context/audits/') && !content.startsWith('---\n')) {
      continue // skip legacy audits without frontmatter
    }
    const fm = parseFrontmatter(content)
    if (fm === null) {
      missing.push(rel)
      continue
    }
    const required = ['title', 'type', 'status', 'date']
    const absent = required.filter((field) => !fm[field])
    if (absent.length > 0) {
      missing.push(`${rel} (missing: ${absent.join(', ')})`)
      continue
    }
    const entry: Entry = {
      title: fm.title,
      type: fm.type,
      status: fm.status,
      date: fm.date,
      path: rel,
    }
    const related = extractRelated(fm._raw)
    if (related.length > 0) entry.related = related
    entries.push(entry)
  }

  return { missing, entries }
}

const buildIndexLines = (entries: Entry[]): string[] => {
  const grouped: Record<string, Entry[]> = {}
  for (const e of entries) {
    const key = TYPE_GROUP_KEY[e.type] ?? 'other'
    grouped[key] ??= []
    grouped[key].push(e)
  }
  const typeOrder = [...TYPE_ORDER]
  if (grouped.other) typeOrder.push('other')

  const statusCounts: Record<string, number> = {}
  for (const e of entries)
    statusCounts[e.status] = (statusCounts[e.status] ?? 0) + 1
  const statusSummary = Object.entries(statusCounts)
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([s, n]) => `${n} ${s}`)
    .join(', ')

  const lines = [
    '# Auto-generated by context-index skill. Do not edit manually.',
    `# Last updated: ${new Date().toISOString().slice(0, 10)}`,
    `# ${entries.length} entries: ${statusSummary}`,
    '',
  ]
  for (const t of typeOrder) {
    const list = grouped[t]
    if (!list || list.length === 0) continue
    lines.push(`# ── ${TYPE_LABEL[t]} (${list.length}) ──`)
    lines.push(`${t}:`)
    for (const e of list) {
      lines.push(`  - path: ${emitScalar(e.path)}`)
      lines.push(`    title: ${emitScalar(e.title)}`)
      lines.push(`    status: ${emitScalar(e.status)}`)
      lines.push(`    date: ${e.date}`)
      if (e.related && e.related.length > 0) {
        lines.push('    related:')
        for (const r of e.related) lines.push(`      - ${emitScalar(r)}`)
      }
    }
    lines.push('')
  }
  return lines
}

const main = (): void => {
  const [root, indexPath, checkModeArg] = process.argv.slice(2)
  const checkMode = checkModeArg === 'true'
  const contextDir = `${root}/.context`

  const { missing, entries } = scanContextFiles(root, contextDir)

  if (missing.length > 0) {
    console.error(
      'WARNING: files with missing frontmatter (excluded from index):',
    )
    for (const f of missing) console.error(`  ${f}`)
  }

  reportStaleFindings(findStaleFindings(root, entries))

  const output = `${buildIndexLines(entries).join('\n')}\n`
  if (checkMode) {
    const current = existsSync(indexPath) ? readFileSync(indexPath, 'utf8') : ''
    if (output !== current) {
      console.error(
        "ERROR: .context/index.yaml is stale — run 'hk fix' to regenerate",
      )
      process.exit(1)
    }
    console.log('context index is fresh')
  } else {
    writeFileSync(indexPath, output)
    console.log(`Generated ${entries.length} entries -> ${indexPath}`)
  }
}

main()
