// The arcade look's stylesheet, shared with the game page. The game page
// serves it at RUNNER/arcade.css; the site takes that copy when it is built,
// so a change to the game page's look reaches the list with the next build.
// When the game page can't be read, or what it answers isn't the stylesheet,
// the copy kept in src/styles/arcade.css is used, so the site always builds.
import kept from '../styles/arcade.css?raw';
import { RUNNER } from './catalog.mjs';

const TOKENS = [
	'--bg', '--card', '--ink', '--muted', '--pink', '--cyan', '--yellow', '--green', '--red', '--violet',
	'--display', '--text', '--mono', '--fs-small', '--fs', '--fs-btn', '--fs-key', '--btn-h',
];
const CLASSES = ['.arcade', '.tube', '.sign', '.card', '.label'];

/** Why `css` is not the stylesheet, or null when it is. */
export function notTheStylesheet(css) {
	if (typeof css !== 'string' || css.length === 0) return 'it is empty';
	if (css.length > 200_000) return 'it is over 200 kB';
	const noToken = TOKENS.find((t) => !new RegExp(`${t}\\s*:`).test(css));
	if (noToken) return `it has no ${noToken}`;
	const noClass = CLASSES.find((c) => !new RegExp(`\\${c}[\\s,.:{]`).test(css));
	if (noClass) return `it has no ${noClass}`;
	// It is written into the page's own style element, and the page asks no
	// other address for anything.
	if (/<\/?\s*style|<\s*script|<!--/i.test(css)) return 'it holds markup';
	if (/@import|url\s*\(/i.test(css)) return 'it asks for another file';
	return null;
}

let taken;

/** The stylesheet's text and where it came from. Read once a build. */
export function arcade() {
	taken ??= take();
	return taken;
}

async function take() {
	const from = `${RUNNER}/arcade.css`;
	let why;
	try {
		const r = await fetch(from, { signal: AbortSignal.timeout(10_000), redirect: 'error' });
		if (!r.ok) why = `it answered ${r.status}`;
		else {
			const css = await r.text();
			why = notTheStylesheet(css);
			if (!why) {
				console.log(`arcade.css: the game page's copy, from ${from}`);
				return { css, from };
			}
		}
	} catch (e) {
		why = e?.name === 'TimeoutError' ? 'it did not answer in 10 s' : `it could not be read (${e?.cause?.code ?? e?.message ?? e})`;
	}
	const css = kept;
	const bad = notTheStylesheet(css);
	if (bad) throw new Error(`src/styles/arcade.css is not the stylesheet: ${bad}`);
	console.log(`arcade.css: the kept copy, src/styles/arcade.css (${from}: ${why})`);
	return { css, from: 'src/styles/arcade.css' };
}
