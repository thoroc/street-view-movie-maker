#!/usr/bin/env bun
// Validates YAML frontmatter in docs/ADR/adr-*.md files against
// assets/schemas/adr-frontmatter.schema.json.
// Usage: validate-adr-frontmatter.ts <schemaPath> <file> [<file> ...]
import { existsSync, readFileSync } from 'node:fs'

type Schema = {
  required?: string[]
  properties?: Record<string, { enum?: string[]; pattern?: string }>
}

const parseFrontmatter = (content: string): Record<string, string> | null => {
  if (!content.startsWith('---\n')) return null
  const end = content.indexOf('---\n', 4)
  if (end === -1) return null
  const fmText = content.slice(4, end)
  const fm: Record<string, string> = {}
  for (const line of fmText.split('\n')) {
    if (line.startsWith(' ')) continue
    if (line.includes(': ')) {
      const idx = line.indexOf(': ')
      const key = line.slice(0, idx).trim()
      const value = line
        .slice(idx + 2)
        .trim()
        .replace(/^"|"$/g, '')
      fm[key] = value
    }
  }
  return fm
}

const main = (): void => {
  const [schemaPath, ...files] = process.argv.slice(2)
  if (files.length === 0) return

  const schema: Schema = JSON.parse(readFileSync(schemaPath, 'utf8'))
  const required = schema.required ?? []
  const props = schema.properties ?? {}

  const enumFields = Object.entries(props).filter(([, v]) => v.enum)
  const patternFields = Object.entries(props)
    .filter(([, v]) => v.pattern)
    .map(([k, v]) => [k, new RegExp(v.pattern as string)] as const)

  const errors: string[] = []

  for (const f of files) {
    if (!existsSync(f)) continue
    const content = readFileSync(f, 'utf8')
    if (!content.startsWith('---\n')) {
      errors.push(`${f}: missing frontmatter (file must begin with ---)`)
      continue
    }
    if (content.indexOf('---\n', 4) === -1) {
      errors.push(`${f}: unclosed frontmatter (no closing ---)`)
      continue
    }
    const fm = parseFrontmatter(content) ?? {}

    for (const field of required) {
      if (!fm[field]) errors.push(`${f}: missing required field '${field}'`)
    }
    for (const [field, def] of enumFields) {
      const values = def.enum as string[]
      if (fm[field] && !values.includes(fm[field])) {
        errors.push(
          `${f}: '${field}' must be one of ${JSON.stringify(values)}, got '${fm[field]}'`,
        )
      }
    }
    for (const [field, pattern] of patternFields) {
      if (fm[field] && !pattern.test(fm[field])) {
        errors.push(
          `${f}: '${field}' does not match pattern '${pattern.source}', got '${fm[field]}'`,
        )
      }
    }
  }

  if (errors.length > 0) {
    console.log('ADR frontmatter validation errors:')
    for (const e of errors) console.log(`  ${e}`)
    process.exit(1)
  }

  console.log(`OK: ${files.length} file(s) validated against schema`)
}

main()
