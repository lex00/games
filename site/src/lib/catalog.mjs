// What the site lists, read from the public registry when the site is built,
// so a newly published game appears without an edit here. A build that can't
// read the registry fails: it must never publish an empty or stale list.
import { parse } from 'smol-toml';

const REGISTRY = process.env.GP_REGISTRY ?? 'https://raw.githubusercontent.com/lex00/gp-registry/main';
export const RUNNER = process.env.GP_RUNNER ?? 'https://nes-donkey-kong.intentius.workers.dev';

async function text(key) {
	const r = await fetch(`${REGISTRY}/${key}`);
	if (!r.ok) throw new Error(`${REGISTRY}/${key} answered ${r.status}`);
	return r.text();
}

/** path -> hash, from a package's manifest. */
function files(manifest) {
	const out = new Map();
	for (const line of manifest.split('\n')) {
		const m = line.match(/^file ([0-9a-f]{64}) \d+ (.+)$/);
		if (m) out.set(m[2], m[1]);
	}
	return out;
}

/** A `.pix` picture as an SVG's rects, run by run. */
export function pixToSvg(pix, label) {
	const lines = pix.split('\n').filter((l) => l && !l.startsWith('#'));
	const size = lines.find((l) => l.startsWith('size '));
	if (lines[0] !== 'pix 1' || !size) throw new Error(`${label}: not a pix 1 picture`);
	const [, w, h] = size.split(' ').map(Number);
	const colors = new Map();
	const rows = [];
	for (const l of lines.slice(lines.indexOf(size) + 1)) {
		const c = l.match(/^(\S) (#[0-9a-fA-F]{6}|transparent)$/);
		if (c && rows.length === 0) colors.set(c[1], c[2]);
		else rows.push(l);
	}
	if (rows.length !== h || rows.some((r) => r.length !== w)) throw new Error(`${label}: its grid isn't ${w} by ${h}`);
	let rects = '';
	rows.forEach((row, y) => {
		for (let x = 0; x < w; ) {
			let end = x + 1;
			while (end < w && row[end] === row[x]) end++;
			const fill = colors.get(row[x]);
			if (!fill) throw new Error(`${label}: no colour for ${row[x]}`);
			if (fill !== 'transparent') rects += `<rect x="${x}" y="${y}" width="${end - x}" height="1" fill="${fill}"/>`;
			x = end;
		}
	});
	return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${w} ${h}" shape-rendering="crispEdges" role="img" aria-label="${label}">${rects}</svg>`;
}

export async function games() {
	const catalog = parse(await text('catalog.toml')).package ?? [];
	const list = [];
	for (const p of catalog.filter((c) => c.kind === 'game')) {
		const have = files(await text(`packages/${p.id}`));
		const pkg = parse(await text(`blobs/${have.get('package.toml')}`));
		const title = p.title;
		// A game's themes, with those from the game's own namespace first (for
		// lex00/donkey-kong, lex00/turnips before a theme someone else made), then
		// by title. The first is the look the card shows.
		const own = (c) => c.name.split('/')[0] === p.name.split('/')[0];
		const themes = catalog
			.filter((c) => c.kind === 'content' && c.for_games.includes(p.name))
			.sort((a, b) => own(b) - own(a) || a.title.localeCompare(b.title));
		// The card's picture: the first theme's preview.png (a theme's own art,
		// never a picture of the original's), served from the registry.
		let picture = null;
		for (const c of themes) {
			const tf = files(await text(`packages/${c.id}`));
			if (tf.has('preview.png')) {
				picture = { src: `${REGISTRY}/blobs/${tf.get('preview.png')}`, alt: `The title screen of ${title} in the ${c.title} look` };
				break;
			}
		}
		list.push({
			name: p.name,
			title,
			version: p.latest,
			// The page's plain address: it plays a ROM the visitor loads, or one kept
			// from an earlier visit, in the page's own theme.
			play: `${RUNNER}/`,
			picture,
			original: pkg.rom ? p.title : null,
			// The package wraps its text at a width; only a blank line is a break.
			howToGet: pkg.rom?.how_to_get?.trim().replace(/([^\n])\n(?=[^\n])/g, '$1 ') ?? null,
			themes: themes.map((c) => ({ title: c.title, play: `${RUNNER}/p?g=${p.name}&c=${c.name}` })),
		});
	}
	if (list.length === 0) throw new Error('the registry lists no games');
	return list;
}
