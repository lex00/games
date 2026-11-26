// Fails the build if the built page lost what it exists for.
import { readFileSync } from 'node:fs';

const html = readFileSync(new URL('../dist/index.html', import.meta.url), 'utf8');
const runner = 'href="https:\\/\\/[^"]+\\/p\\?g=[a-z0-9.-]+\\/[a-z0-9.-]+"';
const play = 'class="[^"]*\\bplay\\b[^"]*"';
const need = [
	[new RegExp(`<a [^>]*${play}[^>]*${runner}|<a [^>]*${runner}[^>]*${play}`), 'a Play link to the game runner naming the game (p?g=NAMESPACE/NAME)'],
	[/<img[^>]*alt="[^"]+ in the [^"]+ look"/, "a game's picture, described as the game in its look"],
	[/href="https:\/\/lex00\.github\.io\/gameprinter\/"/, 'the link to the docs'],
	[/<body[^>]*class="[^"]*\barcade\b/, 'the arcade look on its body'],
	[/<style[^>]*>[^<]*--pink\s*:[^<]*\.tube[\s,.:{]/, 'the shared stylesheet (arcade.css) in the page'],
];
const missing = need.filter(([re]) => !re.test(html)).map(([, what]) => what);
if (missing.length) {
	console.error(`check-built: the page has no ${missing.join(', no ')}`);
	process.exit(1);
}
console.log('check-built: ok');
