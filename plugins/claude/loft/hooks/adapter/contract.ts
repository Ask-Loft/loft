// The Claude adapter's normalized identity and coverage. Protocol one receives its original
// arguments and events; protocol two also receives this explicit report of native integration.

export const HOST = 'claude-code'

export function descriptor(version: string) {
  return {
    format: 'yard-adapter/1',
    version,
    capabilities: {
      events: ['turn.start', 'turn.end', 'request', 'tool.start', 'tool.end', 'subagent.start', 'subagent.end', 'compaction', 'plan.start', 'plan.end'],
      nativeQuestions: true,
      nativePlans: true,
      taskMirroring: true,
      usage: ['input', 'cacheRead', 'cacheWrite', 'output'],
    },
  }
}
