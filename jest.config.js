/**
 * Jest configuration for EarnProof contract bindings tests
 *
 * Runs TypeScript and JavaScript tests in artifacts/bindings/__tests__/
 */

module.exports = {
  displayName: 'bindings',
  testEnvironment: 'node',
  roots: ['<rootDir>/artifacts/bindings'],
  testMatch: ['**/__tests__/**/*.test.ts'],
  moduleFileExtensions: ['ts', 'tsx', 'js', 'jsx', 'json', 'node'],

  transform: {
    '^.+\\.tsx?$': [
      'ts-jest',
      {
        tsconfig: {
          target: 'ES2020',
          module: 'commonjs',
          lib: ['ES2020'],
          declaration: false,
          strict: true,
          esModuleInterop: true,
          skipLibCheck: true,
          forceConsistentCasingInFileNames: true,
        },
      },
    ],
  },

  collectCoverageFrom: [
    'artifacts/bindings/**/*.ts',
    '!artifacts/bindings/**/*.test.ts',
    '!artifacts/bindings/__tests__/**',
    '!artifacts/bindings/index.ts', // Re-export file
  ],

  coveragePathIgnorePatterns: [
    '/node_modules/',
    '__tests__',
  ],

  collectCoverage: false, // Enable with --coverage flag
  coverageThreshold: {
    global: {
      branches: 80,
      functions: 80,
      lines: 80,
      statements: 80,
    },
  },

  // Quiet mode for cleaner output
  verbose: true,

  // Timeout for integration tests
  testTimeout: 30000,
};
