CREATE TYPE lock_type AS ENUM ('Executing', 'Failed', 'Finished', 'Initial', 'Retry');
CREATE TABLE IF NOT EXISTS saga_step (
    id uuid NOT NULL,
    step smallint NOT NULL,
    state text NOT NULL,
    dtc TIMESTAMP NOT NULL DEFAULT NOW ()
);
CREATE TABLE IF NOT EXISTS saga_lock (
    id uuid NOT NULL,
    executor_id uuid NOT NULL,
    name varchar NOT NULL,
    lock lock_type NOT NULL,
    dtc TIMESTAMP NOT NULL DEFAULT NOW ()
);
CREATE TABLE IF NOT EXISTS order_ticket (
    id uuid PRIMARY KEY,
    ticket_id uuid NULL,
    dtc TIMESTAMP NOT NULL DEFAULT NOW ()
);

CREATE UNIQUE INDEX saga_step_id_step_idx ON saga_step (id, step);
CREATE INDEX saga_lock_id_idx ON saga_lock (id);
CREATE INDEX saga_lock_lock_idx ON saga_lock (lock);
CREATE INDEX saga_lock_dtc_idx ON saga_lock (dtc);