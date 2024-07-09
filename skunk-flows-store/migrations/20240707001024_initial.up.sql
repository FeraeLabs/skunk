CREATE TABLE metadata (
    key TEXT NOT NULL PRIMARY KEY,
    value JSONB NOT NULL
);

CREATE TABLE flow (
    flow_id UUID NOT NULL PRIMARY KEY,
    destination_address TEXT NOT NULL,
    destination_port INT NOT NULL,
    protocol UUID,
    timestamp DATETIME NOT NULL,
    metadata JSONB
);

CREATE INDEX index_flow_timestamp ON flow(timestamp);

CREATE TABLE message (
    message_id UUID NOT NULL PRIMARY KEY,
    flow_id UUID NOT NULL,
    kind TINYINT NOT NULL,
    timestamp DATETIME NOT NULL,
    data JSONB NOT NULL,
    metadata JSONB,

    FOREIGN KEY(flow_id) REFERENCES flow(flow_id)
);

CREATE INDEX index_message_timestamp ON message(timestamp);

CREATE TABLE artifact (
    artifact_id UUID NOT NULL PRIMARY KEY,
    message_id UUID,
    flow_id UUID,
    mime_type TEXT,
    file_name TEXT,
    timestamp DATETIME NOT NULL,
    hash BLOB NOT NULL,

    FOREIGN KEY(message_id) REFERENCES flow(message_id),
    FOREIGN KEY(flow_id) REFERENCES flow(flow_id),
    FOREIGN KEY(hash) REFERENCES artifact_blob(hash)
);

CREATE INDEX index_artifact_mime_type ON artifact(mime_type);
CREATE INDEX index_artifact_file_name ON artifact(file_name);

CREATE TABLE artifact_blob (
    hash BLOB NOT NULL PRIMARY KEY,
    size INT NOT NULL,
    data BLOB NOT NULL,
    metadata JSONB
);
