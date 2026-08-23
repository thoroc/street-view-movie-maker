#!/usr/bin/env bun
// PreToolUse guard: block Bash/Read calls that would surface decrypted .env secrets.

const ENV_FILE = /(^|[/\s])\.env(\.keys)?(\b|$)/

const BASH_DUMPS: ReadonlyArray<readonly [RegExp, string]> = [
  [/\bprintenv\b/, 'printenv'],
  [/(^|[;&|]|\s)env(\s*(\||;|&{1,2}|>|$))/, 'a bare `env` dump'],
  [/process\.env(?![.[\w])/, 'a bare `process.env` dump'],
  [/os\.environ(?!\s*[.[])/, 'a bare `os.environ` dump'],
]

const block = (what: string, guidance: string): never => {
  console.error(`Blocked: ${what} would expose decrypted secrets. ${guidance}`)
  process.exit(2)
}

type HookInput = {
  tool_name?: string
  tool_input?: { command?: string; file_path?: string }
}

export const checkHookInput = (data: HookInput): void => {
  const toolName = data.tool_name ?? ''
  const toolInput = data.tool_input ?? {}

  const text =
    toolName === 'Bash'
      ? (toolInput.command ?? '')
      : toolName === 'Read'
        ? (toolInput.file_path ?? '')
        : ''
  if (!text) return

  if (toolName === 'Bash' && /\bdotenvx\b/.test(text)) return

  if (ENV_FILE.test(text)) {
    block(
      'direct access to .env/.env.keys',
      'Use `dotenvx get <KEY>` or `dotenvx run -- <command>` instead (see repo instructions on secrets).',
    )
  }

  if (toolName === 'Bash') {
    for (const [pattern, label] of BASH_DUMPS) {
      if (pattern.test(text)) {
        block(
          label,
          'Ask for the specific derived value or result instead of dumping the process environment.',
        )
      }
    }
  }
}

const main = async (): Promise<void> => {
  const raw = await new Response(Bun.stdin).text()
  checkHookInput(JSON.parse(raw) as HookInput)
}

if (import.meta.main) {
  await main()
}
