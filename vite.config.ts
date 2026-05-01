import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'

export default defineConfig({
  plugins: [react(), tailwindcss()],
  build: {
    rollupOptions: {
      output: {
        manualChunks(id) {
          if (!id.includes('node_modules')) return
          if (id.includes('react') || id.includes('scheduler')) {
            return 'vendor-react'
          }
          if (id.includes('@tiptap') || id.includes('prosemirror') || id.includes('linkifyjs')) {
            return 'vendor-editor'
          }
          if (id.includes('@tauri-apps')) {
            return 'vendor-tauri'
          }
          return 'vendor'
        },
      },
    },
  },
})
