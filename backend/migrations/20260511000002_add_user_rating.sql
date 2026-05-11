ALTER TABLE users ADD COLUMN rating INT NOT NULL DEFAULT 1200;
CREATE INDEX idx_users_rating ON users(rating DESC);
