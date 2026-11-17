// The Claude adapter's normalized identity and coverage. Protocol one receives its original
// arguments and events; protocol two also receives this explicit report of native integration;
// protocol three also hears that the adapter classifies what each tool call consults.

export const HOST = 'claude-code'

/** The yard protocol from which the page server hears what each tool call consults. */
export const CONSULTS = 3

export function descriptor(version: string, protocol: number) {
  return {
    format: 'yard-adapter/1',
    version,
    capabilities: {
      events: ['turn.start', 'turn.end', 'request', 'tool.start', 'tool.end', 'subagent.start', 'subagent.end', 'compaction', 'plan.start', 'plan.end'],
      nativeQuestions: true,
      nativePlans: true,
      taskMirroring: true,
      usage: ['input', 'cacheRead', 'cacheWrite', 'output'],
      ...(protocol >= CONSULTS ? { consults: true } : {}),
    },
  }
}
