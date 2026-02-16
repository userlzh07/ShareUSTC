#!/usr/bin/env python3
"""
ShareUSTC 数据库初始化脚本
跨平台: 支持 Windows, Linux, macOS
功能: 包含数据库创建和表创建两部分功能
"""

import subprocess
import sys
import os
import tempfile
import platform

# ============================================
# 配置变量
# ============================================
DB_NAME = "shareustc"
DB_USER = "shareustc_app"
DB_PASSWORD = "ShareUSTC_default_pwd"  # 生产环境请修改此密码
DB_HOST = "localhost"
DB_PORT = "5432"
POSTGRES_USER = "postgres"  # PostgreSQL 超级用户

# ============================================
# 数据库创建部分的 SQL
# ============================================
DB_CREATION_SQL = '''
-- 检查并创建用户
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = '{db_user}') THEN
        CREATE USER {db_user} WITH PASSWORD '{db_password}';
        RAISE NOTICE '用户 {db_user} 创建成功';
    ELSE
        RAISE NOTICE '用户 {db_user} 已存在，跳过创建';
    END IF;
END $$;

-- 检查并创建数据库
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_database WHERE datname = '{db_name}') THEN
        CREATE DATABASE {db_name} OWNER {db_user} ENCODING 'UTF8' LC_COLLATE '{locale}' LC_CTYPE '{locale}' TEMPLATE template0;
        RAISE NOTICE '数据库 {db_name} 创建成功';
    ELSE
        RAISE NOTICE '数据库 {db_name} 已存在，跳过创建';
    END IF;
END $$;
'''

# ============================================
# 权限授予部分的 SQL
# ============================================
DB_PERMISSION_SQL = '''
-- 授予数据库连接权限
GRANT CONNECT ON DATABASE {db_name} TO {db_user};

-- 在数据库内授予 schema 权限
GRANT USAGE ON SCHEMA public TO {db_user};
GRANT CREATE ON SCHEMA public TO {db_user};

-- 启用 pgcrypto 扩展
CREATE EXTENSION IF NOT EXISTS pgcrypto;

SELECT '权限授予完成' as status;
'''

# ============================================
# 表结构创建部分的 SQL
# ============================================
TABLE_CREATION_SQL = '''
-- ============================================
-- ShareUSTC 数据库增量更新脚本
-- 支持: 1) 首次创建表  2) 添加新列  3) 创建索引和触发器
-- 特点: 可重复执行，不会丢失已有数据
-- ============================================

-- 启用扩展
CREATE EXTENSION IF NOT EXISTS "pgcrypto";

-- ============================================
-- 创建 sn 序列（从1开始自增）
-- ============================================
CREATE SEQUENCE IF NOT EXISTS user_sn_seq START 1;

-- ============================================
-- 增强：确保序列起始值正确（考虑已有数据）
-- 版本迁移注意：如果数据库已有用户数据，此逻辑会自动调整序列
-- ============================================
DO $$
BEGIN
    PERFORM setval('user_sn_seq',
        (SELECT COALESCE(MAX(sn), 0) + 1 FROM users),
        false);
EXCEPTION
    WHEN undefined_table THEN NULL;
    WHEN undefined_column THEN NULL;
END $$;

-- ============================================
-- 1. 用户表
-- ============================================
CREATE TABLE IF NOT EXISTS users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'users' AND column_name = 'sn') THEN
        ALTER TABLE users ADD COLUMN sn BIGINT UNIQUE;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'users' AND column_name = 'username') THEN
        ALTER TABLE users ADD COLUMN username VARCHAR(50) UNIQUE NOT NULL DEFAULT 'temp_' || gen_random_uuid();
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'users' AND column_name = 'password_hash') THEN
        ALTER TABLE users ADD COLUMN password_hash VARCHAR(255) NOT NULL DEFAULT '';
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'users' AND column_name = 'email') THEN
        ALTER TABLE users ADD COLUMN email VARCHAR(255) UNIQUE;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'users' AND column_name = 'role') THEN
        ALTER TABLE users ADD COLUMN role VARCHAR(20) DEFAULT 'user';
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'users' AND column_name = 'bio') THEN
        ALTER TABLE users ADD COLUMN bio TEXT;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'users' AND column_name = 'social_links') THEN
        ALTER TABLE users ADD COLUMN social_links JSONB DEFAULT '{}';
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'users' AND column_name = 'real_info') THEN
        ALTER TABLE users ADD COLUMN real_info JSONB DEFAULT '{}';
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'users' AND column_name = 'is_verified') THEN
        ALTER TABLE users ADD COLUMN is_verified BOOLEAN DEFAULT FALSE;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'users' AND column_name = 'is_active') THEN
        ALTER TABLE users ADD COLUMN is_active BOOLEAN DEFAULT TRUE;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'users' AND column_name = 'avatar_url') THEN
        ALTER TABLE users ADD COLUMN avatar_url VARCHAR(500);
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'users' AND column_name = 'updated_at') THEN
        ALTER TABLE users ADD COLUMN updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP;
    END IF;
END $$;

-- ============================================
-- 2. 资源表
-- ============================================
CREATE TABLE IF NOT EXISTS resources (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'resources' AND column_name = 'title') THEN
        ALTER TABLE resources ADD COLUMN title VARCHAR(255) NOT NULL DEFAULT '';
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'resources' AND column_name = 'author_id') THEN
        ALTER TABLE resources ADD COLUMN author_id UUID REFERENCES users(id);
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'resources' AND column_name = 'uploader_id') THEN
        IF EXISTS (SELECT 1 FROM resources LIMIT 1) THEN
            ALTER TABLE resources ADD COLUMN uploader_id UUID REFERENCES users(id);
        ELSE
            ALTER TABLE resources ADD COLUMN uploader_id UUID NOT NULL REFERENCES users(id) DEFAULT '00000000-0000-0000-0000-000000000000';
        END IF;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'resources' AND column_name = 'course_name') THEN
        ALTER TABLE resources ADD COLUMN course_name VARCHAR(255);
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'resources' AND column_name = 'resource_type') THEN
        ALTER TABLE resources ADD COLUMN resource_type VARCHAR(50);
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'resources' AND column_name = 'category') THEN
        ALTER TABLE resources ADD COLUMN category VARCHAR(50);
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'resources' AND column_name = 'tags') THEN
        ALTER TABLE resources ADD COLUMN tags JSONB DEFAULT '[]';
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'resources' AND column_name = 'file_path') THEN
        ALTER TABLE resources ADD COLUMN file_path VARCHAR(500);
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'resources' AND column_name = 'source_file_path') THEN
        ALTER TABLE resources ADD COLUMN source_file_path VARCHAR(500);
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'resources' AND column_name = 'file_hash') THEN
        ALTER TABLE resources ADD COLUMN file_hash VARCHAR(64);
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'resources' AND column_name = 'file_size') THEN
        ALTER TABLE resources ADD COLUMN file_size BIGINT;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'resources' AND column_name = 'content_accuracy') THEN
        ALTER TABLE resources ADD COLUMN content_accuracy FLOAT8;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'resources' AND column_name = 'audit_status') THEN
        ALTER TABLE resources ADD COLUMN audit_status VARCHAR(20) DEFAULT 'pending';
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'resources' AND column_name = 'ai_reject_reason') THEN
        ALTER TABLE resources ADD COLUMN ai_reject_reason TEXT;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'resources' AND column_name = 'updated_at') THEN
        ALTER TABLE resources ADD COLUMN updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP;
    END IF;
END $$;

-- ============================================
-- 3. 资源统计表
-- ============================================
CREATE TABLE IF NOT EXISTS resource_stats (
    resource_id UUID PRIMARY KEY REFERENCES resources(id) ON DELETE CASCADE
);

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'resource_stats' AND column_name = 'views') THEN
        ALTER TABLE resource_stats ADD COLUMN views INTEGER DEFAULT 0;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'resource_stats' AND column_name = 'downloads') THEN
        ALTER TABLE resource_stats ADD COLUMN downloads INTEGER DEFAULT 0;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'resource_stats' AND column_name = 'likes') THEN
        ALTER TABLE resource_stats ADD COLUMN likes INTEGER DEFAULT 0;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'resource_stats' AND column_name = 'rating_count') THEN
        ALTER TABLE resource_stats ADD COLUMN rating_count INTEGER DEFAULT 0;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'resource_stats' AND column_name = 'difficulty_total') THEN
        ALTER TABLE resource_stats ADD COLUMN difficulty_total INTEGER DEFAULT 0;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'resource_stats' AND column_name = 'difficulty_count') THEN
        ALTER TABLE resource_stats ADD COLUMN difficulty_count INTEGER DEFAULT 0;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'resource_stats' AND column_name = 'overall_quality_total') THEN
        ALTER TABLE resource_stats ADD COLUMN overall_quality_total INTEGER DEFAULT 0;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'resource_stats' AND column_name = 'overall_quality_count') THEN
        ALTER TABLE resource_stats ADD COLUMN overall_quality_count INTEGER DEFAULT 0;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'resource_stats' AND column_name = 'answer_quality_total') THEN
        ALTER TABLE resource_stats ADD COLUMN answer_quality_total INTEGER DEFAULT 0;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'resource_stats' AND column_name = 'answer_quality_count') THEN
        ALTER TABLE resource_stats ADD COLUMN answer_quality_count INTEGER DEFAULT 0;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'resource_stats' AND column_name = 'format_quality_total') THEN
        ALTER TABLE resource_stats ADD COLUMN format_quality_total INTEGER DEFAULT 0;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'resource_stats' AND column_name = 'format_quality_count') THEN
        ALTER TABLE resource_stats ADD COLUMN format_quality_count INTEGER DEFAULT 0;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'resource_stats' AND column_name = 'detail_level_total') THEN
        ALTER TABLE resource_stats ADD COLUMN detail_level_total INTEGER DEFAULT 0;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'resource_stats' AND column_name = 'detail_level_count') THEN
        ALTER TABLE resource_stats ADD COLUMN detail_level_count INTEGER DEFAULT 0;
    END IF;
END $$;

-- ============================================
-- 4. 评分表
-- ============================================
CREATE TABLE IF NOT EXISTS ratings (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'ratings' AND column_name = 'resource_id') THEN
        IF EXISTS (SELECT 1 FROM ratings LIMIT 1) THEN
            ALTER TABLE ratings ADD COLUMN resource_id UUID REFERENCES resources(id) ON DELETE CASCADE;
        ELSE
            ALTER TABLE ratings ADD COLUMN resource_id UUID NOT NULL REFERENCES resources(id) ON DELETE CASCADE DEFAULT '00000000-0000-0000-0000-000000000000';
        END IF;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'ratings' AND column_name = 'user_id') THEN
        IF EXISTS (SELECT 1 FROM ratings LIMIT 1) THEN
            ALTER TABLE ratings ADD COLUMN user_id UUID REFERENCES users(id) ON DELETE CASCADE;
        ELSE
            ALTER TABLE ratings ADD COLUMN user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE DEFAULT '00000000-0000-0000-0000-000000000000';
        END IF;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'ratings' AND column_name = 'difficulty') THEN
        ALTER TABLE ratings ADD COLUMN difficulty INTEGER CHECK (difficulty BETWEEN 1 AND 10);
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'ratings' AND column_name = 'overall_quality') THEN
        ALTER TABLE ratings ADD COLUMN overall_quality INTEGER CHECK (overall_quality BETWEEN 1 AND 10);
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'ratings' AND column_name = 'answer_quality') THEN
        ALTER TABLE ratings ADD COLUMN answer_quality INTEGER CHECK (answer_quality BETWEEN 1 AND 10);
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'ratings' AND column_name = 'format_quality') THEN
        ALTER TABLE ratings ADD COLUMN format_quality INTEGER CHECK (format_quality BETWEEN 1 AND 10);
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'ratings' AND column_name = 'detail_level') THEN
        ALTER TABLE ratings ADD COLUMN detail_level INTEGER CHECK (detail_level BETWEEN 1 AND 10);
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'ratings' AND column_name = 'updated_at') THEN
        ALTER TABLE ratings ADD COLUMN updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP;
    END IF;
END $$;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conname = 'ratings_resource_id_user_id_key' AND conrelid = 'ratings'::regclass
    ) THEN
        ALTER TABLE ratings ADD CONSTRAINT ratings_resource_id_user_id_key UNIQUE (resource_id, user_id);
    END IF;
EXCEPTION
    WHEN unique_violation THEN
        RAISE NOTICE '无法添加唯一约束：存在重复数据 (resource_id, user_id)';
END $$;

-- ============================================
-- 5. 点赞表
-- ============================================
CREATE TABLE IF NOT EXISTS likes (
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'likes' AND column_name = 'resource_id') THEN
        ALTER TABLE likes ADD COLUMN resource_id UUID REFERENCES resources(id) ON DELETE CASCADE;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'likes' AND column_name = 'user_id') THEN
        ALTER TABLE likes ADD COLUMN user_id UUID REFERENCES users(id) ON DELETE CASCADE;
    END IF;
END $$;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conname = 'likes_pkey' AND conrelid = 'likes'::regclass
    ) THEN
        ALTER TABLE likes ADD PRIMARY KEY (resource_id, user_id);
    END IF;
EXCEPTION
    WHEN unique_violation THEN
        RAISE NOTICE '无法添加主键约束：存在重复数据';
END $$;

-- ============================================
-- 6. 评论表
-- ============================================
CREATE TABLE IF NOT EXISTS comments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'comments' AND column_name = 'resource_id') THEN
        IF EXISTS (SELECT 1 FROM comments LIMIT 1) THEN
            ALTER TABLE comments ADD COLUMN resource_id UUID REFERENCES resources(id) ON DELETE CASCADE;
        ELSE
            ALTER TABLE comments ADD COLUMN resource_id UUID NOT NULL REFERENCES resources(id) ON DELETE CASCADE DEFAULT '00000000-0000-0000-0000-000000000000';
        END IF;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'comments' AND column_name = 'user_id') THEN
        IF EXISTS (SELECT 1 FROM comments LIMIT 1) THEN
            ALTER TABLE comments ADD COLUMN user_id UUID REFERENCES users(id) ON DELETE CASCADE;
        ELSE
            ALTER TABLE comments ADD COLUMN user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE DEFAULT '00000000-0000-0000-0000-000000000000';
        END IF;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'comments' AND column_name = 'content') THEN
        ALTER TABLE comments ADD COLUMN content TEXT NOT NULL DEFAULT '';
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'comments' AND column_name = 'audit_status') THEN
        ALTER TABLE comments ADD COLUMN audit_status VARCHAR(20) DEFAULT 'approved';
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'comments' AND column_name = 'updated_at') THEN
        ALTER TABLE comments ADD COLUMN updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP;
    END IF;
END $$;

-- ============================================
-- 7. 收藏夹表
-- ============================================
CREATE TABLE IF NOT EXISTS favorites (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'favorites' AND column_name = 'user_id') THEN
        IF EXISTS (SELECT 1 FROM favorites LIMIT 1) THEN
            ALTER TABLE favorites ADD COLUMN user_id UUID REFERENCES users(id) ON DELETE CASCADE;
        ELSE
            ALTER TABLE favorites ADD COLUMN user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE DEFAULT '00000000-0000-0000-0000-000000000000';
        END IF;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'favorites' AND column_name = 'name') THEN
        ALTER TABLE favorites ADD COLUMN name VARCHAR(255) NOT NULL DEFAULT '未命名收藏夹';
    END IF;
END $$;

-- ============================================
-- 8. 收藏夹资源关联表
-- ============================================
CREATE TABLE IF NOT EXISTS favorite_resources (
    added_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'favorite_resources' AND column_name = 'favorite_id') THEN
        ALTER TABLE favorite_resources ADD COLUMN favorite_id UUID REFERENCES favorites(id) ON DELETE CASCADE;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'favorite_resources' AND column_name = 'resource_id') THEN
        ALTER TABLE favorite_resources ADD COLUMN resource_id UUID REFERENCES resources(id) ON DELETE CASCADE;
    END IF;
END $$;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conname = 'favorite_resources_pkey' AND conrelid = 'favorite_resources'::regclass
    ) THEN
        ALTER TABLE favorite_resources ADD PRIMARY KEY (favorite_id, resource_id);
    END IF;
EXCEPTION
    WHEN unique_violation THEN
        RAISE NOTICE '无法添加主键约束：存在重复数据';
END $$;

-- ============================================
-- 9. 申领表
-- ============================================
CREATE TABLE IF NOT EXISTS claims (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'claims' AND column_name = 'resource_id') THEN
        IF EXISTS (SELECT 1 FROM claims LIMIT 1) THEN
            ALTER TABLE claims ADD COLUMN resource_id UUID REFERENCES resources(id) ON DELETE CASCADE;
        ELSE
            ALTER TABLE claims ADD COLUMN resource_id UUID NOT NULL REFERENCES resources(id) ON DELETE CASCADE DEFAULT '00000000-0000-0000-0000-000000000000';
        END IF;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'claims' AND column_name = 'applicant_id') THEN
        IF EXISTS (SELECT 1 FROM claims LIMIT 1) THEN
            ALTER TABLE claims ADD COLUMN applicant_id UUID REFERENCES users(id) ON DELETE CASCADE;
        ELSE
            ALTER TABLE claims ADD COLUMN applicant_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE DEFAULT '00000000-0000-0000-0000-000000000000';
        END IF;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'claims' AND column_name = 'claim_type') THEN
        ALTER TABLE claims ADD COLUMN claim_type VARCHAR(20);
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'claims' AND column_name = 'reason') THEN
        ALTER TABLE claims ADD COLUMN reason TEXT NOT NULL DEFAULT '';
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'claims' AND column_name = 'proof_files') THEN
        ALTER TABLE claims ADD COLUMN proof_files JSONB DEFAULT '[]';
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'claims' AND column_name = 'status') THEN
        ALTER TABLE claims ADD COLUMN status VARCHAR(20) DEFAULT 'pending';
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'claims' AND column_name = 'reviewer_id') THEN
        ALTER TABLE claims ADD COLUMN reviewer_id UUID REFERENCES users(id);
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'claims' AND column_name = 'reviewed_at') THEN
        ALTER TABLE claims ADD COLUMN reviewed_at TIMESTAMP;
    END IF;
END $$;

-- ============================================
-- 10. 通知表
-- ============================================
CREATE TABLE IF NOT EXISTS notifications (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'notifications' AND column_name = 'recipient_id') THEN
        ALTER TABLE notifications ADD COLUMN recipient_id UUID REFERENCES users(id) ON DELETE CASCADE;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'notifications' AND column_name = 'title') THEN
        ALTER TABLE notifications ADD COLUMN title VARCHAR(255) NOT NULL DEFAULT '';
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'notifications' AND column_name = 'content') THEN
        ALTER TABLE notifications ADD COLUMN content TEXT NOT NULL DEFAULT '';
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'notifications' AND column_name = 'notification_type') THEN
        ALTER TABLE notifications ADD COLUMN notification_type VARCHAR(50);
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'notifications' AND column_name = 'priority') THEN
        ALTER TABLE notifications ADD COLUMN priority VARCHAR(20) DEFAULT 'normal';
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'notifications' AND column_name = 'is_read') THEN
        ALTER TABLE notifications ADD COLUMN is_read BOOLEAN DEFAULT FALSE;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'notifications' AND column_name = 'link_url') THEN
        ALTER TABLE notifications ADD COLUMN link_url VARCHAR(500);
    END IF;
END $$;

-- ============================================
-- 10b. 通知已读记录表
-- ============================================
CREATE TABLE IF NOT EXISTS notification_reads (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    read_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'notification_reads' AND column_name = 'notification_id') THEN
        IF EXISTS (SELECT 1 FROM notification_reads LIMIT 1) THEN
            ALTER TABLE notification_reads ADD COLUMN notification_id UUID REFERENCES notifications(id) ON DELETE CASCADE;
        ELSE
            ALTER TABLE notification_reads ADD COLUMN notification_id UUID NOT NULL REFERENCES notifications(id) ON DELETE CASCADE DEFAULT '00000000-0000-0000-0000-000000000000';
        END IF;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'notification_reads' AND column_name = 'user_id') THEN
        IF EXISTS (SELECT 1 FROM notification_reads LIMIT 1) THEN
            ALTER TABLE notification_reads ADD COLUMN user_id UUID REFERENCES users(id) ON DELETE CASCADE;
        ELSE
            ALTER TABLE notification_reads ADD COLUMN user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE DEFAULT '00000000-0000-0000-0000-000000000000';
        END IF;
    END IF;
END $$;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conname = 'notification_reads_notification_id_user_id_key' AND conrelid = 'notification_reads'::regclass
    ) THEN
        ALTER TABLE notification_reads ADD CONSTRAINT notification_reads_notification_id_user_id_key UNIQUE (notification_id, user_id);
    END IF;
EXCEPTION
    WHEN unique_violation THEN
        RAISE NOTICE '无法添加唯一约束：存在重复数据';
END $$;

-- ============================================
-- 11. 审计日志表
-- ============================================
CREATE TABLE IF NOT EXISTS audit_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'audit_logs' AND column_name = 'user_id') THEN
        ALTER TABLE audit_logs ADD COLUMN user_id UUID REFERENCES users(id);
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'audit_logs' AND column_name = 'action') THEN
        ALTER TABLE audit_logs ADD COLUMN action VARCHAR(100) NOT NULL DEFAULT '';
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'audit_logs' AND column_name = 'target_type') THEN
        ALTER TABLE audit_logs ADD COLUMN target_type VARCHAR(50);
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'audit_logs' AND column_name = 'target_id') THEN
        ALTER TABLE audit_logs ADD COLUMN target_id UUID;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'audit_logs' AND column_name = 'details') THEN
        ALTER TABLE audit_logs ADD COLUMN details JSONB DEFAULT '{}';
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'audit_logs' AND column_name = 'ip_address') THEN
        ALTER TABLE audit_logs ADD COLUMN ip_address INET;
    END IF;
END $$;

-- ============================================
-- 12. 下载记录表
-- ============================================
CREATE TABLE IF NOT EXISTS download_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    downloaded_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'download_logs' AND column_name = 'resource_id') THEN
        IF EXISTS (SELECT 1 FROM download_logs LIMIT 1) THEN
            ALTER TABLE download_logs ADD COLUMN resource_id UUID REFERENCES resources(id) ON DELETE CASCADE;
        ELSE
            ALTER TABLE download_logs ADD COLUMN resource_id UUID NOT NULL REFERENCES resources(id) ON DELETE CASCADE DEFAULT '00000000-0000-0000-0000-000000000000';
        END IF;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'download_logs' AND column_name = 'user_id') THEN
        ALTER TABLE download_logs ADD COLUMN user_id UUID REFERENCES users(id) ON DELETE SET NULL;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'download_logs' AND column_name = 'ip_address') THEN
        ALTER TABLE download_logs ADD COLUMN ip_address INET NOT NULL DEFAULT '0.0.0.0';
    END IF;
END $$;

-- ============================================
-- 13. 图片表
-- ============================================
CREATE TABLE IF NOT EXISTS images (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'images' AND column_name = 'uploader_id') THEN
        IF EXISTS (SELECT 1 FROM images LIMIT 1) THEN
            ALTER TABLE images ADD COLUMN uploader_id UUID REFERENCES users(id);
        ELSE
            ALTER TABLE images ADD COLUMN uploader_id UUID NOT NULL REFERENCES users(id) DEFAULT '00000000-0000-0000-0000-000000000000';
        END IF;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'images' AND column_name = 'file_path') THEN
        ALTER TABLE images ADD COLUMN file_path VARCHAR(500) NOT NULL DEFAULT '';
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'images' AND column_name = 'original_name') THEN
        ALTER TABLE images ADD COLUMN original_name VARCHAR(255);
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'images' AND column_name = 'file_size') THEN
        ALTER TABLE images ADD COLUMN file_size INTEGER;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'images' AND column_name = 'mime_type') THEN
        ALTER TABLE images ADD COLUMN mime_type VARCHAR(50);
    END IF;
END $$;

-- ============================================
-- 为现有用户分配 sn（增量更新支持）
-- ============================================
DO $$
DECLARE
    user_record RECORD;
    current_sn BIGINT := 1;
BEGIN
    FOR user_record IN
        SELECT id FROM users WHERE sn IS NULL ORDER BY created_at ASC
    LOOP
        UPDATE users SET sn = current_sn WHERE id = user_record.id;
        current_sn := current_sn + 1;
    END LOOP;
    IF current_sn > 1 THEN
        PERFORM setval('user_sn_seq', current_sn - 1, true);
    END IF;
END $$;

-- ============================================
-- 创建索引
-- ============================================
CREATE INDEX IF NOT EXISTS idx_users_role ON users(role);
CREATE INDEX IF NOT EXISTS idx_users_is_verified ON users(is_verified);
CREATE INDEX IF NOT EXISTS idx_users_sn ON users(sn);
CREATE INDEX IF NOT EXISTS idx_resources_uploader ON resources(uploader_id);
CREATE INDEX IF NOT EXISTS idx_resources_author ON resources(author_id);
CREATE INDEX IF NOT EXISTS idx_resources_course ON resources(course_name);
CREATE INDEX IF NOT EXISTS idx_resources_type ON resources(resource_type);
CREATE INDEX IF NOT EXISTS idx_resources_category ON resources(category);
CREATE INDEX IF NOT EXISTS idx_resources_audit_status ON resources(audit_status);
CREATE INDEX IF NOT EXISTS idx_resources_tags ON resources USING GIN(tags);
CREATE INDEX IF NOT EXISTS idx_resources_created_at ON resources(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_ratings_resource ON ratings(resource_id);
CREATE INDEX IF NOT EXISTS idx_ratings_user ON ratings(user_id);
CREATE INDEX IF NOT EXISTS idx_likes_user ON likes(user_id);
CREATE INDEX IF NOT EXISTS idx_comments_resource ON comments(resource_id);
CREATE INDEX IF NOT EXISTS idx_comments_user ON comments(user_id);
CREATE INDEX IF NOT EXISTS idx_comments_created_at ON comments(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_favorites_user ON favorites(user_id);
CREATE INDEX IF NOT EXISTS idx_fav_res_resource ON favorite_resources(resource_id);
CREATE INDEX IF NOT EXISTS idx_claims_resource ON claims(resource_id);
CREATE INDEX IF NOT EXISTS idx_claims_applicant ON claims(applicant_id);
CREATE INDEX IF NOT EXISTS idx_claims_status ON claims(status);
CREATE INDEX IF NOT EXISTS idx_notifications_recipient ON notifications(recipient_id);
CREATE INDEX IF NOT EXISTS idx_notifications_priority ON notifications(priority);
CREATE INDEX IF NOT EXISTS idx_notifications_is_read ON notifications(is_read);
CREATE INDEX IF NOT EXISTS idx_notifications_created_at ON notifications(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_notification_reads_notification ON notification_reads(notification_id);
CREATE INDEX IF NOT EXISTS idx_notification_reads_user ON notification_reads(user_id);
CREATE INDEX IF NOT EXISTS idx_notification_reads_unique ON notification_reads(notification_id, user_id);
CREATE INDEX IF NOT EXISTS idx_audit_logs_user ON audit_logs(user_id);
CREATE INDEX IF NOT EXISTS idx_audit_logs_action ON audit_logs(action);
CREATE INDEX IF NOT EXISTS idx_audit_logs_created_at ON audit_logs(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_download_logs_resource ON download_logs(resource_id);
CREATE INDEX IF NOT EXISTS idx_download_logs_user ON download_logs(user_id);
CREATE INDEX IF NOT EXISTS idx_download_logs_time ON download_logs(downloaded_at DESC);
CREATE INDEX IF NOT EXISTS idx_images_uploader ON images(uploader_id);

-- ============================================
-- 创建触发器
-- ============================================
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = CURRENT_TIMESTAMP;
    RETURN NEW;
END;
$$
language 'plpgsql';

DROP TRIGGER IF EXISTS update_users_updated_at ON users;
CREATE TRIGGER update_users_updated_at
    BEFORE UPDATE ON users
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

DROP TRIGGER IF EXISTS update_resources_updated_at ON resources;
CREATE TRIGGER update_resources_updated_at
    BEFORE UPDATE ON resources
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

DROP TRIGGER IF EXISTS update_ratings_updated_at ON ratings;
CREATE TRIGGER update_ratings_updated_at
    BEFORE UPDATE ON ratings
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

DROP TRIGGER IF EXISTS update_comments_updated_at ON comments;
CREATE TRIGGER update_comments_updated_at
    BEFORE UPDATE ON comments
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- ============================================
-- 验证
-- ============================================
SELECT 'users' as table_name, COUNT(*) as column_count FROM information_schema.columns WHERE table_name = 'users'
UNION ALL
SELECT 'resources', COUNT(*) FROM information_schema.columns WHERE table_name = 'resources'
UNION ALL
SELECT 'resource_stats', COUNT(*) FROM information_schema.columns WHERE table_name = 'resource_stats'
UNION ALL
SELECT 'ratings', COUNT(*) FROM information_schema.columns WHERE table_name = 'ratings'
UNION ALL
SELECT 'likes', COUNT(*) FROM information_schema.columns WHERE table_name = 'likes'
UNION ALL
SELECT 'comments', COUNT(*) FROM information_schema.columns WHERE table_name = 'comments'
UNION ALL
SELECT 'favorites', COUNT(*) FROM information_schema.columns WHERE table_name = 'favorites'
UNION ALL
SELECT 'favorite_resources', COUNT(*) FROM information_schema.columns WHERE table_name = 'favorite_resources'
UNION ALL
SELECT 'claims', COUNT(*) FROM information_schema.columns WHERE table_name = 'claims'
UNION ALL
SELECT 'notifications', COUNT(*) FROM information_schema.columns WHERE table_name = 'notifications'
UNION ALL
SELECT 'notification_reads', COUNT(*) FROM information_schema.columns WHERE table_name = 'notification_reads'
UNION ALL
SELECT 'audit_logs', COUNT(*) FROM information_schema.columns WHERE table_name = 'audit_logs'
UNION ALL
SELECT 'download_logs', COUNT(*) FROM information_schema.columns WHERE table_name = 'download_logs'
UNION ALL
SELECT 'images', COUNT(*) FROM information_schema.columns WHERE table_name = 'images';
'''


def find_psql():
    """查找 psql 可执行文件"""
    system = platform.system()

    # 首先检查 PATH
    try:
        if system == "Windows":
            result = subprocess.run(['where', 'psql'], capture_output=True, text=True)
        else:
            result = subprocess.run(['which', 'psql'], capture_output=True, text=True)
        if result.returncode == 0:
            return 'psql'
    except:
        pass

    # 检查常见 PostgreSQL 安装路径
    common_paths = []
    if system == "Windows":
        common_paths = [
            r"C:\Program Files\PostgreSQL",
            r"C:\Program Files (x86)\PostgreSQL"
        ]
    else:
        common_paths = [
            "/usr/bin/psql",
            "/usr/local/bin/psql",
            "/opt/homebrew/bin/psql",
            "/Applications/Postgres.app/Contents/Versions/latest/bin/psql"
        ]
        for path in common_paths:
            if os.path.exists(path):
                return path

    if system == "Windows":
        for base_path in common_paths:
            if os.path.exists(base_path):
                for version in os.listdir(base_path):
                    psql_path = os.path.join(base_path, version, 'bin', 'psql.exe')
                    if os.path.exists(psql_path):
                        return psql_path

    return None


def execute_sql_with_superuser(psql, sql, password, database="postgres"):
    """使用超级用户执行 SQL"""
    env = os.environ.copy()
    env['PGPASSWORD'] = password

    with tempfile.NamedTemporaryFile(mode='w', suffix='.sql', delete=False, encoding='utf-8') as f:
        f.write(sql)
        temp_file = f.name

    try:
        result = subprocess.run(
            [psql, '-h', DB_HOST, '-p', DB_PORT, '-U', POSTGRES_USER, '-d', database, '-f', temp_file, '-q'],
            capture_output=True,
            text=True,
            env=env
        )
        return result
    finally:
        os.unlink(temp_file)


def execute_sql_with_app_user(psql, database, sql):
    """使用应用用户执行 SQL"""
    env = os.environ.copy()
    env['PGPASSWORD'] = DB_PASSWORD

    with tempfile.NamedTemporaryFile(mode='w', suffix='.sql', delete=False, encoding='utf-8') as f:
        f.write(sql)
        temp_file = f.name

    try:
        result = subprocess.run(
            [psql, '-h', DB_HOST, '-p', DB_PORT, '-U', DB_USER, '-d', database, '-f', temp_file, '-q'],
            capture_output=True,
            text=True,
            env=env
        )
        return result
    finally:
        os.unlink(temp_file)


def create_database(psql, postgres_password):
    """步骤1: 创建数据库和用户"""
    print("=== 步骤 1/3: 创建数据库和用户 ===")
    print()

    # 检测操作系统以设置正确的 locale
    system = platform.system()
    locale = "C" if system == "Windows" else "C.UTF-8"

    sql = DB_CREATION_SQL.format(
        db_name=DB_NAME,
        db_user=DB_USER,
        db_password=DB_PASSWORD,
        locale=locale
    )

    result = execute_sql_with_superuser(psql, sql, postgres_password)

    if result.returncode == 0:
        print(f"  数据库 '{DB_NAME}' 和用户 '{DB_USER}' 检查/创建完成")
        return True
    else:
        print(f"  错误: 创建数据库失败")
        print(f"  {result.stderr}")
        return False


def grant_permissions(psql, postgres_password):
    """步骤2: 授予权限"""
    print("=== 步骤 2/3: 授予权限 ===")
    print()

    sql = DB_PERMISSION_SQL.format(
        db_name=DB_NAME,
        db_user=DB_USER
    )

    result = execute_sql_with_superuser(psql, sql, postgres_password, database=DB_NAME)

    if result.returncode == 0:
        print(f"  权限授予完成")
        return True
    else:
        print(f"  错误: 授予权限失败")
        print(f"  {result.stderr}")
        return False


def create_tables(psql):
    """步骤3: 创建表结构"""
    print("=== 步骤 3/3: 创建表结构 ===")
    print()

    print("  开始执行增量更新...")
    result = execute_sql_with_app_user(psql, DB_NAME, TABLE_CREATION_SQL)

    if result.returncode == 0:
        print(result.stdout)
        return True
    else:
        print(f"  错误: SQL 执行失败")
        print(f"  {result.stderr}")
        return False


def test_connection(psql, user, password, database):
    """测试数据库连接"""
    env = os.environ.copy()
    env['PGPASSWORD'] = password

    try:
        result = subprocess.run(
            [psql, '-h', DB_HOST, '-p', DB_PORT, '-U', user, '-d', database, '-c', 'SELECT 1;', '-q'],
            capture_output=True,
            text=True,
            env=env
        )
        return result.returncode == 0 and '1' in result.stdout
    except:
        return False


def main():
    print("=" * 50)
    print("ShareUSTC 数据库初始化脚本")
    print("=" * 50)
    print()
    print("功能: 创建数据库、用户和表结构")
    print("注意: 需要 PostgreSQL 超级用户权限")
    print()

    # 查找 psql
    psql = find_psql()
    if not psql:
        print("错误: 未找到 psql 命令。请安装 PostgreSQL 并确保它在 PATH 中。")
        sys.exit(1)
    print(f"使用 psql: {psql}")
    print()

    # 请求 postgres 用户密码
    system = platform.system()
    if system == "Windows":
        import getpass as gp
        postgres_password = gp.getpass(f"请输入 PostgreSQL '{POSTGRES_USER}' 用户的密码 (默认通常为 'postgres' 或空): ")
    else:
        import getpass as gp
        postgres_password = gp.getpass(f"请输入 PostgreSQL '{POSTGRES_USER}' 用户的密码 (默认通常为 'postgres' 或空): ")
    print()

    # 测试 postgres 连接
    print("测试 postgres 用户连接...")
    if not test_connection(psql, POSTGRES_USER, postgres_password, "postgres"):
        print("错误: 无法连接到 PostgreSQL。请检查密码和服务状态。")
        sys.exit(1)
    print("  连接成功")
    print()

    # 步骤1: 创建数据库和用户
    if not create_database(psql, postgres_password):
        sys.exit(1)
    print()

    # 步骤2: 授予权限
    if not grant_permissions(psql, postgres_password):
        sys.exit(1)
    print()

    # 步骤3: 创建表结构
    if not create_tables(psql):
        sys.exit(1)
    print()

    print("=" * 50)
    print("数据库初始化完成！")
    print("=" * 50)
    print()
    print("数据库信息:")
    print(f"  数据库名: {DB_NAME}")
    print(f"  用户名:   {DB_USER}")
    print(f"  密码:     {DB_PASSWORD}")
    print()
    print("已创建/更新的表:")
    print("  - users (用户表)")
    print("  - resources (资源表)")
    print("  - resource_stats (资源统计表)")
    print("  - ratings (评分表)")
    print("  - likes (点赞表)")
    print("  - comments (评论表)")
    print("  - favorites (收藏夹表)")
    print("  - favorite_resources (收藏夹资源关联表)")
    print("  - claims (申领表)")
    print("  - notifications (通知表)")
    print("  - notification_reads (通知已读记录表)")
    print("  - audit_logs (审计日志表)")
    print("  - download_logs (下载记录表)")
    print("  - images (图片表)")
    print()
    print("索引: 30+")
    print("触发器: 4 (自动更新 updated_at)")
    print()
    print("说明: 此脚本支持增量更新，可重复执行。")
    print()


if __name__ == '__main__':
    main()
