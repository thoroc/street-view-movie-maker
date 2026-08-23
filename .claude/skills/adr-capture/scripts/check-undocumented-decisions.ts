#!/usr/bin/env bun
// Scans .context/**/*.md for decision indicators and cross-references
// against ADR context: links. Reports any context files that appear
// to contain decisions but are not documented as an ADR.
// Usage: check-undocumented-decisions.ts <contextDir> <adrDir>
import { existsSync, readFileSync } from 'node:fs'
import { resolve } from 'node:path'

const DECISION_KEYWORDS = [
  /## Recommended Action/,
  /## Decision/,
  /## Proposed Approach/,
  /## Recommendation/,
  /### Decision:/,
  /### Recommendation/,
  /\*\*Decision:\*\*/,
  /Adopt Option/,
  /Option A.*recommended/,
  /recommended approach/,
  /recommended path/,
]

const collectReferencedPaths = (adrDir: string, root: string): Set<string> => {
  const referenced = new Set<string>()
  if (!existsSync(adrDir)) return referenced
  const glob = new Bun.Glob('adr-*.md')
  for (const filename of glob.scanSync({ cwd: adrDir })) {
    const content = readFileSync(`${adrDir}/${filename}`, 'utf8')
    const match = content.match(/context:\n((?: {2}- .+\n?)+)/)
    if (!match) continue
    for (const line of match[1].split('\n')) {
      const trimmed = line.trim()
      if (!trimmed.startsWith('- path:')) continue
      const pathVal = trimmed
        .split(':', 2)[1]
        .trim()
        .replace(/^["']|["']$/g, '')
      const resolved = resolve(root, pathVal)
      referenced.add(resolved)
    }
  }
  return referenced
}

const main = (): void => {
  const [contextDir, adrDir] = process.argv.slice(2)
  const root = resolve(contextDir, '..')
  const referenced = collectReferencedPaths(adrDir, root)

  const glob = new Bun.Glob('**/*.md')
  const undocumented: Array<[string, string]> = []

  for (const relToContext of [...glob.scanSync({ cwd: contextDir })].sort()) {
    const fullPath = `${contextDir}/${relToContext}`
    if (referenced.has(resolve(fullPath))) continue

    if (relToContext === 'index.yaml') continue

    const content = readFileSync(fullPath, 'utf8')

    const matchedKeyword = DECISION_KEYWORDS.find((kw) => kw.test(content))
    if (matchedKeyword) {
      const rel = `.context/${relToContext}`
      undocumented.push([rel, matchedKeyword.source])
    }
  }

  if (undocumented.length === 0) {
    console.log('All context files with decisions are documented by ADRs.')
    return
  }

  console.log(
    'WARNING: The following context files contain decision indicators but',
  )
  console.log(
    'are NOT referenced by any ADR. Consider creating an ADR for each:',
  )
  console.log()
  for (const [path, keyword] of undocumented) {
    console.log(`  ${path}`)
    console.log(`    Indicator: ${keyword}`)
    console.log()
  }
  console.log(`Total: ${undocumented.length} undocumented decision(s)`)
  console.log(
    'Run .claude/skills/adr-capture/scripts/regenerate-adr-index.sh after creating ADRs.',
  )
  process.exit(2)
}

main()
