CREATE TABLE matches (
    id              UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    red_warrior_id  UUID        NOT NULL REFERENCES warriors(id) ON DELETE CASCADE,
    blue_warrior_id UUID        NOT NULL REFERENCES warriors(id) ON DELETE CASCADE,
    red_user_id     UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    blue_user_id    UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    core_size       INT         NOT NULL DEFAULT 8000,
    max_steps       INT         NOT NULL DEFAULT 80000,
    result          VARCHAR(16) NOT NULL CHECK (result IN ('red_win', 'blue_win', 'tie', 'all_dead')),
    steps_taken     INT         NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_matches_red_user ON matches(red_user_id);
CREATE INDEX idx_matches_blue_user ON matches(blue_user_id);
