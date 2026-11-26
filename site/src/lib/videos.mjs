// A game's video, by the game's name in the registry: the id of a video on
// YouTube that shows it played. The list shows the video's own picture with
// a play mark, linking to the video there; nothing of YouTube's runs on the
// page. A game with no entry shows no video.
export const VIDEOS = {
	'lex00/tapper': { id: 'PIKpRCtSEK8', says: 'Watch it played in the Last Call look' },
};

/** The video of game `name`: where it plays, its picture and its words, or null. */
export function videoOf(name) {
	const v = VIDEOS[name];
	if (!v || !/^[A-Za-z0-9_-]{11}$/.test(v.id)) return null;
	return {
		href: `https://www.youtube.com/watch?v=${v.id}`,
		picture: `https://i.ytimg.com/vi/${v.id}/maxresdefault.jpg`,
		says: v.says,
	};
}
