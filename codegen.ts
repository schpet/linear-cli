import type { CodegenConfig } from '@graphql-codegen/cli';

const config: CodegenConfig = {
  schema: 'https://api.linear.app/graphql',
  documents: ['main.ts'],
  generates: {
    'generated/': {
      preset: 'client',
      plugins: [],
    },
  },
};

export default config;