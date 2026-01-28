import { onMount, createSignal, createResource, For, Show } from 'solid-js';
import { A } from '@solidjs/router';

interface TraceRecord {
  trace_id: string;
  pipeline_id: string;
  pipeline_name: string;
  timestamp: number;
  input: string;
  output: string;
  total_elapsed_ms: number;
  total_input_tokens: number;
  total_output_tokens: number;
  total_tool_calls: number;
  status: string;
}

interface SpanRecord {
  span_id: string;
  trace_id: string;
  node_id: string;
  node_type: string;
  start_time: number;
  end_time: number;
  input: string;
  output: string;
  input_tokens: number;
  output_tokens: number;
  tool_call_count: number;
  iteration_count: number;
}

interface MetricsSummary {
  total_traces: number;
  total_input_tokens: number;
  total_output_tokens: number;
  total_tool_calls: number;
  avg_latency_ms: number;
}

const fetchTraces = async (): Promise<TraceRecord[]> => {
  const res = await fetch('/api/traces?limit=50');
  const data = await res.json();
  return data.traces;
};

const fetchMetrics = async (): Promise<MetricsSummary> => {
  const res = await fetch('/api/metrics/summary');
  return res.json();
};

const fetchTraceDetail = async (traceId: string): Promise<{ trace: TraceRecord; spans: SpanRecord[] }> => {
  const res = await fetch(`/api/traces/${traceId}`);
  return res.json();
};

const formatTimestamp = (ts: number): string => {
  return new Date(ts).toLocaleString();
};

const formatDuration = (ms: number): string => {
  if (ms < 1000) return `${ms}ms`;
  return `${(ms / 1000).toFixed(2)}s`;
};

const truncate = (str: string, len: number): string => {
  return str.length > len ? str.slice(0, len) + '...' : str;
};

export default function Observe() {
  const [traces, { refetch: refetchTraces }] = createResource(fetchTraces);
  const [metrics] = createResource(fetchMetrics);
  const [selectedTraceId, setSelectedTraceId] = createSignal<string | null>(null);
  const [traceDetail, setTraceDetail] = createSignal<{ trace: TraceRecord; spans: SpanRecord[] } | null>(null);

  const selectTrace = async (traceId: string) => {
    setSelectedTraceId(traceId);
    const detail = await fetchTraceDetail(traceId);
    setTraceDetail(detail);
  };

  const closeDetail = () => {
    setSelectedTraceId(null);
    setTraceDetail(null);
  };

  const statusColor = (status: string): string => {
    const colors: Record<string, string> = {
      success: 'bg-green-500/20 text-green-400',
      error: 'bg-red-500/20 text-red-400',
      running: 'bg-yellow-500/20 text-yellow-400',
    };
    return colors[status] || 'bg-gray-500/20 text-gray-400';
  };

  return (
    <div class="min-h-screen bg-zinc-950 text-zinc-100">
      {/* Header */}
      <header class="border-b border-zinc-800 px-6 py-4">
        <div class="flex items-center justify-between">
          <div class="flex items-center gap-4">
            <A href="/" class="text-zinc-400 hover:text-zinc-200">
              &larr; Back
            </A>
            <h1 class="text-xl font-semibold">Observability</h1>
          </div>
          <button
            onClick={() => refetchTraces()}
            class="px-3 py-1.5 bg-zinc-800 hover:bg-zinc-700 rounded text-sm"
          >
            Refresh
          </button>
        </div>
      </header>

      <div class="p-6">
        {/* Metrics Summary */}
        <Show when={metrics()}>
          {(m) => (
            <div class="grid grid-cols-5 gap-4 mb-6">
              <div class="bg-zinc-900 border border-zinc-800 rounded-lg p-4">
                <div class="text-zinc-500 text-sm">Total Traces</div>
                <div class="text-2xl font-semibold">{m().total_traces}</div>
              </div>
              <div class="bg-zinc-900 border border-zinc-800 rounded-lg p-4">
                <div class="text-zinc-500 text-sm">Input Tokens</div>
                <div class="text-2xl font-semibold">{m().total_input_tokens.toLocaleString()}</div>
              </div>
              <div class="bg-zinc-900 border border-zinc-800 rounded-lg p-4">
                <div class="text-zinc-500 text-sm">Output Tokens</div>
                <div class="text-2xl font-semibold">{m().total_output_tokens.toLocaleString()}</div>
              </div>
              <div class="bg-zinc-900 border border-zinc-800 rounded-lg p-4">
                <div class="text-zinc-500 text-sm">Tool Calls</div>
                <div class="text-2xl font-semibold">{m().total_tool_calls}</div>
              </div>
              <div class="bg-zinc-900 border border-zinc-800 rounded-lg p-4">
                <div class="text-zinc-500 text-sm">Avg Latency</div>
                <div class="text-2xl font-semibold">{formatDuration(m().avg_latency_ms)}</div>
              </div>
            </div>
          )}
        </Show>

        <div class="flex gap-6">
          {/* Traces List */}
          <div class="flex-1">
            <h2 class="text-lg font-medium mb-4">Recent Traces</h2>
            <div class="bg-zinc-900 border border-zinc-800 rounded-lg overflow-hidden">
              <table class="w-full text-sm">
                <thead class="bg-zinc-800/50">
                  <tr>
                    <th class="text-left px-4 py-3 font-medium text-zinc-400">Time</th>
                    <th class="text-left px-4 py-3 font-medium text-zinc-400">Pipeline</th>
                    <th class="text-left px-4 py-3 font-medium text-zinc-400">Input</th>
                    <th class="text-right px-4 py-3 font-medium text-zinc-400">Tokens</th>
                    <th class="text-right px-4 py-3 font-medium text-zinc-400">Latency</th>
                    <th class="text-center px-4 py-3 font-medium text-zinc-400">Status</th>
                  </tr>
                </thead>
                <tbody>
                  <Show when={traces()} fallback={<tr><td colspan="6" class="px-4 py-8 text-center text-zinc-500">Loading...</td></tr>}>
                    <For each={traces()} fallback={<tr><td colspan="6" class="px-4 py-8 text-center text-zinc-500">No traces yet</td></tr>}>
                      {(trace) => (
                        <tr
                          class="border-t border-zinc-800 hover:bg-zinc-800/50 cursor-pointer"
                          classList={{ 'bg-zinc-800/30': selectedTraceId() === trace.trace_id }}
                          onClick={() => selectTrace(trace.trace_id)}
                        >
                          <td class="px-4 py-3 text-zinc-400">{formatTimestamp(trace.timestamp)}</td>
                          <td class="px-4 py-3">{trace.pipeline_name}</td>
                          <td class="px-4 py-3 text-zinc-400">{truncate(trace.input, 40)}</td>
                          <td class="px-4 py-3 text-right">{trace.total_input_tokens + trace.total_output_tokens}</td>
                          <td class="px-4 py-3 text-right">{formatDuration(trace.total_elapsed_ms)}</td>
                          <td class="px-4 py-3 text-center">
                            <span class={`px-2 py-0.5 rounded text-xs ${statusColor(trace.status)}`}>
                              {trace.status}
                            </span>
                          </td>
                        </tr>
                      )}
                    </For>
                  </Show>
                </tbody>
              </table>
            </div>
          </div>

          {/* Trace Detail Panel */}
          <Show when={traceDetail()}>
            {(detail) => (
              <div class="w-96 bg-zinc-900 border border-zinc-800 rounded-lg">
                <div class="flex items-center justify-between px-4 py-3 border-b border-zinc-800">
                  <h3 class="font-medium">Trace Detail</h3>
                  <button onClick={closeDetail} class="text-zinc-500 hover:text-zinc-300">&times;</button>
                </div>
                <div class="p-4 space-y-4 max-h-[600px] overflow-y-auto">
                  {/* Trace Info */}
                  <div class="space-y-2 text-sm">
                    <div class="flex justify-between">
                      <span class="text-zinc-500">Pipeline</span>
                      <span>{detail().trace.pipeline_name}</span>
                    </div>
                    <div class="flex justify-between">
                      <span class="text-zinc-500">Status</span>
                      <span class={`px-2 py-0.5 rounded text-xs ${statusColor(detail().trace.status)}`}>
                        {detail().trace.status}
                      </span>
                    </div>
                    <div class="flex justify-between">
                      <span class="text-zinc-500">Total Tokens</span>
                      <span>{detail().trace.total_input_tokens + detail().trace.total_output_tokens}</span>
                    </div>
                    <div class="flex justify-between">
                      <span class="text-zinc-500">Latency</span>
                      <span>{formatDuration(detail().trace.total_elapsed_ms)}</span>
                    </div>
                  </div>

                  {/* Input/Output */}
                  <div>
                    <div class="text-zinc-500 text-xs mb-1">Input</div>
                    <div class="bg-zinc-800 rounded p-2 text-sm">{detail().trace.input}</div>
                  </div>
                  <div>
                    <div class="text-zinc-500 text-xs mb-1">Output</div>
                    <div class="bg-zinc-800 rounded p-2 text-sm max-h-40 overflow-y-auto">{truncate(detail().trace.output, 500)}</div>
                  </div>

                  {/* Spans */}
                  <div>
                    <div class="text-zinc-500 text-xs mb-2">Execution Timeline</div>
                    <div class="space-y-2">
                      <For each={detail().spans}>
                        {(span) => (
                          <div class="bg-zinc-800 rounded p-3 text-sm">
                            <div class="flex items-center justify-between mb-2">
                              <span class="font-medium">{span.node_id}</span>
                              <span class="text-xs text-zinc-500">{span.node_type}</span>
                            </div>
                            <div class="grid grid-cols-3 gap-2 text-xs text-zinc-400">
                              <div>
                                <span class="text-zinc-600">Tokens:</span> {span.input_tokens + span.output_tokens}
                              </div>
                              <div>
                                <span class="text-zinc-600">Time:</span> {formatDuration(span.end_time - span.start_time)}
                              </div>
                              <div>
                                <span class="text-zinc-600">Tools:</span> {span.tool_call_count}
                              </div>
                            </div>
                            <Show when={span.input}>
                              <div class="mt-2 text-xs">
                                <span class="text-zinc-600">In:</span> {truncate(span.input, 100)}
                              </div>
                            </Show>
                            <Show when={span.output}>
                              <div class="mt-1 text-xs">
                                <span class="text-zinc-600">Out:</span> {truncate(span.output, 100)}
                              </div>
                            </Show>
                          </div>
                        )}
                      </For>
                    </div>
                  </div>
                </div>
              </div>
            )}
          </Show>
        </div>
      </div>
    </div>
  );
}
