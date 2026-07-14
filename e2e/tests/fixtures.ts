import { test as base } from '@playwright/test';

// Seed the API key into localStorage before each test so the WEIR-T-0087 auth gate
// passes (the UI sends it as `Authorization: Bearer`). The server-start harness mints
// a key and exports it as WEIR_E2E_KEY.
export const test = base.extend({
  page: async ({ page }, use) => {
    const key = process.env.WEIR_E2E_KEY;
    if (key) {
      await page.addInitScript((k) => localStorage.setItem('weir_api_key', k as string), key);
    }
    await use(page);
  },
});

export { expect } from '@playwright/test';
