-- The controls and RAM gameprinter's MAME capture script reads on this
-- game's romsets (gameprinter's scripts/oracle/mame-capture.lua, which
-- runs this file with dofile; `just mame-record` mounts it). It returns a
-- table from each MAME system name to its table. Facts from MAME's source
-- (mcr.cpp and midway_sound.cpp), nothing from the ROM.
--
-- Each control is a port tag and a bit mask from mcr.cpp's
-- INPUT_PORTS_START( tapper ); all are active low.
local MCR_TAPPER = {
	columns = "CSRLDUT",
	-- Columns a log may add after those, in this order.
	optional = "X",
	-- The joystick's columns, which `joystick 4way` sends through MAME's
	-- 4-way filter.
	joystick = "RLDU",
	controls = {
		C = { ":ssio:IP0", 0x01 }, -- coin 1
		S = { ":ssio:IP0", 0x04 }, -- start 1
		R = { ":ssio:IP1", 0x01 }, -- joystick right
		L = { ":ssio:IP1", 0x02 }, -- joystick left
		D = { ":ssio:IP1", 0x04 }, -- joystick down
		U = { ":ssio:IP1", 0x08 }, -- joystick up
		T = { ":ssio:IP1", 0x10 }, -- tap (button 1)
		X = { ":ssio:IP0", 0x20 }, -- tilt
	},
	-- Every port the game reads through the SSIO board, recorded per frame.
	ports = { ":ssio:IP0", ":ssio:IP1", ":ssio:IP2", ":ssio:IP3", ":ssio:IP4" },
	-- The DIP switch port. Its fields must sit at mcr.cpp's defaults.
	dips = ":ssio:IP3",
	-- What goes in ram.bin for each frame, in this order. Main board RAM
	-- from mcr.cpp's cpu_91490_map, then the SSIO board's RAM from
	-- midway_sound.cpp's ssio_map. The palette RAM is write-only on the bus,
	-- so it's read from its memory share instead.
	ram = {
		{ "main e000-e7ff work RAM (nvram)", "space", ":maincpu", 0xe000, 0xe7ff },
		{ "main e800-e9ff sprite RAM", "space", ":maincpu", 0xe800, 0xe9ff },
		{ "main f000-f7ff video RAM", "space", ":maincpu", 0xf000, 0xf7ff },
		{ "main f800-f87f palette RAM", "share", ":paletteram", 0x00, 0x7f },
		{ "ssio 8000-83ff sound CPU RAM", "space", ":ssio:cpu", 0x8000, 0x83ff },
	},
}

return { tapper = MCR_TAPPER, tapperg = MCR_TAPPER, tappera = MCR_TAPPER,
	tapperb = MCR_TAPPER, sutapper = MCR_TAPPER, rbtapper = MCR_TAPPER }
