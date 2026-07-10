-- 收敛旧版本可能产生的重复在途任务：优先保留已开始处理的任务，其次保留最早任务。
WITH ranked_in_flight_tasks AS (
    SELECT id,
           ROW_NUMBER() OVER (
               PARTITION BY scene_id
               ORDER BY CASE status WHEN 'processing' THEN 0 ELSE 1 END,
                        created_at ASC,
                        id ASC
           ) AS task_rank
    FROM asset_generation_tasks
    WHERE scene_id IS NOT NULL
      AND task_type = 'image_candidates'
      AND status IN ('pending', 'processing')
)
UPDATE asset_generation_tasks task
SET status = 'failed',
    error_message = '同一分镜存在重复在途任务，数据库迁移已停止该重复任务',
    result = jsonb_set(COALESCE(task.result, '{}'::jsonb), '{deduplicated}', 'true'::jsonb, true),
    updated_at = NOW()
FROM ranked_in_flight_tasks ranked
WHERE task.id = ranked.id
  AND ranked.task_rank > 1;

-- 单镜头图片重生属于可计费操作，同一分镜同时只允许一条在途任务。
CREATE UNIQUE INDEX asset_generation_tasks_one_in_flight_image_per_scene
    ON asset_generation_tasks(scene_id)
    WHERE scene_id IS NOT NULL
      AND task_type = 'image_candidates'
      AND status IN ('pending', 'processing');

COMMENT ON INDEX asset_generation_tasks_one_in_flight_image_per_scene IS
    '防止快速连点、网络重试或跨设备并发为同一分镜创建多条可计费图片任务。';

-- 每个到达服务端的幂等键都永久映射到实际返回的任务，覆盖并发请求复用同一在途任务后再迟到重试的场景。
CREATE TABLE asset_generation_task_requests (
    idempotency_key VARCHAR(200) PRIMARY KEY,
    task_id UUID NOT NULL REFERENCES asset_generation_tasks(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

COMMENT ON TABLE asset_generation_task_requests IS
    '素材生成请求幂等键与实际任务的永久映射，确保响应丢失后的迟到重试不会重复计费。';

INSERT INTO asset_generation_task_requests (idempotency_key, task_id)
SELECT idempotency_key, id
FROM asset_generation_tasks
WHERE idempotency_key <> ''
ON CONFLICT (idempotency_key) DO NOTHING;

CREATE INDEX idx_asset_generation_task_requests_task
    ON asset_generation_task_requests(task_id);
