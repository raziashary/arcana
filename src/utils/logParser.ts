export type LogLevel = 'ERROR' | 'WARN' | 'INFO' | 'DEBUG' | 'TRACE' | 'PANIC' | 'UNKNOWN'

export interface ParsedLogLine {
  raw: string
  timestamp: string
  level: LogLevel
  module: string
  message: string
}

/** Parse tracing-subscriber compact output without splitting Rust `::` targets. */
export function parseLogLine(raw: string): ParsedLogLine {
  const match = raw.match(/^(\S+)\s+(ERROR|WARN|INFO|DEBUG|TRACE)\s+(.+?):\s+(.*)$/)
  if (match) {
    return {
      raw,
      timestamp: match[1].replace('T', ' ').replace('Z', ''),
      level: match[2] as LogLevel,
      module: match[3].trim(),
      message: match[4].trim(),
    }
  }

  if (raw.includes('[PANIC]')) {
    return { raw, timestamp: '', level: 'PANIC', module: 'runtime', message: raw }
  }

  return { raw, timestamp: '', level: 'UNKNOWN', module: '', message: raw }
}
