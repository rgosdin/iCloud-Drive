CREATE TABLE files (
	id INTEGER GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
	parent INTEGER NULL REFERENCES files(id) ON DELETE CASCADE,
	name TEXT NOT NULL,
	last_modified timestamp NULL,
	is_directory BOOLEAN NOT NULL,
	is_cover BOOLEAN NOT NULL,
	UNIQUE(parent, name),
	CHECK (NOT (is_directory AND is_cover))
);
COMMENT ON COLUMN files.parent IS
'Is NULL on top level.';
COMMENT ON COLUMN files.last_modified IS
'Is NULL on directories.';

CREATE MATERIALIZED VIEW file_paths AS
WITH RECURSIVE paths(id, path) AS (
		SELECT id, name
		FROM files
		WHERE parent IS NULL
	UNION ALL
		SELECT child.id, root.path || '/' || child.name
		FROM paths root
		INNER JOIN files child ON child.parent = root.id
)
SELECT id, path FROM paths;

CREATE TABLE artists (
	id INTEGER GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
	name TEXT UNIQUE NOT NULL
);

CREATE TABLE albums (
	id INTEGER GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
	artist INTEGER NULL REFERENCES artists(id) ON DELETE CASCADE,
	title TEXT NOT NULL,
	file INTEGER NULL REFERENCES files(id) ON DELETE CASCADE
);
CREATE UNIQUE INDEX
ON albums (artist, title, file)
WHERE file IS NOT NULL;
CREATE UNIQUE INDEX
ON albums (artist, title)
WHERE file IS NULL;
COMMENT ON COLUMN albums.file IS
'When set this Album is contained in a single file. See album_inline_tracks.';

CREATE TABLE album_tracks (
	id INTEGER GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
	file INTEGER UNIQUE NOT NULL REFERENCES files(id) ON DELETE CASCADE,
	album INTEGER NOT NULL REFERENCES albums(id) ON DELETE CASCADE,
	album_disc SMALLINT NOT NULL,
	title TEXT NOT NULL,
	position SMALLINT NOT NULL,
	duration INTEGER NOT NULL CHECK (duration > 0),
	UNIQUE (album, album_disc, position)
);
COMMENT ON COLUMN album_tracks.position IS
'Position of this track inside an album_disc.';

CREATE TABLE album_inline_tracks (
	album INTEGER NOT NULL REFERENCES albums(id) ON DELETE CASCADE,
	start INTEGER NOT NULL CHECK (start >= 0),
	"end" INTEGER NOT NULL CHECK ("end" > 0),
	title TEXT NOT NULL,
	UNIQUE (album, start)
);

-- TODO cleanup directories without albums
-- TODO global track id is not required. just needs to be uniqe per album. maybe reuse pos?
