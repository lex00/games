// Fails the build if the built page lost what it exists for.
import { readFileSync } from 'node:fs';

const html = readFileSync(new URL('../dist/index.html', import.meta.url), 'utf8');
const need = [
	[/href="https:\/\/[^"]+\/p\?g=[^"]+"/, 'a Play link to the game runner'],
	[/<svg[^>]*aria-label="The title screen of /, "a game's picture"],
	[/href="https:\/\/lex00\.github\.io\/gameprinter\/"/, 'the link to the docs'],
];
const missing = need.filter(([re]) => !re.test(html)).map(([, what]) => what);
if (missing.length) {
	console.error(`check-built: the page has no ${missing.join(', no ')}`);
	process.exit(1);
}
console.log('check-built: ok');
