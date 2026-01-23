<script lang="ts">
	import type { PipelineInfo, NodeInfo, EdgeInfo, ModelConfig } from '$lib/types';

	export let config: PipelineInfo;
	export let models: ModelConfig[];
	export let templates: PipelineInfo[] = [];
	export let onUpdate: (config: PipelineInfo) => void;
	export let onClose: () => void;
	export let onSave: (config: PipelineInfo) => void = () => {};

	function applyTemplate(templateId: string) {
		const tpl = templates.find(t => t.id === templateId);
		if (!tpl) return;
		onUpdate({ ...config, nodes: structuredClone(tpl.nodes), edges: structuredClone(tpl.edges) });
	}

	const nodeTypes = ['llm', 'worker', 'coordinator', 'aggregator', 'orchestrator', 'synthesizer', 'router', 'gate', 'evaluator'];

	let selectedNodeId: string | null = null;

	$: selectedNode = selectedNodeId ? config.nodes.find(n => n.id === selectedNodeId) : null;

	function selectNode(id: string) {
		selectedNodeId = id;
	}

	function updateNodeField(nodeId: string, field: string, value: string | null) {
		const nodes = config.nodes.map(n =>
			n.id === nodeId ? { ...n, [field]: value } : n
		);
		onUpdate({ ...config, nodes });
	}

	function addNode() {
		const id = `worker_${config.nodes.length + 1}`;
		const node: NodeInfo = { id, node_type: 'worker', model: null, prompt: 'You are a helpful assistant.' };
		onUpdate({ ...config, nodes: [...config.nodes, node] });
		selectedNodeId = id;
	}

	function removeNode(id: string) {
		const nodes = config.nodes.filter(n => n.id !== id);
		const edges = config.edges.filter(e => {
			const fromIds = Array.isArray(e.from) ? e.from : [e.from];
			const toIds = Array.isArray(e.to) ? e.to : [e.to];
			return !fromIds.includes(id) && !toIds.includes(id);
		});
		if (selectedNodeId === id) selectedNodeId = null;
		onUpdate({ ...config, nodes, edges });
	}

	function addEdge(from: string, to: string) {
		const edge: EdgeInfo = { from, to };
		onUpdate({ ...config, edges: [...config.edges, edge] });
	}

	function removeEdge(index: number) {
		const edges = config.edges.filter((_, i) => i !== index);
		onUpdate({ ...config, edges });
	}

	function formatEndpoint(ep: string | string[]): string {
		return Array.isArray(ep) ? ep.join(', ') : ep;
	}

	let newEdgeFrom = '';
	let newEdgeTo = '';
</script>

<div class="editor-container">
	<div class="editor-header">
		<input
			class="pipeline-name-input"
			type="text"
			value={config.name}
			on:input={(e) => onUpdate({ ...config, name: e.currentTarget.value })}
			placeholder="Pipeline name..."
		/>
		<div class="header-actions">
			<select class="template-select" on:change={(e) => { applyTemplate(e.currentTarget.value); e.currentTarget.value = ''; }}>
				<option value="">Apply template...</option>
				{#each templates as tpl}
					<option value={tpl.id}>{tpl.name}</option>
				{/each}
			</select>
			<button class="add-btn" on:click={addNode}>+ Add Node</button>
			<button class="save-btn" on:click={() => onSave(config)}>Save</button>
			<button class="close-btn" on:click={onClose}>Done</button>
		</div>
	</div>

	<div class="editor-body">
		<div class="nodes-panel">
			<h3>Nodes</h3>
			<div class="node-list">
				{#each config.nodes as node}
					<button
						class="node-card"
						class:selected={selectedNodeId === node.id}
						on:click={() => selectNode(node.id)}
					>
						<span class="node-id">{node.id}</span>
						<span class="node-type">{node.node_type}</span>
					</button>
				{/each}
			</div>

			<h3>Edges</h3>
			<div class="edge-list">
				{#each config.edges as edge, i}
					<div class="edge-row">
						<span>{formatEndpoint(edge.from)} → {formatEndpoint(edge.to)}</span>
						{#if edge.edge_type}
							<span class="edge-type">{edge.edge_type}</span>
						{/if}
						<button class="remove-edge-btn" on:click={() => removeEdge(i)}>×</button>
					</div>
				{/each}
				<div class="add-edge-row">
					<select bind:value={newEdgeFrom}>
						<option value="">from...</option>
						<option value="input">input</option>
						{#each config.nodes as node}
							<option value={node.id}>{node.id}</option>
						{/each}
					</select>
					<span>→</span>
					<select bind:value={newEdgeTo}>
						<option value="">to...</option>
						{#each config.nodes as node}
							<option value={node.id}>{node.id}</option>
						{/each}
						<option value="output">output</option>
					</select>
					<button
						class="add-edge-btn"
						disabled={!newEdgeFrom || !newEdgeTo}
						on:click={() => { addEdge(newEdgeFrom, newEdgeTo); newEdgeFrom = ''; newEdgeTo = ''; }}
					>+</button>
				</div>
			</div>
		</div>

		{#if selectedNode}
			<aside class="properties-panel">
				<h3>Properties: {selectedNode.id}</h3>

				<label>
					<span>ID</span>
					<input
						type="text"
						value={selectedNode.id}
						on:change={(e) => updateNodeField(selectedNode.id, 'id', e.currentTarget.value)}
					/>
				</label>

				<label>
					<span>Type</span>
					<select
						value={selectedNode.node_type}
						on:change={(e) => updateNodeField(selectedNode.id, 'node_type', e.currentTarget.value)}
					>
						{#each nodeTypes as type}
							<option value={type}>{type}</option>
						{/each}
					</select>
				</label>

				<label>
					<span>Model</span>
					<select
						value={selectedNode.model || ''}
						on:change={(e) => updateNodeField(selectedNode.id, 'model', e.currentTarget.value || null)}
					>
						<option value="">Default</option>
						{#each models as model}
							<option value={model.id}>{model.name}</option>
						{/each}
					</select>
				</label>

				<label>
					<span>Prompt</span>
					<textarea
						value={selectedNode.prompt || ''}
						on:input={(e) => updateNodeField(selectedNode.id, 'prompt', e.currentTarget.value || null)}
						placeholder="System prompt for this node..."
						rows="8"
					></textarea>
				</label>

				<button class="delete-btn" on:click={() => removeNode(selectedNode.id)}>
					Delete Node
				</button>
			</aside>
		{/if}
	</div>
</div>

<style>
	.editor-container {
		position: fixed;
		inset: 0;
		background: var(--bg, #1a1a1a);
		z-index: 1000;
		display: flex;
		flex-direction: column;
	}

	.editor-header {
		display: flex;
		justify-content: space-between;
		align-items: center;
		padding: 0.75rem 1rem;
		border-bottom: 1px solid var(--border, #333);
	}

	.pipeline-name-input {
		font-size: 1.125rem;
		font-weight: 600;
		background: transparent;
		border: 1px solid transparent;
		border-radius: 4px;
		color: var(--text, #fff);
		padding: 0.25rem 0.5rem;
	}

	.pipeline-name-input:hover,
	.pipeline-name-input:focus {
		border-color: var(--border, #333);
		outline: none;
	}

	.header-actions {
		display: flex;
		gap: 0.5rem;
	}

	.template-select {
		padding: 0.4rem 0.5rem;
		border-radius: 4px;
		border: 1px solid var(--border, #333);
		background: var(--bg-secondary, #2a2a2a);
		color: var(--text, #fff);
		font-size: 0.875rem;
		cursor: pointer;
	}

	.add-btn, .close-btn, .save-btn {
		padding: 0.4rem 0.75rem;
		border-radius: 4px;
		border: 1px solid var(--border, #333);
		background: var(--bg-secondary, #2a2a2a);
		color: var(--text, #fff);
		cursor: pointer;
		font-size: 0.875rem;
	}

	.add-btn:hover { background: #3b82f6; }
	.save-btn:hover { background: #22c55e; }
	.close-btn:hover { background: var(--border, #333); }

	.editor-body {
		flex: 1;
		display: flex;
		overflow: hidden;
	}

	.nodes-panel {
		flex: 1;
		padding: 1rem;
		overflow-y: auto;
	}

	.nodes-panel h3 {
		margin: 0 0 0.75rem 0;
		font-size: 0.875rem;
		color: var(--text-secondary, #888);
		text-transform: uppercase;
	}

	.node-list {
		display: flex;
		flex-wrap: wrap;
		gap: 0.5rem;
		margin-bottom: 1.5rem;
	}

	.node-card {
		display: flex;
		flex-direction: column;
		gap: 0.25rem;
		padding: 0.5rem 0.75rem;
		border-radius: 6px;
		border: 1px solid var(--border, #333);
		background: var(--bg-secondary, #2a2a2a);
		color: var(--text, #fff);
		cursor: pointer;
		text-align: left;
	}

	.node-card:hover { border-color: #3b82f6; }
	.node-card.selected {
		border-color: #3b82f6;
		box-shadow: 0 0 0 1px #3b82f6;
	}

	.node-id {
		font-family: monospace;
		font-weight: 600;
		font-size: 0.875rem;
	}

	.node-type {
		font-size: 0.7rem;
		color: var(--text-secondary, #888);
		text-transform: uppercase;
	}

	.edge-list {
		display: flex;
		flex-direction: column;
		gap: 0.4rem;
	}

	.edge-row {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		padding: 0.35rem 0.5rem;
		border-radius: 4px;
		background: var(--bg-secondary, #2a2a2a);
		font-size: 0.8rem;
		font-family: monospace;
	}

	.edge-type {
		font-size: 0.65rem;
		padding: 0.1rem 0.3rem;
		border-radius: 3px;
		background: #22c55e33;
		color: #22c55e;
		text-transform: uppercase;
	}

	.remove-edge-btn {
		margin-left: auto;
		background: none;
		border: none;
		color: #ef4444;
		cursor: pointer;
		font-size: 1rem;
	}

	.add-edge-row {
		display: flex;
		align-items: center;
		gap: 0.4rem;
		margin-top: 0.5rem;
	}

	.add-edge-row select {
		flex: 1;
		padding: 0.3rem;
		border-radius: 4px;
		border: 1px solid var(--border, #333);
		background: var(--bg, #1a1a1a);
		color: var(--text, #fff);
		font-size: 0.75rem;
	}

	.add-edge-btn {
		padding: 0.3rem 0.6rem;
		border-radius: 4px;
		border: 1px solid var(--border, #333);
		background: var(--bg-secondary, #2a2a2a);
		color: var(--text, #fff);
		cursor: pointer;
	}

	.add-edge-btn:disabled { opacity: 0.4; cursor: not-allowed; }

	.properties-panel {
		width: 320px;
		padding: 1rem;
		border-left: 1px solid var(--border, #333);
		background: var(--bg-secondary, #2a2a2a);
		overflow-y: auto;
	}

	.properties-panel h3 {
		margin: 0 0 1rem 0;
		font-size: 0.9rem;
	}

	.properties-panel label {
		display: flex;
		flex-direction: column;
		gap: 0.25rem;
		margin-bottom: 1rem;
	}

	.properties-panel label span {
		font-size: 0.7rem;
		color: var(--text-secondary, #888);
		text-transform: uppercase;
	}

	.properties-panel input,
	.properties-panel select,
	.properties-panel textarea {
		padding: 0.5rem;
		border-radius: 4px;
		border: 1px solid var(--border, #333);
		background: var(--bg, #1a1a1a);
		color: var(--text, #fff);
		font-family: inherit;
		font-size: 0.875rem;
	}

	.properties-panel textarea {
		resize: vertical;
		min-height: 120px;
	}

	.delete-btn {
		width: 100%;
		padding: 0.5rem;
		border-radius: 4px;
		border: 1px solid #ef4444;
		background: transparent;
		color: #ef4444;
		cursor: pointer;
		margin-top: 1rem;
	}

	.delete-btn:hover { background: #ef4444; color: white; }
</style>
