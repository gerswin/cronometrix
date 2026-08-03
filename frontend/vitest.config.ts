import { defineConfig } from 'vitest/config'
import react from '@vitejs/plugin-react'
import path from 'path'

export default defineConfig({
  plugins: [react()],
  test: {
    environment: 'jsdom',
    globals: true,
    setupFiles: ['./src/__tests__/setup.ts'],
    include: ['src/**/*.{test,spec}.{ts,tsx}'],
    exclude: ['e2e/**', 'node_modules/**', 'dist/**', '.next/**'],
    coverage: {
      provider: 'v8',
      reporter: ['text', 'html', 'lcov', 'json-summary'],
      reportsDirectory: './coverage',
      include: [
        'src/components/**/*.{ts,tsx}',
        'src/hooks/**/*.{ts,tsx}',
        'src/lib/**/*.{ts,tsx}',
      ],
      // This include/exclude pair is also the source of truth for
      // scripts/enforce-frontend-file-floor.mjs's per-file floor (loaded
      // programmatically, not copy-pasted — see the thresholds comment
      // below). Editing these globs changes what that script enforces too.
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
        // D-14 line 2 (a per-file floor of 70/60/70/70, softer than the
        // project gate above) USED to live here as a glob-keyed threshold
        // block ('**/*.{ts,tsx}': {...}). It looked right and wasn't:
        // Vitest evaluates a glob-keyed threshold over the AGGREGATE of
        // every file the glob matches, not file by file. Since the glob
        // covered the whole `include` set below, it measured roughly the
        // same thing as the project-wide gate with lower numbers, so it
        // never bound — files far under 70% still exited 0.
        // `perFile: true` doesn't fix this either: it applies the PROJECT
        // thresholds (90/85/90/90) to every file, which is stricter than
        // the intended floor, not equal to it. "project 90, per-file 70"
        // is not expressible in Vitest's threshold config.
        // The per-file floor now lives in
        // scripts/enforce-frontend-file-floor.mjs, which reads
        // coverage/coverage-summary.json (emitted by the 'json-summary'
        // reporter above) after this run finishes and fails per-file. It
        // reads `include`/`exclude` straight out of this file via Vite's
        // config loader, so they cannot drift from what's configured here.
        // See Makefile's coverage-frontend target and the "Frontend
        // Coverage" job in .github/workflows/ci.yml for how it's wired in.
      },
    },
  },
  resolve: {
    alias: { '@': path.resolve(__dirname, './src') },
  },
})
