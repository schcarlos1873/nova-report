import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

// Configuração recomendada pelo Tauri 2: porta fixa e sem limpar a tela do terminal.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 1420, strictPort: true, watch: { ignored: ['**/src-tauri/**'] }, fs: { allow: ['.', '../../examples'] } },
  envPrefix: ['VITE_', 'TAURI_ENV_'],
  build: { target: 'safari15', sourcemap: false, chunkSizeWarningLimit: 1500 },
});
