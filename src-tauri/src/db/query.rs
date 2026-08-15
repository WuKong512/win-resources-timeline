use crate::models::{
    AppIdentity, AppResourceHistoryPoint, AppResourceSample, AppUsageSummary, ForegroundInterval,
    ResourceApp, SystemSample, TodayOverview,
};
use rusqlite::{params, Connection};
use std::collections::BTreeSet;

fn valid_range(start_ms: i64, end_ms: i64) -> rusqlite::Result<()> {
    if end_ms <= start_ms {
        return Err(rusqlite::Error::InvalidParameterName(
            "end_ms must be greater than start_ms".into(),
        ));
    }
    Ok(())
}

pub fn list_apps(conn: &Connection) -> rusqlite::Result<Vec<AppIdentity>> {
    let mut stmt = conn.prepare(
        "SELECT e.id, a.process_name, CASE WHEN e.normalized_path LIKE 'path:%' THEN substr(e.normalized_path, 6) END, a.display_name, a.publisher, a.is_hidden, a.first_seen_at_ms, a.last_seen_at_ms FROM app_executable e JOIN app a ON a.id = e.app_id ORDER BY a.display_name COLLATE NOCASE",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(AppIdentity {
            id: r.get(0)?,
            process_name: r.get(1)?,
            exe_path: r.get(2)?,
            display_name: r.get(3)?,
            publisher: r.get(4)?,
            is_hidden: r.get::<_, i64>(5)? != 0,
            first_seen_at_ms: r.get(6)?,
            last_seen_at_ms: r.get(7)?,
        })
    })?;
    rows.collect()
}

pub fn foreground_intervals(
    conn: &Connection,
    start_ms: i64,
    end_ms: i64,
    include_hidden: bool,
    include_idle: bool,
) -> rusqlite::Result<Vec<ForegroundInterval>> {
    valid_range(start_ms, end_ms)?;
    let mut stmt = conn.prepare(
        r#"SELECT fi.id, fi.app_executable_id, a.process_name, a.display_name,
                  MAX(fi.start_time_ms, ?1), MIN(COALESCE(fi.end_time_ms, fi.last_seen_time_ms), ?2),
                  fi.activity_state, a.is_hidden
           FROM foreground_interval fi
           JOIN app_executable e ON e.id = fi.app_executable_id
           JOIN app a ON a.id = e.app_id
           WHERE fi.start_time_ms < ?2 AND COALESCE(fi.end_time_ms, fi.last_seen_time_ms) > ?1
             AND (?3 = 1 OR a.is_hidden = 0) AND (?4 = 1 OR fi.activity_state = 'active')
           ORDER BY fi.start_time_ms"#,
    )?;
    let rows = stmt.query_map(
        params![start_ms, end_ms, include_hidden as i64, include_idle as i64],
        |r| {
            let start: i64 = r.get(4)?;
            let end: i64 = r.get(5)?;
            Ok(ForegroundInterval {
                id: r.get(0)?,
                app_id: r.get(1)?,
                app_name: r.get(2)?,
                display_name: r.get(3)?,
                start_time_ms: start,
                end_time_ms: end,
                duration_ms: (end - start).max(0),
                activity_state: r.get(6)?,
                is_hidden: r.get::<_, i64>(7)? != 0,
            })
        },
    )?;
    rows.collect()
}

pub fn timeline_available_dates(conn: &Connection) -> rusqlite::Result<Vec<String>> {
    let mut statement = conn.prepare(
        r#"WITH RECURSIVE
           bounds AS (
             SELECT date(min(start_time_ms) / 1000.0, 'unixepoch', 'localtime') AS min_day,
                    date(max(COALESCE(end_time_ms, last_seen_time_ms)) / 1000.0, 'unixepoch', 'localtime') AS max_day
             FROM foreground_interval
           ),
           dates(day) AS (
             SELECT min_day FROM bounds WHERE min_day IS NOT NULL
             UNION ALL
             SELECT date(day, '+1 day') FROM dates, bounds WHERE day < max_day
           )
           SELECT day FROM dates
           WHERE EXISTS (
             SELECT 1 FROM foreground_interval fi
             WHERE fi.start_time_ms < CAST(strftime('%s', datetime(day, '+1 day'), 'utc') AS INTEGER) * 1000
               AND COALESCE(fi.end_time_ms, fi.last_seen_time_ms) > CAST(strftime('%s', day || ' 00:00:00', 'utc') AS INTEGER) * 1000
           )
           ORDER BY day"#,
    )?;
    let rows = statement.query_map([], |row| row.get(0))?;
    rows.collect()
}

pub fn resource_available_dates(conn: &Connection) -> rusqlite::Result<Vec<String>> {
    let mut statement = conn.prepare(
        "SELECT DISTINCT date(ts / 1000.0, 'unixepoch', 'localtime') FROM sample_frame ORDER BY 1",
    )?;
    let rows = statement.query_map([], |row| row.get(0))?;
    rows.collect()
}

pub fn overview_available_dates(conn: &Connection) -> rusqlite::Result<Vec<String>> {
    let mut dates = BTreeSet::new();
    dates.extend(timeline_available_dates(conn)?);
    dates.extend(resource_available_dates(conn)?);
    Ok(dates.into_iter().collect())
}

pub fn usage_summary(
    conn: &Connection,
    start_ms: i64,
    end_ms: i64,
    include_hidden: bool,
) -> rusqlite::Result<Vec<AppUsageSummary>> {
    let intervals = foreground_intervals(conn, start_ms, end_ms, include_hidden, true)?;
    let total_active: i64 = intervals
        .iter()
        .filter(|i| i.activity_state == "active")
        .map(|i| i.duration_ms)
        .sum();
    let mut grouped = std::collections::BTreeMap::<i64, AppUsageSummary>::new();
    for interval in intervals {
        let item = grouped.entry(interval.app_id).or_insert(AppUsageSummary {
            app_id: interval.app_id,
            app_name: interval.app_name,
            display_name: interval.display_name,
            active_seconds: 0,
            idle_seconds: 0,
            percentage: 0.0,
            is_hidden: interval.is_hidden,
        });
        if interval.activity_state == "active" {
            item.active_seconds += interval.duration_ms / 1000;
        } else {
            item.idle_seconds += interval.duration_ms / 1000;
        }
    }
    let mut values: Vec<_> = grouped.into_values().collect();
    for item in &mut values {
        item.percentage = if total_active > 0 {
            item.active_seconds as f64 * 100_000.0 / total_active as f64
        } else {
            0.0
        };
    }
    values.sort_by_key(|value| std::cmp::Reverse(value.active_seconds));
    Ok(values)
}

pub fn system_samples(
    conn: &Connection,
    start_ms: i64,
    end_ms: i64,
    max_points: usize,
) -> rusqlite::Result<Vec<SystemSample>> {
    valid_range(start_ms, end_ms)?;
    let mut stmt = conn.prepare(
        r#"SELECT f.ts, f.duration_ms, cpu.usage_pct, memory.usage_pct, memory.used_bytes,
                  CASE WHEN memory.used_bytes IS NOT NULL AND memory.available_bytes IS NOT NULL
                       THEN memory.used_bytes + memory.available_bytes END,
                  disk.read_bps, disk.write_bps, f.process_snapshot_present
           FROM sample_frame f
           LEFT JOIN cpu_sample cpu ON cpu.frame_id = f.id
           LEFT JOIN memory_sample memory ON memory.frame_id = f.id
           LEFT JOIN (
             SELECT frame_id, SUM(read_bps) AS read_bps, SUM(write_bps) AS write_bps
             FROM disk_sample GROUP BY frame_id
           ) disk ON disk.frame_id = f.id
           WHERE f.ts >= ?1 AND f.ts < ?2
           ORDER BY f.ts"#,
    )?;
    let all: Vec<SystemSample> = stmt
        .query_map(params![start_ms, end_ms], |r| {
            Ok(SystemSample {
                timestamp_ms: r.get(0)?,
                sample_duration_ms: r.get(1)?,
                cpu_percent: r.get(2)?,
                memory_percent: r.get(3)?,
                memory_used_bytes: r.get(4)?,
                memory_total_bytes: r.get(5)?,
                disk_read_bytes_per_sec: r.get(6)?,
                disk_write_bytes_per_sec: r.get(7)?,
                has_app_snapshot: r.get::<_, i64>(8)? != 0,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    if all.len() <= max_points {
        return Ok(all);
    }
    let stride = all.len().div_ceil(max_points.max(1));
    Ok(all.into_iter().step_by(stride).collect())
}

pub fn app_resource_samples(
    conn: &Connection,
    timestamp_ms: i64,
) -> rusqlite::Result<Vec<AppResourceSample>> {
    let mut stmt = conn.prepare(
        r#"SELECT a.stable_key, a.process_name,
                  CASE WHEN e.normalized_path LIKE 'path:%' THEN substr(e.normalized_path, 6) END,
                  p.process_count, p.cpu_pct, p.working_set_bytes, p.read_bps, p.write_bps
           FROM process_sample p
           JOIN sample_frame f ON f.id = p.frame_id
           JOIN process_instance i ON i.id = p.process_instance_id
           JOIN app_executable e ON e.id = i.app_executable_id
           JOIN app a ON a.id = e.app_id
           WHERE f.ts = ?1
           ORDER BY p.cpu_pct DESC, p.working_set_bytes DESC"#,
    )?;
    let rows = stmt.query_map([timestamp_ms], |r| {
        Ok(AppResourceSample {
            app_key: r.get(0)?,
            process_name: r.get(1)?,
            exe_path: r.get(2)?,
            process_count: r.get(3)?,
            cpu_percent: r.get(4)?,
            memory_used_bytes: r.get(5)?,
            io_read_bytes_per_sec: r.get(6)?,
            io_write_bytes_per_sec: r.get(7)?,
        })
    })?;
    rows.collect()
}

pub fn resource_apps(conn: &Connection) -> rusqlite::Result<Vec<ResourceApp>> {
    let mut stmt = conn.prepare(
        r#"WITH samples AS (
             SELECT LOWER(a.process_name) AS process_key, a.process_name, a.display_name,
                     CASE WHEN e.normalized_path LIKE 'path:%' THEN substr(e.normalized_path, 6) END AS exe_path,
                     f.ts
             FROM process_sample p
             JOIN sample_frame f ON f.id = p.frame_id
             JOIN process_instance i ON i.id = p.process_instance_id
             JOIN app_executable e ON e.id = i.app_executable_id
             JOIN app a ON a.id = e.app_id
             WHERE a.process_name IS NOT NULL AND trim(a.process_name) <> ''
           ),
           ranked AS (
             SELECT samples.*,
                    ROW_NUMBER() OVER (PARTITION BY process_key ORDER BY ts DESC) AS row_number
             FROM samples
           ),
           friendly AS (
             SELECT process_key, display_name
             FROM (
               SELECT process_key, display_name,
                      ROW_NUMBER() OVER (PARTITION BY process_key ORDER BY ts DESC) AS row_number
               FROM samples
               WHERE LOWER(TRIM(display_name)) <> LOWER(TRIM(process_name))
             )
             WHERE row_number = 1
           )
           SELECT 'process:' || ranked.process_key, ranked.process_name,
                  COALESCE(friendly.display_name, ranked.display_name), ranked.exe_path, ranked.ts
           FROM ranked
           LEFT JOIN friendly ON friendly.process_key = ranked.process_key
           WHERE ranked.row_number = 1
           ORDER BY COALESCE(friendly.display_name, ranked.display_name) COLLATE NOCASE"#,
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(ResourceApp {
            app_key: r.get(0)?,
            process_name: r.get(1)?,
            display_name: r.get(2)?,
            exe_path: r.get(3)?,
            last_sample_at_ms: r.get(4)?,
        })
    })?;
    rows.collect()
}

pub fn app_resource_available_dates(
    conn: &Connection,
    app_key: &str,
) -> rusqlite::Result<Vec<String>> {
    let (predicate, key) = app_resource_predicate(app_key);
    let predicate = predicate.replace("?3", "?1");
    let sql = format!(
        "SELECT DISTINCT date(f.ts / 1000.0, 'unixepoch', 'localtime') FROM process_sample p JOIN sample_frame f ON f.id=p.frame_id JOIN process_instance i ON i.id=p.process_instance_id JOIN app_executable e ON e.id=i.app_executable_id JOIN app a ON a.id=e.app_id WHERE {predicate} ORDER BY 1"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([key], |row| row.get(0))?;
    rows.collect()
}

pub fn app_resource_history(
    conn: &Connection,
    app_key: &str,
    start_ms: i64,
    end_ms: i64,
    max_points: usize,
) -> rusqlite::Result<Vec<AppResourceHistoryPoint>> {
    valid_range(start_ms, end_ms)?;
    let (predicate, key) = app_resource_predicate(app_key);
    let sql = format!(
        r#"WITH app_samples AS (
             SELECT p.frame_id, SUM(p.cpu_pct) AS cpu_percent, SUM(p.working_set_bytes) AS memory_used_bytes,
                    SUM(p.read_bps) AS io_read_bytes_per_sec, SUM(p.write_bps) AS io_write_bytes_per_sec
             FROM process_sample p
             JOIN process_instance i ON i.id=p.process_instance_id
             JOIN app_executable e ON e.id=i.app_executable_id
             JOIN app a ON a.id=e.app_id
             WHERE {predicate}
             GROUP BY p.frame_id
           )
           SELECT f.ts, f.duration_ms, app.cpu_percent, app.memory_used_bytes,
                  app.io_read_bytes_per_sec, app.io_write_bytes_per_sec
            FROM sample_frame f
            LEFT JOIN app_samples app ON app.frame_id=f.id
            WHERE f.process_snapshot_present = 1
              AND f.ts >= ?1 AND f.ts < ?2
           ORDER BY f.ts"#
    );
    let mut stmt = conn.prepare(&sql)?;
    let all: Vec<AppResourceHistoryPoint> = stmt
        .query_map(params![start_ms, end_ms, key], |r| {
            Ok(AppResourceHistoryPoint {
                timestamp_ms: r.get(0)?,
                sample_duration_ms: r.get(1)?,
                cpu_percent: r.get(2)?,
                memory_used_bytes: r.get(3)?,
                io_read_bytes_per_sec: r.get(4)?,
                io_write_bytes_per_sec: r.get(5)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    if all.len() <= max_points {
        return Ok(all);
    }
    let stride = all.len().div_ceil(max_points.max(1));
    Ok(all.into_iter().step_by(stride).collect())
}

fn app_resource_predicate(app_key: &str) -> (&'static str, &str) {
    if let Some(process_name) = app_key.strip_prefix("process:") {
        ("LOWER(a.process_name) = LOWER(?3)", process_name)
    } else {
        ("a.stable_key = ?3", app_key)
    }
}

pub fn today_overview(
    conn: &Connection,
    start_ms: i64,
    end_ms: i64,
) -> rusqlite::Result<TodayOverview> {
    let all_usage = usage_summary(conn, start_ms, end_ms, true)?;
    let total_active_foreground_seconds = all_usage.iter().map(|a| a.active_seconds).sum();
    let total_idle_foreground_seconds = all_usage.iter().map(|a| a.idle_seconds).sum();
    let hidden_active_foreground_seconds = all_usage
        .iter()
        .filter(|a| a.is_hidden)
        .map(|a| a.active_seconds)
        .sum();
    let visible_active_seconds: i64 = all_usage
        .iter()
        .filter(|app| !app.is_hidden)
        .map(|app| app.active_seconds)
        .sum();
    let mut top_apps: Vec<_> = all_usage
        .into_iter()
        .filter(|a| !a.is_hidden)
        .take(10)
        .collect();
    for app in &mut top_apps {
        app.percentage = if visible_active_seconds > 0 {
            app.active_seconds as f64 * 100.0 / visible_active_seconds as f64
        } else {
            0.0
        };
    }
    let samples = system_samples(conn, start_ms, end_ms, 10_000)?;
    Ok(TodayOverview {
        start_ms,
        end_ms,
        total_active_foreground_seconds,
        total_idle_foreground_seconds,
        hidden_active_foreground_seconds,
        top_apps,
        cpu_sampled_peak: samples
            .iter()
            .filter_map(|s| s.cpu_percent)
            .reduce(f64::max),
        memory_sampled_peak: samples
            .iter()
            .filter_map(|s| s.memory_percent)
            .reduce(f64::max),
        disk_read_sampled_peak: samples
            .iter()
            .filter_map(|s| s.disk_read_bytes_per_sec)
            .max(),
        disk_write_sampled_peak: samples
            .iter()
            .filter_map(|s| s.disk_write_bytes_per_sec)
            .max(),
    })
}
