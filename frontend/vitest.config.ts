import { defineConfig } from 'vitest/config'
import react from '@vitejs/plugin-react'
import path from 'path'

export default defineConfig({
  plugins: [react()],
  test: {
    environment: 'jsdom',
    globals: true,
    setupFiles: ['./src/__tests__/setup.ts'],
    coverage: {
      provider: 'v8',
      reporter: ['text', 'html', 'lcov'],
      reportsDirectory: './coverage',
      include: [
        'src/components/**/*.{ts,tsx}',
        'src/hooks/**/*.{ts,tsx}',
        'src/lib/**/*.{ts,tsx}',
      ],
      exclude: [
        'src/components/ui/**',          // shadcn vendored copies (D-10)
        'src/components/providers.tsx',  // D-09: pure QueryClientProvider wrapper, no logic
        'src/components/layout/top-bar.tsx', // D-09: pure display, no logic
        'src/components/common/access-restricted.tsx', // D-09: pure display, no logic
        'src/**/__tests__/**',
        'src/**/*.test.{ts,tsx}',
        'src/**/*.spec.{ts,tsx}',
        'src/**/*.d.ts',
      ],
      thresholds: {
        // Project-wide gate — D-14 line 1
        lines: 90,
        branches: 85,
        functions: 90,
        statements: 90,
        // Per-file floor — D-14 line 2 (softer than project gate)
        '**/*.{ts,tsx}': {
          lines: 70,
          branches: 60,
          functions: 70,
          statements: 70,
        },
      },
    },
  },
  resolve: {
    alias: { '@': path.resolve(__dirname, './src') },
  },
})
