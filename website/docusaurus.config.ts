import {themes as prismThemes} from 'prism-react-renderer';
import type {Config} from '@docusaurus/types';
import type * as Preset from '@docusaurus/preset-classic';

const config: Config = {
  title: 'NexusQL',
  tagline: 'AI-Native Database with Brain-Inspired Computing',
  favicon: 'img/favicon.ico',

  url: 'https://nguyenetic.github.io',
  baseUrl: '/NexusQL/',

  organizationName: 'Nguyenetic',
  projectName: 'NexusQL',

  onBrokenLinks: 'warn',
  onBrokenMarkdownLinks: 'warn',

  i18n: {
    defaultLocale: 'en',
    locales: ['en'],
  },

  markdown: {
    format: 'detect', // Auto-detect MD vs MDX
  },

  presets: [
    [
      'classic',
      {
        docs: {
          sidebarPath: './sidebars.ts',
          editUrl: 'https://github.com/Nguyenetic/NexusQL/edit/master/website/',
          showLastUpdateTime: true,
          showLastUpdateAuthor: true,
        },
        blog: {
          showReadingTime: true,
          editUrl: 'https://github.com/Nguyenetic/NexusQL/edit/master/website/',
          blogTitle: 'NexusQL Blog',
          blogDescription: 'Updates, tutorials, and insights about NexusQL',
        },
        theme: {
          customCss: './src/css/custom.css',
        },
      } satisfies Preset.Options,
    ],
  ],

  themeConfig: {
    image: 'img/docusaurus-social-card.jpg',
    navbar: {
      title: 'NexusQL',
      logo: {
        alt: 'NexusQL Logo',
        src: 'img/logo.svg',
      },
      items: [
        {
          type: 'docSidebar',
          sidebarId: 'tutorialSidebar',
          position: 'left',
          label: 'Documentation',
        },
        {
          to: '/docs/api/typescript/README',
          label: 'API Reference',
          position: 'left',
        },
        {
          to: '/blog',
          label: 'Blog',
          position: 'left'
        },
        {
          href: 'https://www.npmjs.com/package/@nguyenetic/nexusql',
          label: 'npm',
          position: 'right',
        },
        {
          href: 'https://github.com/Nguyenetic/NexusQL',
          label: 'GitHub',
          position: 'right',
        },
      ],
    },
    footer: {
      style: 'dark',
      links: [
        {
          title: 'Documentation',
          items: [
            {
              label: 'Getting Started',
              to: '/docs/guides/getting-started',
            },
            {
              label: 'API Reference',
              to: '/docs/api/typescript/README',
            },
            {
              label: 'Architecture',
              to: '/docs/architecture/README',
            },
          ],
        },
        {
          title: 'Community',
          items: [
            {
              label: 'GitHub Discussions',
              href: 'https://github.com/Nguyenetic/NexusQL/discussions',
            },
            {
              label: 'GitHub Issues',
              href: 'https://github.com/Nguyenetic/NexusQL/issues',
            },
          ],
        },
        {
          title: 'More',
          items: [
            {
              label: 'npm Package',
              href: 'https://www.npmjs.com/package/@nguyenetic/nexusql',
            },
            {
              label: 'GitHub',
              href: 'https://github.com/Nguyenetic/NexusQL',
            },
            {
              label: 'Contributing',
              to: '/docs/CONTRIBUTING',
            },
            {
              label: 'Security',
              to: '/docs/SECURITY',
            },
          ],
        },
      ],
      copyright: `Copyright © ${new Date().getFullYear()} NexusQL. Built with Docusaurus.`,
    },
    prism: {
      theme: prismThemes.github,
      darkTheme: prismThemes.dracula,
      additionalLanguages: ['rust', 'typescript', 'javascript', 'bash', 'sql'],
    },
  } satisfies Preset.ThemeConfig,
};

export default config;
