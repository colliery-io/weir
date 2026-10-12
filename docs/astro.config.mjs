import { existsSync } from 'node:fs';
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';
import starlightAurora from 'starlight-aurora';
import oldAddresses from './redirects.mjs';

// The Pages workflow sets DOCS_SITE and DOCS_BASE. On a pull request and
// locally they are empty, and the site is at /.
const site = process.env.DOCS_SITE || undefined;
const base = process.env.DOCS_BASE || '/';

const PROJECT = 'weir';
const REPO = 'https://github.com/colliery-io/weir';

// `plissken render` writes the API reference and its sidebar group. The pages
// are not in git: without a render, the site builds without them.
const API_SIDEBAR = new URL('./src/content/docs/reference/api/_sidebar.mjs', import.meta.url);
const apiSidebar = existsSync(API_SIDEBAR) ? [(await import(API_SIDEBAR.href)).default] : [];

export default defineConfig({
	site,
	base,
	integrations: [
		starlight({
			title: PROJECT,
			description: 'An open-source, no-code data ingestion & reverse-ETL platform.',
			social: [{ icon: 'github', label: 'GitHub', href: REPO }],
			plugins: [starlightAurora({ hyper: true })],
			expressiveCode: { frames: { extractFileNameFromCode: false } },
			sidebar: [
				{ label: 'Home', link: '/' },
				{
					label: 'Tutorials',
					items: [{ label: 'Your first sync', slug: 'tutorials/first-sync' }],
				},
				{
					label: 'How-to guides',
					items: [
						{ label: 'Onboard a declarative connector', slug: 'how-to/onboard-manifest-connector' },
						{ label: 'Schedule syncs', slug: 'how-to/schedule-syncs' },
						{ label: 'Capture changes + propagate deletes', slug: 'how-to/cdc-deletes' },
						{ label: 'Secure the control plane', slug: 'how-to/secure-control-plane' },
						{ label: 'Give a secret as a reference', slug: 'how-to/secret-references' },
						{ label: 'Manage tenants', slug: 'how-to/manage-tenants' },
						{ label: 'Deploy to Kubernetes', slug: 'how-to/deploy-kubernetes' },
						{ label: 'Map fields', slug: 'how-to/field-mapping' },
						{ label: 'Author a connector', slug: 'how-to/connector-authoring' },
						{ label: 'Soak testing', slug: 'how-to/soak-testing' },
						{ label: 'Demo pipelines', slug: 'how-to/demo-pipelines' },
						{ label: 'First-hour triage', slug: 'how-to/first-hour-triage' },
					],
				},
				{
					label: 'Reference',
					items: [
						{ label: 'Installation', slug: 'reference/installation' },
						{ label: 'CLI', slug: 'reference/cli' },
						{ label: 'Connection config', slug: 'reference/connection-config' },
						{ label: 'Connector contract', slug: 'reference/connector-contract' },
						{ label: 'HTTP API', slug: 'reference/http-api' },
						...apiSidebar,
					],
				},
				{
					label: 'Explanation',
					items: [
						{ label: 'Architecture', slug: 'explanation/architecture' },
						{ label: 'The connector model', slug: 'explanation/connector-model' },
						{ label: 'Multi-tenancy', slug: 'explanation/multi-tenancy' },
						{ label: 'Tenancy posture (alpha)', slug: 'explanation/tenancy-posture' },
						{ label: 'Secrets posture', slug: 'explanation/secrets-posture' },
						{ label: 'Delivery guarantees', slug: 'explanation/delivery-guarantees' },
						{ label: 'Typed schemas', slug: 'explanation/typed-schemas' },
						{ label: 'Deployment topology', slug: 'explanation/deployment' },
					],
				},
			],
		}),
		oldAddresses(),
	],
});
