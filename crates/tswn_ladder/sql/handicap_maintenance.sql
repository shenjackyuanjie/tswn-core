-- handicap 表空间回收与索引整理
-- =============================================================================
-- 背景（2026-10-23 线上实测）：
--   handicap 表 24 GB（heap 22 GB + 索引 2.5 GB），但 reltuples 只有约 570、
--   实际有效行约 1.9 万（活跃玩家的全部两两组合）；2.85M 个 8 KB 页面里
--   有 98% 是真空闲/死元组。成因是旧版天梯更新器每轮把整张表无条件
--   INSERT ... ON CONFLICT DO UPDATE 一遍（一分钟一次，累计 8000 万次更新），
--   而 match_count 上的索引让每次更新都无法走 HOT、必然新写索引项。
--
-- 什么时候跑：
--   * 新版 tswn_ladder 已经部署并至少跑过一个完整轮次（确认写入量降到每轮
--     约 1800 行，见日志里的 `handicap写入=`）；
--   * 选低峰时段。VACUUM FULL 会持有 ACCESS EXCLUSIVE 锁，期间天梯那一轮会被
--     阻塞、卡在 `[db]` 读取手势上；表很空，实际耗时主要是把 22 GB 页面读一遍
--     （线上实测第一次 17.7 s）。
--   * 确认磁盘有至少 1 GB 余量（表的有效数据只有几 MB，VACUUM FULL 会重写一份新的）。
--
-- 跑完如果发现它报告「found 0 removable, N nonremovable row versions」并且表还是
-- 几百 MB（2026-10-23 首次执行就遇到了），说明那一刻数据库里有一个很老的快照把
-- VACUUM 的可清理边界钉住了（`VACUUM (VERBOSE) 表名` 会打印 `removable cutoff`）。
-- 处理办法：先查是谁钉的，等那个事务结束（或者重启持有旧连接的进程）之后，
-- 再跑一次本脚本的第 4 步 —— 那时它会把剩下的死元组一次清干净并直接把表缩到目标大小。
-- 查法：
--   SELECT pid, usename, datname, state, now()-xact_start AS age, age(backend_xmin)
--   FROM pg_stat_activity WHERE backend_xmin IS NOT NULL ORDER BY xact_start;
--
-- 不建议：
--   * 直接 TRUNCATE —— 会丢掉全部对局克制数据，天梯要重新学；只有确认可以接受
--     「克制值全部归零」时才用，那种情况用它更快。
--   * 在旧版本还在跑的时候回收 —— 回收完旧的写放大立刻又会把表撑回去。
--
-- 执行：psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -f handicap_maintenance.sql
-- =============================================================================

\timing on

-- 1) 先看清楚当前状态（只读，很便宜）
SELECT relname,
       relpages,
       reltuples::bigint AS est_rows,
       pg_size_pretty(pg_total_relation_size('handicap')) AS total_size,
       pg_size_pretty(pg_relation_size('handicap')) AS heap_size,
       pg_size_pretty(pg_indexes_size('handicap')) AS index_size
FROM pg_class
WHERE relname = 'handicap';

-- 2) 去掉三个没用的索引
--    * idx_handicap_match_count：旧版每轮那条 `DELETE ... WHERE match_count = 0`
--      才会用到；新版改成按 (number, player1_id, player2_id) 精确删除后不再需要。
--      它同时是「每次更新都必须新写索引项」的元凶，去掉之后 value/match_count
--      的更新可以走 HOT，死元组在同一页内重用，表不会再膨胀。
--    * idx_handicap_player1 / idx_handicap_player2：没有任何查询按单列过滤；
--      表回收后只有几 MB，删会员时的外键检查直接顺序扫一遍即可。
DROP INDEX IF EXISTS idx_handicap_match_count;
DROP INDEX IF EXISTS idx_handicap_player1;
DROP INDEX IF EXISTS idx_handicap_player2;

-- 3) 留 30% 页内空闲给 HOT 更新用（不加这一条，页写满后又会回到「每次更新都新写页」）
ALTER TABLE handicap SET (fillfactor = 70, autovacuum_vacuum_scale_factor = 0.05);

-- 4) 回收空间：把 22 GB 的死元组/空闲页真正还给文件系统
VACUUM (FULL, ANALYZE) handicap;

-- 5) 复核
SELECT relname,
       relpages,
       reltuples::bigint AS est_rows,
       pg_size_pretty(pg_total_relation_size('handicap')) AS total_size,
       pg_size_pretty(pg_relation_size('handicap')) AS heap_size,
       pg_size_pretty(pg_indexes_size('handicap')) AS index_size
FROM pg_class
WHERE relname = 'handicap';

-- 期望：total_size 从 24 GB 掉到几 MB，relpages 从 2.85M 掉到几百。
-- 之后 gunicorn / 天梯都不需要重启；autovacuum 会按新的 fillfactor 维持这个规模。
