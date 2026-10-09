-- A match is "rated" when it moved Elo and counts toward profile W/L/T.
-- Matchmaking writes rated matches; POST /api/matches writes unrated ones.
-- No column default: every insert has to choose explicitly.
ALTER TABLE matches ADD COLUMN rated BOOLEAN;

-- Before this column, every UI-reachable match came from matchmaking, which
-- can't pair a user with themselves. Rows where one user is on both sides
-- could only have come from POST /api/matches, so they are unrated.
UPDATE matches SET rated = (red_user_id <> blue_user_id);

ALTER TABLE matches ALTER COLUMN rated SET NOT NULL;

-- Playing your own warriors never counts, whatever path inserted the row.
ALTER TABLE matches
    ADD CONSTRAINT matches_self_match_unrated
    CHECK (NOT rated OR red_user_id <> blue_user_id);
