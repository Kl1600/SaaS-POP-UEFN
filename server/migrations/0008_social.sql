-- Vidéos de plusieurs plateformes (TikTok via Apify, YouTube Shorts via l'API YouTube).
ALTER TABLE tt_videos ADD COLUMN platform TEXT NOT NULL DEFAULT 'tiktok';
ALTER TABLE tt_videos ADD COLUMN thumb TEXT NOT NULL DEFAULT '';
CREATE TABLE social_status (
  studio_id INTEGER NOT NULL REFERENCES studios(id) ON DELETE CASCADE,
  platform TEXT NOT NULL,
  last_run TEXT,
  last_ok TEXT,
  last_error TEXT,
  last_count INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (studio_id, platform)
);
