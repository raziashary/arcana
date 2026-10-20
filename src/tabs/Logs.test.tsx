import { beforeEach, describe, expect, it, vi } from 'vitest'

const invoke = vi.hoisted(() => vi.fn())
const save = vi.hoisted(() => vi.fn(async () => 'C:/diagnostics/arcana-full.log'))
vi.mock('@tauri-apps/api/core', () => ({ invoke }))
vi.mock('@tauri-apps/plugin-dialog', () => ({ save }))

import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import LogsTab from './Logs'

describe('Logs tab', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    HTMLElement.prototype.scrollIntoView = vi.fn()
    invoke.mockImplementation(async (command: string) => {
      if (command === 'list_log_files') return ['2026-09-28']
      if (command === 'read_log_file') {
        return {
          content: '2026-09-28T10:12:13.123Z  INFO arcana_core::agent::engine: worker task finished',
          total_lines: 3_005,
          truncated: true,
          full_size_bytes: 192_000,
        }
      }
      if (command === 'read_structured_events') {
        return {
          content: `${JSON.stringify({ timestamp: '2026-09-28T10:12:14Z', target: 'arcana::telemetry', fields: { event_type: 'database_statement', database: 'tasks', operation: 'read', duration_ns: 1_250_000 }, spans: [{ name: 'worker_tick' }] })}\n`,
          total_lines: 1,
          truncated: false,
          full_size_bytes: 210,
        }
      }
      if (command === 'summarize_observability_events') return {
        total_events: 1, malformed_events: 0, provider_calls: 0, provider_attempts: 0,
        tool_calls: 0, database_statements: 1, readable_writer_failures: 0,
        structured_writer_failures: 0, by_event: {},
      }
      if (command === 'export_log_file') return 'C:/diagnostics/arcana-full.log'
      if (command === 'export_observability_bundle') return 'C:/diagnostics/arcana-observability.json'
      throw new Error(`Unexpected command: ${command}`)
    })
  })

  it('renders structured Rust targets, shows refresh size, and exports the complete file', async () => {
    render(<LogsTab />)

    expect(await screen.findByText('worker task finished')).toBeInTheDocument()
    expect(screen.getByText('arcana_core::agent::engine')).toBeInTheDocument()
    expect(screen.getByText(/refresh .* ms · 192,000 bytes/)).toBeInTheDocument()

    fireEvent.click(screen.getByRole('button', { name: 'Full' }))
    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith('export_log_file', {
        date: '2026-09-28',
        destPath: 'C:/diagnostics/arcana-full.log',
        full: true,
        tailLines: null,
      })
    })
  })

  it('exposes four expandable observability sections and exports the complete date bundle', async () => {
    render(<LogsTab />)

    await waitFor(() => expect(invoke).toHaveBeenCalledWith('read_structured_events', {
      date: '2026-09-28',
      tailLines: 2000,
    }))
    await waitFor(() => expect(invoke).toHaveBeenCalledWith('summarize_observability_events', { date: '2026-09-28' }))
    const sections = Array.from(document.querySelectorAll('summary')).map(item => item.textContent ?? '')
    expect(sections.some(label => label.startsWith('Structured Events'))).toBe(true)
    expect(sections.some(label => label.startsWith('Traces'))).toBe(true)
    expect(sections.some(label => label.startsWith('Metrics'))).toBe(true)
    expect(screen.getByRole('button', { name: 'Export observability' })).toBeInTheDocument()

    fireEvent.click(screen.getByRole('button', { name: 'Export observability' }))
    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith('export_observability_bundle', {
        date: '2026-09-28',
        destPath: 'C:/diagnostics/arcana-full.log',
      })
    })
  })

  it('keeps readable logs visible when the structured telemetry reader fails', async () => {
    invoke.mockImplementation(async (command: string) => {
      if (command === 'list_log_files') return ['2026-09-28']
      if (command === 'read_log_file') return {
        content: '2026-09-28T10:12:13.123Z  INFO arcana_core::agent::engine: readable log survives',
        total_lines: 1,
        truncated: false,
        full_size_bytes: 80,
      }
      if (command === 'read_structured_events') throw new Error('structured store unavailable')
      if (command === 'summarize_observability_events') return { total_events: 0, by_event: {} }
      throw new Error(`Unexpected command: ${command}`)
    })

    render(<LogsTab />)

    expect(await screen.findByText('readable log survives')).toBeInTheDocument()
    fireEvent.click(screen.getByText(/Structured Events/))
    expect(await screen.findByRole('alert')).toHaveTextContent('structured store unavailable')
  })

  it('warns when the structured event tail is truncated and names the full export path', async () => {
    invoke.mockImplementation(async (command: string) => {
      if (command === 'list_log_files') return ['2026-09-28']
      if (command === 'read_log_file') return { content: '', total_lines: 0, truncated: false, full_size_bytes: 0 }
      if (command === 'read_structured_events') return {
        content: `${JSON.stringify({ fields: { event_type: 'fleet_cli_run_completed' } })}\n`,
        total_lines: 2_421,
        truncated: true,
        full_size_bytes: 5_000_000,
      }
      if (command === 'summarize_observability_events') return { total_events: 2_421, by_event: {} }
      if (command === 'export_observability_bundle') return 'C:/diagnostics/arcana-observability.json'
      throw new Error(`Unexpected command: ${command}`)
    })

    render(<LogsTab />)

    expect(await screen.findByRole('status')).toHaveTextContent('newest 2,000 structured events of 2,421')
    fireEvent.click(screen.getByRole('button', { name: 'Export observability' }))
    await waitFor(() => expect(invoke).toHaveBeenCalledWith('export_observability_bundle', {
      date: '2026-09-28',
      destPath: 'C:/diagnostics/arcana-full.log',
    }))
  })

  it('renders span ancestry as a nested trace tree with operation duration', async () => {
    invoke.mockImplementation(async (command: string) => {
      if (command === 'list_log_files') return ['2026-09-28']
      if (command === 'read_log_file') return { content: '', total_lines: 0, truncated: false, full_size_bytes: 0 }
      if (command === 'read_structured_events') return {
        content: `${JSON.stringify({
          timestamp: '2026-09-28T10:12:14Z',
          target: 'arcana::telemetry',
          fields: { event_type: 'operation_timing', operation: 'provider_stream', duration_ns: 1_250_000, task_id: 42 },
          spans: [{ name: 'fleet_tick' }, { name: 'worker_run' }],
        })}\n`,
        total_lines: 1,
        truncated: false,
        full_size_bytes: 240,
      }
      if (command === 'summarize_observability_events') return { total_events: 1, by_event: {} }
      throw new Error(`Unexpected command: ${command}`)
    })

    render(<LogsTab />)
    fireEvent.click(await screen.findByText(/Traces/))

    const root = await screen.findByText('task 42')
    const rootDetails = root.closest('details')
    expect(rootDetails).toContainElement(screen.getByText('fleet_tick'))
    const parent = screen.getByText('fleet_tick')
    const parentDetails = parent.closest('details')
    expect(parentDetails).toContainElement(screen.getByText('worker_run'))
    expect(parentDetails).toHaveTextContent('provider_stream · task 42')
    expect(parentDetails).toHaveTextContent('1.25 ms')
  })

  it('aggregates sub-operation count, wall-clock window, and slowest duration per trace node', async () => {
    const timing = (timestamp: string, spans: Array<Record<string, unknown>>, operation: string, duration_ns: number) => ({
      timestamp,
      target: 'arcana::telemetry',
      fields: { event_type: 'operation_timing', operation, duration_ns, fleet_correlation_id: 'fleet-agg' },
      spans,
    })
    const records = [
      timing('2026-10-20T10:00:00.100Z', [{ name: 'fleet_tick', span_id: 'tick-1' }, { name: 'worker_run', span_id: 'worker-1' }], 'provider_call', 2_000_000),
      timing('2026-10-20T10:00:00.500Z', [{ name: 'fleet_tick', span_id: 'tick-1' }, { name: 'worker_run', span_id: 'worker-1' }], 'tool_call', 6_000_000),
      timing('2026-10-20T10:00:01.100Z', [{ name: 'fleet_tick', span_id: 'tick-1' }], 'database_statement', 1_000_000),
    ]
    invoke.mockImplementation(async (command: string) => {
      if (command === 'list_log_files') return ['2026-10-20']
      if (command === 'read_log_file') return { content: '', total_lines: 0, truncated: false, full_size_bytes: 0 }
      if (command === 'read_structured_events') return {
        content: `${records.map(record => JSON.stringify(record)).join('\n')}\n`,
        total_lines: records.length,
        truncated: false,
        full_size_bytes: 900,
      }
      if (command === 'summarize_observability_events') return { total_events: records.length, by_event: {} }
      throw new Error(`Unexpected command: ${command}`)
    })

    render(<LogsTab />)
    fireEvent.click(await screen.findByText(/Traces/))

    const rootDetails = (await screen.findByText('fleet fleet-agg')).closest('details')
    expect(rootDetails).toHaveTextContent('3 ops')
    expect(rootDetails).toHaveTextContent('1000.00 ms window')
    expect(rootDetails).toHaveTextContent('slowest 6.00 ms')

    // The worker subtree aggregates only its own two calls, so a root total is
    // not silently copied down onto every child.
    const workerDetails = screen.getByText('worker_run').closest('details')
    expect(workerDetails).toHaveTextContent('2 ops')
    expect(workerDetails).toHaveTextContent('400.00 ms window')
    expect(workerDetails).toHaveTextContent('slowest 6.00 ms')
  })

  it('renders the persisted Memory search operation in Structured Events and Traces', async () => {
    const event = {
      timestamp: '2026-10-04T10:12:14Z',
      target: 'arcana::telemetry',
      fields: {
        event_type: 'operation_timing',
        operation: 'memory_search_all',
        database: 'memory',
        call_id: 'call-memory-observability',
        run_id: 'run-memory-observability',
        task_id: '42',
        outcome: 'fallback',
        result_count: 1,
        duration_ns: 1_250_000,
      },
      spans: [{ name: 'worker_tick' }],
    }
    invoke.mockImplementation(async (command: string) => {
      if (command === 'list_log_files') return ['2026-10-04']
      if (command === 'read_log_file') return { content: '', total_lines: 0, truncated: false, full_size_bytes: 0 }
      if (command === 'read_structured_events') return {
        content: `${JSON.stringify(event)}\n`, total_lines: 1, truncated: false, full_size_bytes: 400,
      }
      if (command === 'summarize_observability_events') return {
        total_events: 1,
        malformed_events: 0,
        provider_calls: 0,
        provider_attempts: 0,
        tool_calls: 0,
        database_statements: 0,
        readable_writer_failures: 0,
        structured_writer_failures: 0,
        by_event: { operation_timing: { count: 1, failures: 0, result_count_total: 1, duration_count: 1 } },
      }
      throw new Error(`Unexpected command: ${command}`)
    })

    render(<LogsTab />)
    fireEvent.click(await screen.findByText(/Structured Events/))
    expect(await screen.findAllByText(/memory_search_all/)).not.toHaveLength(0)
    expect(screen.getAllByText(/call-memory-observability/)).not.toHaveLength(0)

    fireEvent.click(screen.getByText(/Traces/))
    const call = await screen.findByText('call call-memory-observability')
    const trace = call.closest('details')
    expect(trace).toHaveTextContent('memory_search_all')
    expect(trace).toHaveTextContent('fallback')
    expect(trace).toHaveTextContent('1.25 ms')
  })

  it('groups cancelled project-command stage timings by call and shows their outcome', async () => {
    invoke.mockImplementation(async (command: string) => {
      if (command === 'list_log_files') return ['2026-10-03']
      if (command === 'read_log_file') return { content: '', total_lines: 0, truncated: false, full_size_bytes: 0 }
      if (command === 'read_structured_events') return {
        content: `${JSON.stringify({
          timestamp: '2026-10-03T10:12:14Z',
          target: 'arcana::telemetry',
          fields: {
            event_type: 'project_command_stage', stage: 'process_wait', outcome: 'incomplete',
            duration_ns: 300_000_000, call_id: 'command-call-1', task_id: 42, session_id: 'session-1',
          },
        })}\n`,
        total_lines: 1,
        truncated: false,
        full_size_bytes: 260,
      }
      if (command === 'summarize_observability_events') return { total_events: 1, by_event: {} }
      throw new Error(`Unexpected command: ${command}`)
    })

    render(<LogsTab />)
    fireEvent.click(await screen.findByText(/Traces/))

    const call = await screen.findByText('call command-call-1')
    const trace = call.closest('details')
    expect(trace).toHaveTextContent('project_command_stage · process_wait · incomplete · task 42 · session session-1 · call command-call-1')
    expect(trace).toHaveTextContent('300.00 ms')

    fireEvent.change(screen.getByRole('searchbox', { name: 'Filter traces by correlation' }), {
      target: { value: 'command-call-1' },
    })
    expect(await screen.findByText('call command-call-1')).toBeInTheDocument()
  })

  it('groups and filters production flattened SQLite span fields by task', async () => {
    invoke.mockImplementation(async (command: string) => {
      if (command === 'list_log_files') return ['2026-09-28']
      if (command === 'read_log_file') return { content: '', total_lines: 0, truncated: false, full_size_bytes: 0 }
      if (command === 'read_structured_events') return {
        content: `${JSON.stringify({
          timestamp: '2026-09-28T10:12:14Z',
          target: 'arcana::telemetry',
          fields: { event_type: 'database_statement', database: 'tasks', operation: 'update', duration_ns: 1_250_000, outcome: 'duration_observed' },
          spans: [{ name: 'database_task_assignment', database: 'tasks', event_type: 'trace_span', operation: 'database_task_assign_with_routing', task_id: 72, worker: 'worker-a' }],
        })}\n`,
        total_lines: 1,
        truncated: false,
        full_size_bytes: 420,
      }
      if (command === 'summarize_observability_events') return { total_events: 1, by_event: {} }
      throw new Error(`Unexpected command: ${command}`)
    })

    render(<LogsTab />)
    fireEvent.click(await screen.findByText(/Traces/))
    const taskRoot = await screen.findByText('task 72')
    const rootDetails = taskRoot.closest('details')
    expect(rootDetails).toContainElement(screen.getByText('database_task_assignment'))
    expect(rootDetails).toHaveTextContent('update · duration_observed · task 72 · worker worker-a')

    fireEvent.change(screen.getByRole('searchbox', { name: 'Filter traces by correlation' }), {
      target: { value: '72' },
    })
    expect(await screen.findByText('task 72')).toBeInTheDocument()
  })

  it('joins graph parent timing and phase events under the shared trace id', async () => {
    invoke.mockImplementation(async (command: string) => {
      if (command === 'list_log_files') return ['2026-09-28']
      if (command === 'read_log_file') return { content: '', total_lines: 0, truncated: false, full_size_bytes: 0 }
      if (command === 'read_structured_events') return {
        content: [
          { fields: { event_type: 'operation_timing', operation: 'graph_build', trace_id: 'graph-run-1', duration_ns: 4_000_000, outcome: 'success' } },
          { fields: { event_type: 'graph_build_phase', phase: 'detect', trace_id: 'graph-run-1', duration_ns: 1_000_000, outcome: 'success' } },
        ].map(record => JSON.stringify(record)).join('\n'),
        total_lines: 2,
        truncated: false,
        full_size_bytes: 300,
      }
      if (command === 'summarize_observability_events') return { total_events: 2, by_event: {} }
      throw new Error(`Unexpected command: ${command}`)
    })

    render(<LogsTab />)
    fireEvent.click(await screen.findByText(/Traces/))
    const root = await screen.findByText('trace graph-run-1')
    const details = root.closest('details')
    expect(details).toHaveTextContent('graph_build')
    expect(details).toHaveTextContent('graph_build_phase')
    expect(details).toHaveTextContent('detect')
  })

  it('keeps trace trees separate across runs and filters by correlation', async () => {
    invoke.mockImplementation(async (command: string) => {
      if (command === 'list_log_files') return ['2026-09-28']
      if (command === 'read_log_file') return { content: '', total_lines: 0, truncated: false, full_size_bytes: 0 }
      if (command === 'read_structured_events') return {
        content: [
          { fields: { event_type: 'operation_timing', operation: 'worker_tick', duration_ns: 10, run_id: 'run-a' }, spans: [{ name: 'fleet_tick' }] },
          { fields: { event_type: 'operation_timing', operation: 'worker_tick', duration_ns: 20, run_id: 'run-b' }, spans: [{ name: 'fleet_tick' }] },
        ].map(record => JSON.stringify(record)).join('\n'),
        total_lines: 2,
        truncated: false,
        full_size_bytes: 300,
      }
      if (command === 'summarize_observability_events') return { total_events: 2, by_event: {} }
      throw new Error(`Unexpected command: ${command}`)
    })

    render(<LogsTab />)
    fireEvent.click(await screen.findByText(/Traces/))
    expect(await screen.findByText('run run-a')).toBeInTheDocument()
    expect(screen.getByText('run run-b')).toBeInTheDocument()
    fireEvent.change(screen.getByRole('searchbox', { name: 'Filter traces by correlation' }), { target: { value: 'run-b' } })
    expect(screen.getByText('run run-b')).toBeInTheDocument()
    expect(screen.queryByText('run run-a')).not.toBeInTheDocument()
  })

  it('keeps repeated same-named spans as separate operations within one run', async () => {
    invoke.mockImplementation(async (command: string) => {
      if (command === 'list_log_files') return ['2026-09-28']
      if (command === 'read_log_file') return { content: '', total_lines: 0, truncated: false, full_size_bytes: 0 }
      if (command === 'read_structured_events') return {
        content: [
          { fields: { event_type: 'operation_timing', operation: 'database_task_read', duration_ns: 1_000_000, run_id: 'run-a' }, spans: [{ name: 'worker_run' }] },
          { fields: { event_type: 'operation_timing', operation: 'database_task_read', duration_ns: 2_000_000, run_id: 'run-a' }, spans: [{ name: 'worker_run' }] },
        ].map(record => JSON.stringify(record)).join('\n'),
        total_lines: 2,
        truncated: false,
        full_size_bytes: 300,
      }
      if (command === 'summarize_observability_events') return { total_events: 2, by_event: {} }
      throw new Error(`Unexpected command: ${command}`)
    })

    render(<LogsTab />)
    fireEvent.click(await screen.findByText(/Traces/))

    const spans = await screen.findAllByText('worker_run')
    expect(spans).toHaveLength(2)
    expect(spans[0].closest('details')).toHaveTextContent('1.00 ms')
    expect(spans[0].closest('details')).not.toHaveTextContent('2.00 ms')
    expect(spans[1].closest('details')).toHaveTextContent('2.00 ms')
  })

  it('re-reads with the latest correlation filter when a filter changes during a load', async () => {
    let finishFirstRead!: (value: { content: string; total_lines: number; truncated: boolean; full_size_bytes: number }) => void
    let structuredReads = 0
    invoke.mockImplementation(async (command: string) => {
      if (command === 'list_log_files') return ['2026-09-28']
      if (command === 'read_log_file') return { content: '', total_lines: 0, truncated: false, full_size_bytes: 0 }
      if (command === 'read_structured_events') {
        structuredReads += 1
        if (structuredReads === 1) {
          return await new Promise(resolve => { finishFirstRead = resolve })
        }
        return { content: '', total_lines: 0, truncated: false, full_size_bytes: 0 }
      }
      if (command === 'summarize_observability_events') return { total_events: 0, by_event: {} }
      throw new Error(`Unexpected command: ${command}`)
    })

    render(<LogsTab />)
    fireEvent.click(await screen.findByText(/Traces/))
    await waitFor(() => expect(screen.getByTitle('Select log date')).toHaveValue('2026-09-28'))
    const filter = screen.getByRole('textbox', { name: 'Filter logs by run or correlation ID' })
    fireEvent.change(filter, { target: { value: 'run-latest' } })
    fireEvent.click(screen.getByRole('button', { name: 'Filter run' }))

    finishFirstRead({ content: '', total_lines: 0, truncated: false, full_size_bytes: 0 })
    await waitFor(() => expect(invoke).toHaveBeenCalledWith('read_structured_events', {
      date: '2026-09-28',
      tailLines: 2000,
      correlationId: 'run-latest',
    }))
    expect(structuredReads).toBe(2)
  })

  it('keeps calls under the same task and session in separate trace roots', async () => {
    invoke.mockImplementation(async (command: string) => {
      if (command === 'list_log_files') return ['2026-09-28']
      if (command === 'read_log_file') return { content: '', total_lines: 0, truncated: false, full_size_bytes: 0 }
      if (command === 'read_structured_events') return {
        content: [
          {
            fields: { event_type: 'operation_timing', operation: 'provider_stream_a', duration_ns: 10, task_id: 42, session_id: 'session-1', call_id: 'call-a' },
            spans: [{ name: 'agent_call' }, { name: 'provider_stream_a' }],
          },
          {
            fields: { event_type: 'operation_timing', operation: 'provider_stream_b', duration_ns: 20, task_id: 42, session_id: 'session-1', call_id: 'call-b' },
            spans: [{ name: 'agent_call' }, { name: 'provider_stream_b' }],
          },
        ].map(record => JSON.stringify(record)).join('\n'),
        total_lines: 2,
        truncated: false,
        full_size_bytes: 400,
      }
      if (command === 'summarize_observability_events') return { total_events: 2, by_event: {} }
      throw new Error(`Unexpected command: ${command}`)
    })

    render(<LogsTab />)
    fireEvent.click(await screen.findByText(/Traces/))

    const callA = await screen.findByText('call call-a')
    const callB = screen.getByText('call call-b')
    const traceA = callA.closest('details')
    const traceB = callB.closest('details')

    expect(traceA).toHaveTextContent('provider_stream_a · task 42 · session session-1 · call call-a')
    expect(traceA).not.toHaveTextContent('provider_stream_b')
    expect(traceB).toHaveTextContent('provider_stream_b · task 42 · session session-1 · call call-b')
    expect(traceB).not.toHaveTextContent('provider_stream_a')
  })

  it('groups CLI correlation_id with the matching worker fleet_correlation_id', async () => {
    invoke.mockImplementation(async (command: string) => {
      if (command === 'list_log_files') return ['2026-09-28']
      if (command === 'read_log_file') return { content: '', total_lines: 0, truncated: false, full_size_bytes: 0 }
      if (command === 'read_structured_events') return {
        content: [
          { fields: { event_type: 'fleet_cli_run_started', correlation_id: 'fleet-run-7' }, spans: [{ name: 'cli_fleet' }] },
          { fields: { event_type: 'operation_timing', operation: 'worker_tick', fleet_correlation_id: 'fleet-run-7' }, spans: [{ name: 'worker_tick' }] },
          { fields: { event_type: 'tool_call_result', operation: 'write_file', call_id: 'tool-call-7', fleet_correlation_id: 'fleet-run-7' }, spans: [{ name: 'worker_tick' }] },
        ].map(record => JSON.stringify(record)).join('\n'),
        total_lines: 3,
        truncated: false,
        full_size_bytes: 320,
      }
      if (command === 'summarize_observability_events') return { total_events: 2, by_event: {} }
      throw new Error(`Unexpected command: ${command}`)
    })

    render(<LogsTab />)
    fireEvent.click(await screen.findByText(/Traces/))

    const root = await screen.findByText('fleet fleet-run-7')
    const rootDetails = root.closest('details')
    expect(rootDetails).toContainElement(screen.getByText('cli_fleet'))
    expect(rootDetails).toContainElement(screen.getAllByText('worker_tick')[0])
    expect(rootDetails).toContainElement(screen.getByText('call tool-call-7'))
    expect(screen.queryByText('correlation fleet-run-7')).not.toBeInTheDocument()
  })

  it('counts denied tool outcomes in the failed-outcomes metric', async () => {
    invoke.mockImplementation(async (command: string) => {
      if (command === 'list_log_files') return ['2026-09-28']
      if (command === 'read_log_file') return { content: '', total_lines: 0, truncated: false, full_size_bytes: 0 }
      if (command === 'read_structured_events') return { content: '', total_lines: 0, truncated: false, full_size_bytes: 0 }
      if (command === 'summarize_observability_events') return {
        total_events: 1,
        outcomes: { denied: 1 },
        by_event: { tool_call_result: { count: 1, failures: 1 } },
      }
      throw new Error(`Unexpected command: ${command}`)
    })

    render(<LogsTab />)
    fireEvent.click(await screen.findByText(/Metrics/))

    const metricLabel = await screen.findByText('Failed outcomes')
    expect(metricLabel.parentElement).toHaveTextContent('1')
  })

  it('includes fallback outcomes in the failed-outcomes metric', async () => {
    invoke.mockImplementation(async (command: string) => {
      if (command === 'list_log_files') return ['2026-09-28']
      if (command === 'read_log_file') return { content: '', total_lines: 0, truncated: false, full_size_bytes: 0 }
      if (command === 'read_structured_events') return { content: '', total_lines: 0, truncated: false, full_size_bytes: 0 }
      if (command === 'summarize_observability_events') return {
        total_events: 3,
        outcomes: { failure: 2, fallback: 1 },
        by_event: { build_evidence_persistence: { count: 3, failures: 3 } },
      }
      throw new Error(`Unexpected command: ${command}`)
    })

    render(<LogsTab />)
    fireEvent.click(await screen.findByText(/Metrics/))

    const metricLabel = await screen.findByText('Failed outcomes')
    expect(metricLabel.parentElement).toHaveTextContent('3')
  })

  it('counts production tool and MCP failures from event aggregates', async () => {
    invoke.mockImplementation(async (command: string) => {
      if (command === 'list_log_files') return ['2026-09-28']
      if (command === 'read_log_file') return { content: '', total_lines: 0, truncated: false, full_size_bytes: 0 }
      if (command === 'read_structured_events') return { content: '', total_lines: 0, truncated: false, full_size_bytes: 0 }
      if (command === 'summarize_observability_events') return {
        total_events: 2,
        outcomes: { unspecified: 1, connection_refused: 1 },
        by_event: {
          tool_call_result: { count: 1, failures: 1 },
          mcp_reconnect_attempt: { count: 1, failures: 1 },
        },
      }
      throw new Error(`Unexpected command: ${command}`)
    })

    render(<LogsTab />)
    fireEvent.click(await screen.findByText(/Metrics/))

    const metricLabel = await screen.findByText('Failed outcomes')
    expect(metricLabel.parentElement).toHaveTextContent('2')
  })

  it('reports zero-duration aggregates as below timer resolution', async () => {
    invoke.mockImplementation(async (command: string) => {
      if (command === 'list_log_files') return ['2026-09-28']
      if (command === 'read_log_file') return { content: '', total_lines: 0, truncated: false, full_size_bytes: 0 }
      if (command === 'read_structured_events') return { content: '', total_lines: 0, truncated: false, full_size_bytes: 0 }
      if (command === 'summarize_observability_events') return {
        total_events: 2,
        by_event: { sqlite_read: { count: 1, duration_count: 0, duration_zero_count: 1, duration_total_ns: 0, duration_max_ns: 0 } },
        by_provider_model: { 'openai/gpt-test': { calls: 1, duration_count: 0, duration_zero_count: 1, duration_total_ns: 0, duration_max_ns: 0, result_blocks_total: 5 } },
      }
      throw new Error(`Unexpected command: ${command}`)
    })

    render(<LogsTab />)
    fireEvent.click(await screen.findByText(/Metrics/))

    expect(await screen.findAllByText('below timer resolution (1 samples)')).toHaveLength(2)
    expect(screen.getByText('5 result blocks')).toBeInTheDocument()
    expect(screen.queryByText('avg 0.00 ms')).not.toBeInTheDocument()
    expect(screen.queryByText('max 0.00 ms')).not.toBeInTheDocument()
  })

  it('filters structured events and metrics by a fleet correlation ID and carries it to export', async () => {
    render(<LogsTab />)
    await waitFor(() => expect(invoke).toHaveBeenCalledWith('summarize_observability_events', { date: '2026-09-28' }))

    fireEvent.change(screen.getByRole('textbox', { name: 'Filter logs by run or correlation ID' }), {
      target: { value: 'fleet-run-7' },
    })
    fireEvent.click(screen.getByRole('button', { name: 'Filter run' }))

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith('read_structured_events', {
        date: '2026-09-28',
        tailLines: 2000,
        correlationId: 'fleet-run-7',
      })
      expect(invoke).toHaveBeenCalledWith('summarize_observability_events', {
        date: '2026-09-28',
        correlationId: 'fleet-run-7',
      })
    })
    expect(screen.getByText(/selected correlation/)).toBeInTheDocument()
    expect(screen.getByText(/Selected-correlation totals/)).toBeInTheDocument()

    fireEvent.click(screen.getByRole('button', { name: 'Export observability' }))
    await waitFor(() => expect(invoke).toHaveBeenCalledWith('export_observability_bundle', {
      date: '2026-09-28',
      destPath: 'C:/diagnostics/arcana-full.log',
      correlationId: 'fleet-run-7',
    }))
  })

  it('shows process-lifetime log writer failures in the metrics section', async () => {
    invoke.mockImplementation(async (command: string) => {
      if (command === 'list_log_files') return ['2026-09-28']
      if (command === 'read_log_file') return { content: '', total_lines: 0, truncated: false, full_size_bytes: 0 }
      if (command === 'read_structured_events') return { content: '', total_lines: 0, truncated: false, full_size_bytes: 0 }
      if (command === 'summarize_observability_events') return {
        total_events: 0, malformed_events: 0, readable_writer_failures: 2,
        structured_writer_failures: 3, by_event: {},
      }
      throw new Error(`Unexpected command: ${command}`)
    })

    render(<LogsTab />)
    fireEvent.click(await screen.findByText(/Metrics/))
    expect(await screen.findByText('Readable writer errors')).toBeInTheDocument()
    expect(screen.getByText('Structured writer errors')).toBeInTheDocument()
    expect(screen.getByText('2')).toBeInTheDocument()
    expect(screen.getByText('3')).toBeInTheDocument()
    expect(screen.getByText(/writer errors are process-lifetime totals/)).toBeInTheDocument()
  })

  it('reports a missing file sink instead of an empty day when no log file exists', async () => {
    invoke.mockImplementation(async (command: string) => {
      if (command === 'list_log_files') return []
      if (command === 'summarize_observability_events') return {
        total_events: 0, malformed_events: 0, readable_writer_failures: 0,
        structured_writer_failures: 0, file_logging_unavailable: true, by_event: {},
      }
      throw new Error(`Unexpected command: ${command}`)
    })

    render(<LogsTab />)

    expect(await screen.findByRole('status')).toHaveTextContent('File logging unavailable in this process — console only')
    expect(screen.queryByText(/No log files found/)).not.toBeInTheDocument()
  })

  it('keeps the restart advice when the log directory exists and is merely empty today', async () => {
    invoke.mockImplementation(async (command: string) => {
      if (command === 'list_log_files') return []
      if (command === 'summarize_observability_events') return {
        total_events: 0, malformed_events: 0, readable_writer_failures: 0,
        structured_writer_failures: 0, file_logging_unavailable: false, by_event: {},
      }
      throw new Error(`Unexpected command: ${command}`)
    })

    render(<LogsTab />)

    expect(await screen.findByText(/No log files found/)).toBeInTheDocument()
    expect(screen.queryByText(/File logging unavailable/)).not.toBeInTheDocument()
    expect(screen.queryByText(/telemetry line/)).not.toBeInTheDocument()
  })

  it('warns that records are missing when the writer queue dropped telemetry', async () => {
    invoke.mockImplementation(async (command: string) => {
      if (command === 'list_log_files') return ['2026-10-20']
      if (command === 'read_log_file') return { content: '', total_lines: 0, truncated: false, full_size_bytes: 0 }
      if (command === 'read_structured_events') return { content: '', total_lines: 0, truncated: false, full_size_bytes: 0 }
      if (command === 'summarize_observability_events') return {
        total_events: 0, malformed_events: 0, readable_writer_failures: 0,
        structured_writer_failures: 0, file_logging_unavailable: false,
        readable_dropped_lines: 2, structured_dropped_lines: 3, by_event: {},
      }
      throw new Error(`Unexpected command: ${command}`)
    })

    render(<LogsTab />)

    // A dropped fragment never reaches the writer, so no failure counter moves;
    // without this warning the view would report a healthy, quiet host.
    expect(await screen.findByText(/5 telemetry lines dropped by the writer queue/)).toBeInTheDocument()
  })
})
