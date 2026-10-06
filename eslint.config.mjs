// Lint configuration (run with any ESLint 9+: `eslint src tests`). Kept minimal: unused code and undefined names.
export default [
  { ignores: ['node_modules/**', 'legacy/**', 'CadForge.html', 'src/solid/vendor/**'] },
  {
    files: ['**/*.js', '**/*.mjs'],
    languageOptions: {
      ecmaVersion: 2023, sourceType: 'module',
      globals: Object.fromEntries(['window', 'document', 'navigator', 'localStorage', 'console', 'setTimeout', 'clearTimeout', 'requestAnimationFrame', 'ResizeObserver', 'MutationObserver', 'Blob', 'URL', 'Worker', 'self', 'HTMLInputElement', 'HTMLTextAreaElement', 'HTMLSelectElement', 'Node', 'MouseEvent', 'structuredClone', 'innerWidth', 'innerHeight', 'File', 'TextEncoder', 'performance', 'process', 'Buffer'].map(n => [n, 'readonly'])),
    },
    rules: { 'no-unused-vars': ['warn', { args: 'none', caughtErrors: 'none', varsIgnorePattern: '^_' }], 'no-undef': 'error' },
  },
];
