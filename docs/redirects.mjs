// The MkDocs site had the how-to guides at guides/<page>/, the HTTP API page
// at api/, and the plissken API reference at api/weir/ and api/rust/<crate>/.
// After the build, this writes a page at each old address that sends the
// reader to the new one. Each target is relative, so the pages work under any
// base path. It never writes over a real page.
import { existsSync } from 'node:fs';
import { mkdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { redirectPage } from 'starlight-aurora/mdbook-redirects';

export default function oldAddresses() {
	return {
		name: 'weir-old-addresses',
		hooks: {
			'astro:build:done': async ({ dir, pages, logger }) => {
				const out = fileURLToPath(dir);
				const map = { 'api/': 'reference/http-api/' };
				for (const { pathname } of pages) {
					if (pathname.startsWith('how-to/')) map[`guides/${pathname.slice('how-to/'.length)}`] = pathname;
					if (pathname.startsWith('reference/api/')) map[pathname.slice('reference/'.length)] = pathname;
				}
				let written = 0;
				for (const [from, to] of Object.entries(map)) {
					const file = join(out, from, 'index.html');
					if (existsSync(file)) continue;
					const up = '../'.repeat(from.split('/').length - 1);
					await mkdir(join(out, from), { recursive: true });
					await writeFile(file, redirectPage(up + to));
					written++;
				}
				logger.info(`${written} redirects from the old addresses`);
			},
		},
	};
}
