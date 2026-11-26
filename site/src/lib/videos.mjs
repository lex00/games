// A game's video, by the game's name in the registry: the id of a video on
// YouTube that shows it played. The list shows the video's own picture with
// a play mark, linking to the video there; nothing of YouTube's runs on the
// page. A game with no entry shows no video.
// `look` is the theme the video shows; every card says the same words of it.
export const VIDEOS = {
	'lex00/tapper': { id: 'PIKpRCtSEK8', look: 'Last Call' },
	'lex00/donkey-kong': { id: '9nsA4A4e_x8', look: 'Turnips' },
};

/** The video of game `name`: where it plays, its picture and its words, or null. */
export function videoOf(name) {
	const v = VIDEOS[name];
	if (!v || !/^[A-Za-z0-9_-]{11}$/.test(v.id)) return null;
	return {
		href: `https://www.youtube.com/watch?v=${v.id}`,
		picture: `https://i.ytimg.com/vi/${v.id}/maxresdefault.jpg`,
		says: `Watch it played in the ${v.look} look`,
	};
}
