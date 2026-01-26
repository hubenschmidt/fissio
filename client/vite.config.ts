import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

export default defineConfig({
	plugins: [sveltekit()],
	server: {
		port: 3001,
		strictPort: true
	},
	optimizeDeps: {
		include: ['@dagrejs/dagre', '@dagrejs/graphlib']
	},
	ssr: {
		noExternal: ['@dagrejs/dagre', '@dagrejs/graphlib']
	}
});
