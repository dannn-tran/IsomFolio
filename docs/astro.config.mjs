// @ts-check
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

export default defineConfig({
	integrations: [
		starlight({
			title: 'IsomFolio',
			description: 'Photo library manager for macOS. Fast, local, AI-optional.',
			social: [
				{ icon: 'github', label: 'GitHub', href: 'https://github.com/picas9dan/isomfolio' },
			],
			sidebar: [
				{
					label: 'Getting Started',
					items: [
						{ label: 'Introduction', slug: 'getting-started/introduction' },
						{ label: 'Installation', slug: 'getting-started/installation' },
						{ label: 'Your First Catalog', slug: 'getting-started/first-catalog' },
					],
				},
				{
					label: 'User Guide',
					items: [
						{ label: 'Interface Overview', slug: 'guide/interface' },
						{ label: 'Browsing Photos', slug: 'guide/browsing' },
						{ label: 'Culling — Ratings & Flags', slug: 'guide/culling' },
						{ label: 'Tagging', slug: 'guide/tagging' },
						{ label: 'Albums & Smart Albums', slug: 'guide/albums' },
						{ label: 'Searching & Filtering', slug: 'guide/search' },
						{ label: 'People (Face Recognition)', slug: 'guide/people' },
						{ label: 'Keyboard Shortcuts', slug: 'guide/keyboard-shortcuts' },
					],
				},
				{
					label: 'Extensions',
					items: [
						{ label: 'What are Extensions?', slug: 'extensions/overview' },
						{ label: 'Installing Extensions', slug: 'extensions/installing' },
						{ label: 'Auto-Tagging with CLIP', slug: 'extensions/autotag-clip' },
						{ label: 'Auto-Tagging with OpenAI', slug: 'extensions/autotag-openai' },
						{ label: 'Face Clustering', slug: 'extensions/face-clustering' },
					],
				},
				{
					label: 'Reference',
					items: [
						{ label: 'Catalog Format', slug: 'reference/catalog-format' },
						{ label: 'Supported Formats', slug: 'reference/supported-formats' },
						{ label: 'FAQ', slug: 'reference/faq' },
						{ label: 'Comparison', slug: 'reference/comparison' },
					],
				},
			],
			customCss: ['./src/styles/custom.css'],
		}),
	],
});
