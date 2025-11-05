import type {SidebarsConfig} from '@docusaurus/plugin-content-docs';

const sidebars: SidebarsConfig = {
  tutorialSidebar: [
    {
      type: 'doc',
      id: 'intro',
      label: 'Introduction',
    },
    {
      type: 'category',
      label: 'Getting Started',
      items: [
        'guides/getting-started',
      ],
    },
    {
      type: 'category',
      label: 'Guides',
      items: [
        'guides/parameterized-queries',
        'guides/performance',
      ],
    },
    {
      type: 'category',
      label: 'API Reference',
      items: [
        'api/typescript/README',
        'api/typescript/CLASSES',
        'api/typescript/METHODS',
        'api/typescript/INTERFACES',
        'api/typescript/EXAMPLES',
      ],
    },
    {
      type: 'category',
      label: 'Architecture',
      items: [
        'architecture/README',
        'architecture/neural-technologies',
      ],
    },
    {
      type: 'category',
      label: 'Development',
      items: [
        'development/testing-security',
        'CONTRIBUTING',
      ],
    },
    {
      type: 'doc',
      id: 'SECURITY',
      label: 'Security Policy',
    },
    {
      type: 'doc',
      id: 'ROADMAP',
      label: 'Roadmap',
    },
  ],
};

export default sidebars;
