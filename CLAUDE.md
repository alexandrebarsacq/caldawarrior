<!-- foundry-sdd-start -->
## Foundry SDD Workflow

| When you need to... | Use |
|---------------------|-----|
| Create/review/modify a spec | `foundry-spec` skill |
| Find next task, implement | `foundry-implement` skill |
| Verify implementation | `foundry-review` skill |
| Run tests and debug | Run tests directly via Bash |
| Create PR with spec context | `gh pr create` |

### Key Rules

**Always use skills over direct MCP calls:**
- Skills provide workflow orchestration, error handling, and context
- Do NOT call `mcp__plugin_foundry_foundry-mcp__authoring` directly
- Do NOT call `mcp__plugin_foundry_foundry-mcp__task` directly
- For phases with tasks, use `phase-add-bulk` (not `phase-add`)

**Use Explore subagent before skills:**
- Before `foundry-spec`: Understand codebase architecture and existing patterns
- Before `foundry-implement`: Find related code, test files, dependencies
- Thoroughness levels: `quick` (single file), `medium` (related files), `very thorough` (subsystem)

**Task completion gates - NEVER mark complete if:**
- Tests are failing (unless phase has separate verify task)
- Implementation is partial or incomplete
- Unresolved errors encountered
- Required files or dependencies missing
- Instead: keep `in_progress` and document blocker

**LSP pre-checks for speed:**
- Use `documentSymbol` before expensive AI reviews (foundry-review)
- Use `findReferences` to assess impact before refactoring
- LSP catches structural issues in seconds vs minutes for full analysis

**Research tool defaults - let config decide:**
- Do NOT specify `timeout_per_provider` or `timeout_per_operation` - use `foundry-mcp.toml` defaults
- Do NOT specify `providers` - use configured consensus providers
- Only override when user explicitly requests different behavior
- Minimal call: `action="consensus" prompt="..." strategy="synthesize"`
<!-- foundry-sdd-end -->
