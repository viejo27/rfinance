CREATE TABLE user_data (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role VARCHAR(50) NOT NULL DEFAULT 'user',
    name VARCHAR(255),
    last_name VARCHAR(255),
    nick_name VARCHAR(100),
    age INTEGER,
    gender VARCHAR(50),
    ip VARCHAR(45),
    user_agent TEXT,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP
);

CREATE INDEX idx_user_data_user_id ON user_data(user_id);
