<script lang="ts">
	import { onMount, tick } from 'svelte';
	import { goto } from '$app/navigation';
	import { chat } from '$lib/stores/chat';
	import Header from '$lib/components/Header.svelte';
	import ChatMessage from '$lib/components/ChatMessage.svelte';
	import ChatInput from '$lib/components/ChatInput.svelte';
	import type { PipelineInfo } from '$lib/types';

	const { messages, isConnected, isStreaming, isThinking, models, selectedModel, pipelines, selectedPipeline, pipelineConfig, pipelineModified, modelStatus, composeMode, composeDraft } = chat;
	const WS_URL = 'ws://localhost:8000/ws';

	let inputText = '';
	let messagesContainer: HTMLDivElement;
	let prevModel = '';
	let prevPipeline = '';

	onMount(() => {
		chat.connect(WS_URL);
		return () => chat.disconnect();
	});

	$: if ($selectedModel !== prevModel) {
		handleModelChange(prevModel, $selectedModel);
		prevModel = $selectedModel;
	}

	function handleModelChange(prev: string, next: string) {
		const prevIsLocal = prev && chat.isLocalModel(prev);
		const nextIsLocal = next && chat.isLocalModel(next);

		// Unload GPU: switching to "none"
		if (next === 'none' && prevIsLocal) {
			chat.unload(prev);
			return;
		}

		// Switching to a local model: wake it (and unload previous if also local)
		if (nextIsLocal) {
			const prevToUnload = prevIsLocal ? prev : undefined;
			chat.wake(next, prevToUnload);
			return;
		}

		// Switching from local to cloud: unload the local model
		if (prevIsLocal && !nextIsLocal) {
			chat.unload(prev);
		}
	}

	async function scrollToBottom() {
		await tick();
		if (messagesContainer) {
			messagesContainer.scrollTop = messagesContainer.scrollHeight;
		}
	}

	$: if ($messages || $isThinking) {
		scrollToBottom();
	}

	function handleSend() {
		if (!inputText.trim() || $isStreaming) return;

		const trimmed = inputText.trim().toLowerCase();

		// Handle /compose command
		if (trimmed === '/compose') {
			chat.enterComposeMode();
			inputText = '';
			return;
		}

		// Handle /done command in compose mode
		if (trimmed === '/done' && $composeMode === 'composing') {
			chat.send('/done');
			inputText = '';
			return;
		}

		chat.send(inputText);
		inputText = '';
	}

	$: if ($selectedPipeline === '__new__' && prevPipeline !== '__new__') {
		createNewPipeline();
		prevPipeline = '__new__';
	} else if ($selectedPipeline !== '__new__') {
		prevPipeline = $selectedPipeline;
	}

	function createNewPipeline() {
		const id = `custom_${Date.now()}`;
		const blank: PipelineInfo = {
			id,
			name: 'New Agent',
			description: '',
			nodes: [
				{ id: 'llm1', node_type: 'llm', model: null, prompt: 'You are a helpful assistant.' }
			],
			edges: [
				{ from: 'input', to: 'llm1' },
				{ from: 'llm1', to: 'output' }
			]
		};
		chat.selectedPipeline.set('');
		chat.pipelineConfig.set(blank);
		chat.pipelineModified.set(true);
		goto('/composer');
	}

	function openEditor() {
		goto('/composer');
	}

	function handleDeletePipeline(id: string) {
		chat.deletePipeline(id);
	}

	async function handleSaveComposed() {
		const draft = $composeDraft;
		if (!draft) return;

		const config: PipelineInfo = {
			id: draft.id || `composed_${Date.now()}`,
			name: draft.name || 'Composed Pipeline',
			description: draft.description || '',
			nodes: (draft.nodes || []).map((n) => ({
				id: n.id,
				node_type: n.node_type,
				model: n.model ?? null,
				prompt: n.prompt ?? null
			})),
			edges: draft.edges || []
		};

		await chat.savePipeline(config);
		chat.selectedPipeline.set(config.id);
		chat.exitComposeMode();
	}

	function handleCancelCompose() {
		chat.exitComposeMode();
	}
</script>

<div class="app">
	<Header
		isConnected={$isConnected}
		models={$models}
		bind:selectedModel={$selectedModel}
		pipelines={$pipelines}
		bind:selectedPipeline={$selectedPipeline}
		modelStatus={$modelStatus}
		pipelineModified={$pipelineModified}
		onEditPipeline={openEditor}
		onDeletePipeline={handleDeletePipeline}
	/>

	<main>
		<div class="messages" bind:this={messagesContainer}>
			{#each $messages as message}
				<ChatMessage
					user={message.user}
					msg={message.msg}
					streaming={message.streaming}
					metadata={message.metadata}
				/>
			{/each}
			{#if $isThinking}
				<div class="message bot thinking">
					<span class="thinking-dots">
						<span></span>
						<span></span>
						<span></span>
					</span>
				</div>
			{/if}
		</div>

		{#if $composeMode === 'composing'}
			<div class="compose-indicator">
				<span class="compose-badge">COMPOSE MODE</span>
				<span class="compose-hint">Type <code>/done</code> when design is complete</span>
			</div>
		{/if}

		{#if $composeMode === 'finalizing' && $composeDraft}
			<div class="compose-preview">
				<div class="compose-preview-header">
					<h4>{$composeDraft.name || 'Unnamed Pipeline'}</h4>
					<span class="compose-preview-meta">
						{$composeDraft.nodes?.length || 0} nodes, {$composeDraft.edges?.length || 0} edges
					</span>
				</div>
				<p class="compose-preview-desc">{$composeDraft.description || 'No description'}</p>
				<div class="compose-preview-actions">
					<button class="btn-save" onclick={handleSaveComposed}>Save & Use</button>
					<button class="btn-cancel" onclick={handleCancelCompose}>Cancel</button>
				</div>
			</div>
		{/if}

		<ChatInput
			bind:value={inputText}
			disabled={!$isConnected || $selectedModel === 'none'}
			sendDisabled={!$isConnected || $isStreaming || !inputText.trim() || $selectedModel === 'none'}
			onSend={handleSend}
		/>
	</main>
</div>

