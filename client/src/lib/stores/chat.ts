import { writable, get, derived } from 'svelte/store';
import type { ChatMsg, ModelConfig, PipelineInfo, RuntimePipelineConfig, WsPayload, WsResponse, WsMetadata } from '$lib/types';
import { devMode } from './settings';

function createChatStore() {
	const messages = writable<ChatMsg[]>([
		{ user: 'Bot', msg: 'Welcome! How can I help you today?' }
	]);
	const isConnected = writable(false);
	const isStreaming = writable(false);
	const isThinking = writable(false);
	const models = writable<ModelConfig[]>([]);
	const selectedModel = writable<string>('');
	const pipelines = writable<PipelineInfo[]>([]);
	const selectedPipeline = writable<string>('');
	const nodeModelOverrides = writable<Record<string, string>>({});
	const modelStatus = writable<string>('');

	// Mutable pipeline config (cloned from preset, user can modify)
	const pipelineConfig = writable<PipelineInfo | null>(null);
	const pipelineModified = writable(false);

	let ws: WebSocket | null = null;
	const uuid = crypto.randomUUID();

	// When preset changes, clone it as the working config
	selectedPipeline.subscribe((id) => {
		const presets = get(pipelines);
		const preset = presets.find((p) => p.id === id);
		if (preset) {
			pipelineConfig.set(structuredClone(preset));
			pipelineModified.set(false);
			nodeModelOverrides.set({});
		}
	});

	function connect(url: string) {
		ws = new WebSocket(url);

		ws.onopen = () => {
			isConnected.set(true);
			const payload: WsPayload = { uuid, init: true };
			ws?.send(JSON.stringify(payload));
		};

		ws.onclose = () => {
			isConnected.set(false);
			isStreaming.set(false);
			isThinking.set(false);
		};

		ws.onerror = () => {
			isConnected.set(false);
		};

		ws.onmessage = (event) => {
			const data: WsResponse = JSON.parse(event.data);

			if (data.models) {
				models.set(data.models);
				if (data.models.length > 0 && !get(selectedModel)) {
					selectedModel.set(data.models[0].id);
				}
			}

			if (data.pipelines) {
				pipelines.set(data.pipelines);
				if (data.pipelines.length > 0 && !get(selectedPipeline)) {
					selectedPipeline.set(data.pipelines[0].id);
				}
			}

			if (data.models || data.pipelines) {
				return;
			}

			if (data.model_status !== undefined) {
				modelStatus.set(data.model_status);
				return;
			}

			if (data.on_chat_model_stream !== undefined) {
				handleStreamChunk(data.on_chat_model_stream);
				return;
			}

			if (data.on_chat_model_end) {
				handleStreamEnd(data.metadata);
			}
		};
	}

	function handleStreamChunk(chunk: string) {
		isThinking.set(false);
		messages.update((msgs) => {
			const last = msgs[msgs.length - 1];

			if (last?.user === 'Bot' && last.streaming) {
				return [
					...msgs.slice(0, -1),
					{ user: 'Bot', msg: last.msg + chunk, streaming: true }
				];
			}

			isStreaming.set(true);
			return [...msgs, { user: 'Bot', msg: chunk, streaming: true }];
		});
	}

	function handleStreamEnd(metadata?: WsMetadata) {
		isStreaming.set(false);
		isThinking.set(false);
		messages.update((msgs) => {
			const last = msgs[msgs.length - 1];
			if (last?.streaming) {
				return [...msgs.slice(0, -1), { ...last, streaming: false, metadata }];
			}
			return msgs;
		});
	}

	function toRuntimeConfig(config: PipelineInfo): RuntimePipelineConfig {
		return {
			nodes: config.nodes.map((n) => ({
				id: n.id,
				type: n.node_type,
				model: n.model,
				prompt: n.prompt
			})),
			edges: config.edges.map((e) => ({
				from: e.from,
				to: e.to,
				edge_type: e.edge_type
			}))
		};
	}

	function send(text: string) {
		if (!ws || !text.trim()) return;

		messages.update((msgs) => [...msgs, { user: 'User', msg: text }]);
		isThinking.set(true);

		const config = get(pipelineConfig);
		const modified = get(pipelineModified);
		const overrides = get(nodeModelOverrides);

		const payload: WsPayload = {
			uuid,
			message: text,
			model_id: get(selectedModel),
			verbose: get(devMode)
		};

		// Send full config if modified, otherwise just pipeline_id
		if (config && modified) {
			payload.pipeline_config = toRuntimeConfig(config);
		} else {
			payload.pipeline_id = get(selectedPipeline);
			if (Object.keys(overrides).length > 0) {
				payload.node_models = overrides;
			}
		}

		ws.send(JSON.stringify(payload));
	}

	function updateNode(nodeId: string, updates: Partial<{ prompt: string; model: string | null; node_type: string }>) {
		pipelineConfig.update((config) => {
			if (!config) return config;
			const nodes = config.nodes.map((n) =>
				n.id === nodeId ? { ...n, ...updates } : n
			);
			return { ...config, nodes };
		});
		pipelineModified.set(true);
	}

	function addNode(node: { id: string; node_type: string; prompt: string | null; model: string | null }) {
		pipelineConfig.update((config) => {
			if (!config) return config;
			return { ...config, nodes: [...config.nodes, node] };
		});
		pipelineModified.set(true);
	}

	function removeNode(nodeId: string) {
		pipelineConfig.update((config) => {
			if (!config) return config;
			const nodes = config.nodes.filter((n) => n.id !== nodeId);
			// Also remove edges referencing this node
			const edges = config.edges.filter((e) => {
				const fromIds = Array.isArray(e.from) ? e.from : [e.from];
				const toIds = Array.isArray(e.to) ? e.to : [e.to];
				return !fromIds.includes(nodeId) && !toIds.includes(nodeId);
			});
			return { ...config, nodes, edges };
		});
		pipelineModified.set(true);
	}

	function updateEdges(edges: PipelineInfo['edges']) {
		pipelineConfig.update((config) => {
			if (!config) return config;
			return { ...config, edges };
		});
		pipelineModified.set(true);
	}

	function resetPipeline() {
		const id = get(selectedPipeline);
		const preset = get(pipelines).find((p) => p.id === id);
		if (preset) {
			pipelineConfig.set(structuredClone(preset));
			pipelineModified.set(false);
		}
	}

	function addCustomPipeline(config: PipelineInfo) {
		pipelines.update((list) => [...list, config]);
		selectedPipeline.set(config.id);
	}

	function wake(modelId: string, previousModelId?: string) {
		if (!ws) return;
		const payload: WsPayload = {
			uuid,
			wake_model_id: modelId,
			unload_model_id: previousModelId
		};
		ws.send(JSON.stringify(payload));
	}

	function unload(modelId: string) {
		if (!ws) return;
		const payload: WsPayload = {
			uuid,
			unload_model_id: modelId
		};
		ws.send(JSON.stringify(payload));
	}

	function isLocalModel(modelId: string): boolean {
		const model = get(models).find((m) => m.id === modelId);
		return model?.api_base !== null && model?.api_base !== undefined;
	}

	function reset() {
		messages.set([{ user: 'Bot', msg: 'Welcome! How can I help you today?' }]);
	}

	function disconnect() {
		ws?.close();
		ws = null;
	}

	return {
		messages,
		isConnected,
		isStreaming,
		isThinking,
		models,
		selectedModel,
		pipelines,
		selectedPipeline,
		nodeModelOverrides,
		modelStatus,
		pipelineConfig,
		pipelineModified,
		connect,
		send,
		wake,
		unload,
		isLocalModel,
		reset,
		disconnect,
		updateNode,
		addNode,
		removeNode,
		updateEdges,
		resetPipeline,
		addCustomPipeline
	};
}

export const chat = createChatStore();
