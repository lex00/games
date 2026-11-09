import { defineConfig } from 'astro/config';

// The list of games, at lex00.github.io/games. Each Play button links to the
// game runner, which is a separate page: see src/lib/catalog.mjs.
export default defineConfig({
	site: process.env.GP_SITE_ORIGIN ?? 'https://lex00.github.io',
	base: process.env.GP_SITE_BASE ?? '/games',
});
