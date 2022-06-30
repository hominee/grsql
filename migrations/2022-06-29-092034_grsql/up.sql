-- Your SQL goes here
CREATE TABLE IF NOT EXISTS grsql (
	id Long PRIMARY KEY AUTOINCREMENT NOT NULL ,
	name Text NOT NULL,
	mime Text NOT NULL,
	created INTEGER NOT NULL,
	updated INTEGER NOT NULL,
	content BLOB NOT NULL
);
